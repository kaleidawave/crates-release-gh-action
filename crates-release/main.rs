mod update;
mod verify;

use std::collections::HashMap;
use std::str::FromStr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref().unwrap_or("help") {
        "publish" => {
            todo!("change version + cargo publish + git commit");
        }
        "verify" => {
            let path = args.next();

            if let Some(path) = path
                && path.ends_with("Cargo.toml")
            {
                let content = std::fs::read_to_string(path).unwrap();
                verify::verify(&content);
            } else {
                let content = {
                    let mut command = std::process::Command::new("cargo");
                    command.args([
                        "metadata",
                        "--offline",
                        "--format-version",
                        "1",
                        "--no-deps",
                    ]);

                    let output = command.output()?;
                    String::from_utf8(output.stdout)?
                };

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

                            let content = std::fs::read_to_string(toml_path).unwrap();

                            verify::verify(&content);
                        }
                    })
                };

                assert!(result.is_ok(), "JSON did not parse");
            }

            Ok(())
        }
        "change-version" => {
            let Some(argument) = args.next() else {
                panic!("expected version argument");
            };

            let mut dry_run = false;
            for arg in args {
                let arg = arg.as_str();
                if let "--dry-run" = arg {
                    dry_run = true;
                }
            }

            let transformation = update::ProjectTransformation::from_str(&argument)?;

            let content = {
                let mut command = std::process::Command::new("cargo");
                command.args([
                    "metadata",
                    "--offline",
                    "--format-version",
                    "1",
                    "--no-deps",
                ]);

                let output = command.output()?;
                String::from_utf8(output.stdout)?
            };

            let options = update::Options { dry_run };

            let mut changes: update::VersionChanges = HashMap::new();

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

                        update::update_toml(
                            std::path::Path::new(toml_path),
                            &transformation,
                            options,
                            &mut changes,
                        );
                    }
                })
            };

            let github_output_file = std::env::vars()
                .find_map(|(name, value)| (name == "GITHUB_OUTPUT").then_some(value));

            if let Some(github_output_file) = github_output_file {
                use std::fs::File;
                use std::io::Write;

                let mut file = File::options()
                    .append(true)
                    .create(true)
                    .open(&github_output_file)
                    .expect("cannot open file");

                let value = if changes.len() == 1 {
                    changes.values().next().unwrap()
                } else {
                    "*multiple"
                };

                writeln!(&mut file, "new_version={value}").unwrap();

                // json_array
                {
                    let mut buf = String::new();
                    json_builder_macro::ToJSON::append(
                        &changes
                            .iter()
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>(),
                        &mut buf,
                    );
                    writeln!(&mut file, "new_versions={buf}").unwrap();
                }

                // json_object
                {
                    let mut buf = String::new();
                    json_builder_macro::ToJSON::append(&changes, &mut buf);
                    writeln!(&mut file, "new-versions-json-object={buf}").unwrap();
                }

                {
                    let description = format(&changes);
                    writeln!(&mut file, "new-versions-description={description}").unwrap();
                }

                // per thingy
                for (name, new_version) in changes {
                    eprintln!("writing '{name}={new_version}' to {github_output_file}");
                    writeln!(&mut file, "{name}={new_version}").unwrap();
                }
            }

            assert!(result.is_ok(), "JSON did not parse");

            Ok(())
        }
        "info" | "--help" => {
            println!("crates-release");
            println!("helper for publishing crate(s). Updates version fields across `Cargo.toml`s");
            println!("verifys and lints `Cargo.toml`s before publish");
            Ok(())
        }
        command => {
            panic!("unknown command {command:?}. See --help");
        }
    }
}

fn format(changes: &HashMap<String, String>) -> String {
    let mut description: String = String::default();
    let mut values = changes.iter();
    for (name, version) in values.by_ref().take(changes.len().saturating_sub(1)) {
        if !description.is_empty() {
            description.push_str(", ");
        }
        description.push_str(name);
        description.push_str(" to ");
        description.push_str(version);
    }
    if changes.len() > 1 {
        description.push_str(" and ");
    }

    {
        let (name, version) = values.next().unwrap();
        description.push_str(name);
        description.push_str(" to ");
        description.push_str(version);
    }

    description
}
