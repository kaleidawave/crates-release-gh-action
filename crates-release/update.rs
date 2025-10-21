pub type SliceRange = std::ops::Range<usize>;
pub type FileChanges = Vec<(SliceRange, String)>;

#[derive(Clone, Copy)]
pub struct Options {
    pub dry_run: bool,
}

pub struct Manifest {
    pub content: String,
    pub association: Option<String>,
    pub name_span: SliceRange,
    pub version_span: SliceRange,
    pub local_dependency_spans: Vec<(SliceRange, SliceRange)>,
}

pub type VersionChanges = std::collections::HashMap<String, String>;
pub type Manifests = Vec<(std::path::PathBuf, Manifest)>;

pub fn update_cargo_workspace(
    transformation: &ProjectTransformation,
    options: Options,
) -> Result<VersionChanges, Box<dyn std::error::Error>> {
    use simple_json_parser::{JSONKey, RootJSONValue, parse as parse_json};
    use std::path::Path;

    let cargo_metadata = {
        let mut command = std::process::Command::new("cargo");
        command.args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--no-deps",
        ]);

        let output = command.output()?;

        if let Some(101) = output.status.code() {
            return Err(Box::from(
                "cargo metadata returned 101. make sure you are in a cargo workspace",
            ));
        }

        String::from_utf8(output.stdout)?
    };

    let mut version_changes: VersionChanges = VersionChanges::new();
    let mut manifests: Manifests = Manifests::new();

    let mut workspace_members = 0;
    let mut workspace_version_sync = false;

    let result = parse_json(&cargo_metadata, |keys, value| {
        // We pass the path, as we want positional information
        if let &[
            JSONKey::Slice("packages"),
            JSONKey::Index(_),
            JSONKey::Slice("manifest_path"),
        ] = keys
        {
            let RootJSONValue::String(path) = value else {
                panic!();
            };

            workspace_members += 1;

            let path = Path::new(path);

            // Skip if already read from version_sync
            let skip = manifests
                .iter()
                .any(|(manifest_path, _)| path == manifest_path);

            if !skip {
                let result = parse_cargo_toml(path, &mut manifests, None);

                // TODO lift error through
                result.unwrap();
            }
        } else if let &[JSONKey::Slice("metadata"), JSONKey::Slice("version_sync")] = keys {
            workspace_version_sync = matches!(value, RootJSONValue::Boolean(true));
        }
    });

    assert!(result.is_ok(), "JSON did not parse");

    if let ProjectTransformation::Singular(_) = transformation
        && !workspace_version_sync
        && workspace_members > 1
    {
        return Err(Box::from(
            "updating workspace with >1 package with single transform requires 'metadata.version_sync = true'",
        ));
    }

    // Pass one: calculate version changes
    for (_, manifest) in manifests.iter().rev() {
        let name = manifest.content[manifest.name_span.clone()].to_owned();

        // Can happen for version_sync versions
        if let Some(ref association) = manifest.association {
            // This is okay because manifests order is
            let exsting = version_changes.get(association).unwrap().clone();
            version_changes.insert(name, exsting);
        } else {
            let existing_version = &manifest.content[manifest.version_span.clone()];
            let existing_version = semver::Version::parse(existing_version).unwrap();
            let new_version: semver::Version = transformation
                .get_transformation_for_package(&name)
                .apply(existing_version);

            version_changes.insert(name, new_version.to_string());
        }
    }

    // Pass two: update versions, associates and path dependencies and write to file
    for (path, manifest) in manifests {
        let Manifest {
            content,
            association,
            name_span,
            local_dependency_spans,
            version_span,
        } = manifest;
        let mut changes = FileChanges::new();

        let new_version = if let Some(association) = association {
            version_changes.get(&association)
        } else {
            version_changes.get(&content[name_span])
        };

        if let Some(new_version) = new_version {
            changes.push((version_span, new_version.to_owned()));
        }

        for (name_span, version_span) in local_dependency_spans {
            let new_version = version_changes.get(&content[name_span]);
            if let Some(new_version) = new_version {
                changes.push((version_span, new_version.to_owned()));
            }
        }

        if options.dry_run {
            let revised = apply_changes_to_string(&content, changes);
            println!("{path}:{revised}", path = path.display());
        } else {
            let mut file = std::fs::File::create(path).unwrap();
            apply_changes(&mut file, &content, changes).unwrap();
        }
    }

    Ok(version_changes)
}

fn is_version_sync_key_chain(keys: &[simple_toml_parser::TOMLKey]) -> bool {
    use simple_toml_parser::TOMLKey::{Index as I, Slice as S};
    matches!(
        keys,
        &[S("package"), S("metadata"), S("version_sync"), I(_)]
    ) || matches!(keys, &[S("package"), S("metadata"), S("version_sync")])
}

fn extract_slice_position(subslice: &str, slice: &str) -> SliceRange {
    let start = (subslice.as_ptr() as usize)
        .checked_sub(slice.as_ptr() as usize)
        .expect("slice not in whole");
    let end = start + subslice.len();
    start..end
}

fn parse_cargo_toml(
    path: &std::path::Path,
    manifests: &mut Manifests,
    association: Option<String>,
) -> std::io::Result<()> {
    use simple_toml_parser::{RootTOMLValue, TOMLKey, parse_toml};

    let content = std::fs::read_to_string(path)?;

    let mut manifest = Manifest {
        name_span: 0..0,
        version_span: 0..0,
        local_dependency_spans: Vec::new(),
        association,
        content,
    };

    let result = parse_toml(&manifest.content, |keys, value| {
        if let &[TOMLKey::Slice("package"), TOMLKey::Slice("name")] = keys {
            let RootTOMLValue::String(value) = value else {
                panic!();
            };
            manifest.name_span = extract_slice_position(value.raw(), &manifest.content);
        } else if let &[TOMLKey::Slice("package"), TOMLKey::Slice("version")] = keys {
            let RootTOMLValue::String(value) = value else {
                panic!();
            };

            manifest.version_span = extract_slice_position(value.raw(), &manifest.content);
        } else if is_version_sync_key_chain(keys) {
            if let RootTOMLValue::String(nested) = value {
                let path = path
                    .parent()
                    .unwrap()
                    .join(nested.raw())
                    .canonicalize()
                    .unwrap();

                let path = if path.is_dir() {
                    // TODO if exists, else package.json
                    path.join("Cargo.toml")
                } else {
                    path
                };

                let extension = path.extension().and_then(|ext| ext.to_str());
                let associate = Some(manifest.content[manifest.name_span.clone()].to_owned());

                if let Some("json") = extension {
                    // TODO lift error
                    parse_package_json(&path, manifests, associate).unwrap();
                } else if let Some("toml") = extension {
                    // TODO lift error
                    parse_cargo_toml(&path, manifests, associate).unwrap();
                } else {
                    eprintln!(
                        "Cannot update {path}. Unknown format",
                        path = path.display()
                    );
                }
            } else {
                eprintln!("unknown {keys:?} {value:?}");
            }
        } else if let &[
            TOMLKey::Slice("dependencies"),
            TOMLKey::Slice(name),
            TOMLKey::Slice("version"),
        ] = keys
        {
            let RootTOMLValue::String(value) = value else {
                panic!();
            };

            let name = extract_slice_position(name, &manifest.content);
            let version = extract_slice_position(value.raw(), &manifest.content);
            manifest.local_dependency_spans.push((name, version));
        }
    });

    assert!(result.is_ok(), "TOML did not parse");

    manifests.push((path.to_owned(), manifest));

    Ok(())
}

/// for version_sync versions
fn parse_package_json(
    path: &std::path::Path,
    manifests: &mut Manifests,
    association: Option<String>,
) -> std::io::Result<()> {
    use simple_json_parser::{JSONKey, RootJSONValue, parse as parse_json};

    let content = std::fs::read_to_string(path)?;

    let mut manifest = Manifest {
        name_span: 0..0,
        version_span: 0..0,
        local_dependency_spans: Vec::new(),
        association,
        content,
    };

    let result = parse_json(&manifest.content, |keys, value| {
        if let &[JSONKey::Slice("version")] = keys {
            let RootJSONValue::String(value) = value else {
                panic!();
            };
            manifest.version_span = extract_slice_position(value, &manifest.content);
        }
    });

    assert!(result.is_ok(), "JSON did not parse");

    manifests.push((path.to_owned(), manifest));

    Ok(())
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

pub fn apply_changes<W: std::io::Write>(
    to: &mut W,
    on: &str,
    changes: FileChanges,
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

pub fn apply_changes_to_string(on: &str, changes: FileChanges) -> String {
    let mut buf = Vec::new();
    apply_changes(&mut buf, on, changes).unwrap();
    // TODO check on debug
    unsafe { String::from_utf8_unchecked(buf) }
}
