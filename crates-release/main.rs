fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("publish") => {
            todo!();
        }
        Some("verify") => {
            todo!();
        }
        Some("change-version") => {
            let Some(argument) = args.next() else {
                panic!("expected version argument");
            };

            let rest: Vec<_> = args.collect();
            let dry_run = rest.iter().any(|arg| arg == "--dry-run");

            let transformation = Transformation(argument);

            let mut command = std::process::Command::new("cargo");
            command.args([
                "metadata",
                "--offline",
                "--format-version",
                "1",
                "--no-deps",
            ]);

            let output = command.output()?;
            let content = String::from_utf8(output.stdout)?;

            let options = Options { dry_run };

            let result = {
                use simple_json_parser::{JSONKey, RootJSONValue, parse as parse_json};

                parse_json(&content, |keys, value| {
                    if let &[
                        JSONKey::Slice("packages"),
                        JSONKey::Index(_),
                        JSONKey::Slice("manifest_path"),
                    ] = keys
                    {
                        let RootJSONValue::String(toml_path) = value else {
                            panic!();
                        };

                        update_toml(
                            std::path::Path::new(toml_path),
                            transformation.clone(),
                            options,
                        );
                    }
                })
            };

            assert!(result.is_ok(), "JSON did not parse");

            Ok(())
        }
        None | Some("info") | Some("--help") => {
            todo!()
        }
        command => {
            panic!("unknown command {command:?}. See --help");
        }
    }
}

#[derive(Clone, Copy)]
pub struct Options {
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct Transformation(pub String);

fn update_toml(path: &std::path::Path, transformation: Transformation, options: Options) {
    use simple_toml_parser::{RootTOMLValue, TOMLKey, parse as parse_toml};
    use std::fs::File;

    let mut toml_changes: Changes = Vec::new();
    let mut new_version: Transformation = transformation.clone();

    let mut name = "";

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
            let new_version_ = update_version(existing_version, &transformation).to_string();

            let github_output_file = std::env::vars()
                .find_map(|(name, value)| (name == "GITHUB_OUTPUT").then_some(value));

            if let Some(file) = github_output_file {
                use std::io::Write;

                eprintln!("writing '{name}={new_version_}' to {file}");
                writeln!(&mut File::create(file).unwrap(), "{name}={new_version_}").unwrap();
            }

            new_version = Transformation(new_version_);
            toml_changes.push((start..(start + raw.len()), new_version.0.clone()));
        } else if let &[
            TOMLKey::Slice("package"),
            TOMLKey::Slice("metadata"),
            TOMLKey::Slice("associated"),
            TOMLKey::Index(_),
        ] = keys
        {
            if let RootTOMLValue::String(json_path) = value {
                let path = path.parent().unwrap().join(json_path.raw());
                update_json(&path, &new_version, options);
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

fn update_json(path: &std::path::Path, transformation: &Transformation, options: Options) {
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

            json_changes.push((start..(start + raw.len()), transformation.0.clone()));
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

fn update_version(existing: semver::Version, argument: &Transformation) -> semver::Version {
    match argument.0.as_str() {
        "patch" => semver::Version::new(existing.major, existing.minor, existing.patch + 1),
        "minor" => semver::Version::new(existing.major, existing.minor + 1, existing.patch),
        "major" => semver::Version::new(existing.major + 1, existing.minor, existing.patch),
        version => semver::Version::parse(version).unwrap(),
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
