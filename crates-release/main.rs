mod update;

use std::str::FromStr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref().unwrap_or("help") {
        "publish" => {
            todo!("change version + cargo publish + git commit");
        }
        "verify" => {
            todo!()
        }
    match args.next().as_deref() {
        Some("publish") => {
            todo!();
        }
        Some("verify") => {
            todo!();
        }
        Some("change-version") => {
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
                        );
                    }
                })
            };

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
