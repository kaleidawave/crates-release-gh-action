#[derive(Clone, Copy)]
pub struct Options {
    pub dry_run: bool,
}

pub fn update_toml(
    path: &std::path::Path,
    transformation: &ProjectTransformation,
    options: Options,
) {
    use simple_toml_parser::{RootTOMLValue, TOMLKey, parse_toml};
    use std::fs::File;

    let mut toml_changes: Changes = Vec::new();

    let mut name = "";
    let mut new_version = None;

    let content = std::fs::read_to_string(path).unwrap();
    let result = parse_toml(&content, |keys, value| {
        if let &[TOMLKey::Slice("package"), TOMLKey::Slice("name")] = keys {
            let RootTOMLValue::String(value) = value else {
                panic!();
            };
            name = value.raw();
        } else if let &[TOMLKey::Slice("package"), TOMLKey::Slice("version")] = keys {
            let RootTOMLValue::String(value) = value else {
                panic!();
            };
            let raw = value.raw();
            let start = (raw.as_ptr() as usize)
                .checked_sub(content.as_ptr() as usize)
                .expect("slice not in whole");

            let existing_version = semver::Version::parse(raw).unwrap();
            let new_version_: semver::Version = transformation
                .get_transformation_for_package(name)
                .apply(existing_version);

            let github_output_file = std::env::vars()
                .find_map(|(name, value)| (name == "GITHUB_OUTPUT").then_some(value));

            if let Some(github_output_file) = github_output_file {
                use std::io::Write;

                eprintln!("writing '{name}={new_version_}' to {github_output_file}");
                let mut file = File::options()
                    .append(true)
                    .create(true)
                    .open(github_output_file)
                    .expect("cannot open file");
                writeln!(&mut file, "{name}={new_version_}").unwrap();
            }

            toml_changes.push((start..(start + raw.len()), new_version_.to_string()));
            new_version = Some(new_version_);
        } else if let &[
            TOMLKey::Slice("package"),
            TOMLKey::Slice("metadata"),
            TOMLKey::Slice("associated"),
            TOMLKey::Index(_),
        ] = keys
        {
            if let RootTOMLValue::String(nested) = value {
                let version = new_version.clone().expect("version not set");
                let path = path.parent().unwrap().join(nested.raw());
                let extension = path.extension().and_then(|ext| ext.to_str());
                if let Some("json") = extension {
                    update_json(&path, version.to_string(), options);
                } else if let Some("toml") = extension {
                    let new_version =
                        ProjectTransformation::Singular(Transformation::Exact(version));
                    update_toml(&path, &new_version, options);
                } else {
                    eprintln!("Cannot update {path}", path = path.display());
                }
            } else {
                eprintln!("unknown {keys:?} {value:?}");
            }
        }
    });

    assert!(result.is_ok(), "TOML did not parse");

    if options.dry_run {
        let revised = apply_changes_to_string(&content, toml_changes);
        println!("{revised}");
    } else {
        apply_changes(&mut File::create(path).unwrap(), &content, toml_changes).unwrap();
    }
}

/// for associated versions
fn update_json(path: &std::path::Path, new_version: String, options: Options) {
    use simple_json_parser::{JSONKey, RootJSONValue, parse as parse_json};
    use std::fs::File;

    let mut json_changes: Changes = Vec::new();
    let content = std::fs::read_to_string(path).unwrap();
    let result = parse_json(&content, |keys, value| {
        if let &[JSONKey::Slice("version")] = keys {
            let RootJSONValue::String(value) = value else {
                panic!();
            };
            let raw = value;
            let start = (raw.as_ptr() as usize)
                .checked_sub(content.as_ptr() as usize)
                .expect("slice not in whole");

            json_changes.push((start..(start + raw.len()), new_version.clone()));
        }
    });

    assert!(result.is_ok(), "JSON did not parse");

    if options.dry_run {
        let revised = apply_changes_to_string(&content, json_changes);
        println!("{revised}");
    } else {
        apply_changes(&mut File::create(path).unwrap(), &content, json_changes).unwrap();
    }
}

#[derive(Debug)]
pub enum Transformation {
    Keep,
    Patch,
    Minor,
    Major,
    Exact(semver::Version),
}

impl std::str::FromStr for Transformation {
    type Err = semver::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "keep" => Ok(Self::Keep),
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            version => semver::Version::parse(version).map(Self::Exact),
        }
    }
}

impl Transformation {
    pub fn apply(&self, existing: semver::Version) -> semver::Version {
        match self {
            Self::Keep => existing,
            Self::Patch => semver::Version::new(existing.major, existing.minor, existing.patch + 1),
            Self::Minor => semver::Version::new(existing.major, existing.minor + 1, existing.patch),
            Self::Major => semver::Version::new(existing.major + 1, existing.minor, existing.patch),
            Self::Exact(new_version) => new_version.clone(),
        }
    }
}

#[derive(Debug)]
pub enum ProjectTransformation {
    Singular(Transformation),
    Workspace(std::collections::HashMap<String, Transformation>),
}

impl std::str::FromStr for ProjectTransformation {
    type Err = semver::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('=') {
            s.split(',')
                .map(|part| {
                    let (name, transform) =
                        part.split_once('=').expect("expected 'name=*transform*'");
                    transform.parse().map(|version| (name.to_owned(), version))
                })
                .collect::<Result<std::collections::HashMap<_, _>, _>>()
                .map(ProjectTransformation::Workspace)
        } else {
            s.parse().map(ProjectTransformation::Singular)
        }
    }
}

static KEEP: Transformation = Transformation::Keep;

impl ProjectTransformation {
    /// returns [`Transformation::Keep`] if nothing specified
    pub fn get_transformation_for_package(&self, package_name: &str) -> &Transformation {
        match self {
            Self::Singular(transform) => transform,
            Self::Workspace(packages) => packages.get(package_name).unwrap_or(&KEEP),
        }
    }
}

pub type SliceRange = std::ops::Range<usize>;
pub type Changes = Vec<(SliceRange, String)>;

pub fn apply_changes<W: std::io::Write>(
    to: &mut W,
    on: &str,
    changes: Changes,
) -> std::io::Result<()> {
    let mut cur = 0;
    for (range, item) in changes {
        let (lhs, rhs) = (range.start, range.end);
        write!(to, "{item}", item = &on[cur..lhs])?;
        write!(to, "{item}")?;
        cur = rhs;
    }
    write!(to, "{item}", item = &on[cur..])
}

pub fn apply_changes_to_string(on: &str, changes: Changes) -> String {
    let mut buf = Vec::new();
    apply_changes(&mut buf, on, changes).unwrap();
    // TODO check on debug
    unsafe { String::from_utf8_unchecked(buf) }
}
