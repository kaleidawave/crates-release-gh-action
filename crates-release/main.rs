mod update;
mod verify;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref().unwrap_or("--help") {
        "info" | "--help" => {
            let after = args.next();
            if let Some("reserved") = after.as_deref() {
                println!("Reserved:");
                for (i, word) in verify::RESERVED.split(',').enumerate() {
                    let i = i % 6;
                    if i == 5 {
                        println!();
                    } else if i == 0 {
                        print!("{word}");
                    } else {
                        print!(" {word}");
                    }
                }
            } else if let Some("categories") = after.as_deref() {
                println!("Categories:");
                for (i, word) in verify::CATEGORIES.split(',').enumerate() {
                    let i = i % 6;
                    if i == 5 {
                        println!();
                    } else if i == 0 {
                        print!("{word}");
                    } else {
                        print!(" {word}");
                    }
                }
            } else {
                println!("crates-release");
                println!("helpers for publishing crate(s)");
                println!("update: Updates version fields across `Cargo.toml`s");
                println!(
                    "verify: verifys and lints `Cargo.toml`s for whether they are ready to be published"
                );
            }
            Ok(())
        }
        "verify" => {
            let path = args.next();

            if let Some(path) = path
                && path.ends_with("Cargo.toml")
            {
                let path = Path::new(&path);
                let content = std::fs::read_to_string(path).unwrap();
                // TODO this might not quite be the root. Maybe cwd?
                let root = path.parent().unwrap();
                verify::verify(&content, path, root);
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

                let mut toml_paths = Vec::new();
                let mut workspace_root = PathBuf::new();

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
                            toml_paths.push(PathBuf::from(toml_path));
                        } else if let &[JSONKey::Slice("workspace_root")] = keys {
                            let RootJSONValue::String(toml_path) = value else {
                                panic!();
                            };
                            workspace_root = PathBuf::from(toml_path);
                        }
                    })
                };

                for toml_path in toml_paths {
                    let content = std::fs::read_to_string(&toml_path).unwrap();
                    verify::verify(&content, &toml_path, &workspace_root);
                }

                assert!(result.is_ok(), "JSON did not parse");
            }

            Ok(())
        }
        "change-version" => {
            let Some(argument) = args.next() else {
                panic!("expected version argument");
            };

            let mut dry_run = false;
            let mut print_new_version = false;
            for arg in args {
                match arg.as_str() {
                    "--dry-run" => {
                        dry_run = true;
                    }
                    "--print-new-version" => {
                        print_new_version = true;
                    }
                    arg => {
                        eprintln!("unknown arg {arg:?}");
                    }
                }
            }

            let transformation = update::ProjectTransformation::from_str(&argument)?;

            let options = update::Options { dry_run };

            let changes = update::update_cargo_workspace(&transformation, options)?;

            if changes.is_empty() {
                panic!("no crates update. make sure you are in a cargo workspace");
            }

            let new_version = if changes.len() == 1 {
                changes.values().next().unwrap()
            } else {
                "*multiple*"
            };

            if print_new_version {
                println!("{new_version}");
            }

            let github_output_file = std::env::vars()
                .find_map(|(name, value)| (name == "GITHUB_OUTPUT").then_some(value));

            if let Some(github_output_file) = github_output_file {
                use json_builder_macro::ToJSON;
                use std::fs::File;
                use std::io::Write;

                let mut file = File::options()
                    .append(true)
                    .create(true)
                    .open(&github_output_file)
                    .expect("cannot open file");

                // single version
                writeln!(&mut file, "new-version={new_version}").unwrap();

                // json_array
                let versions_array = changes
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .as_json_string();

                writeln!(&mut file, "new-versions={versions_array}").unwrap();

                // json_object
                let versions_object = changes.as_json_string();
                writeln!(&mut file, "new-versions-json-object={versions_object}").unwrap();

                // description
                let description = format(&changes);
                writeln!(&mut file, "new-versions-description={description}").unwrap();

                // per thingy
                for (name, new_version) in changes {
                    eprintln!("writing '{name}={new_version}' to {github_output_file}");
                    writeln!(&mut file, "{name}={new_version}").unwrap();
                }
            }

            // Update local dependencies in lockfile
            std::process::Command::new("cargo")
                .arg("c")
                .output()
                .unwrap();

            Ok(())
        }
        "publish" => {
            todo!("change version + cargo publish + git commit");
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

    if let Some((name, version)) = values.next() {
        description.push_str(name);
        description.push_str(" to ");
        description.push_str(version);
    }

    description
}
