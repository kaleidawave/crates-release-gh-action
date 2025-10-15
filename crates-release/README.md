This contains a WIP CLI project (written in Rust) for the management of publishing Rust crates.

## Current features

### Publishing crates

- Publishing multiple crates though comma delimeted `package_name=argument` syntax
- Ability to change version to absolute or relative (`patch`, `minor`, `major`)
- Associate `Cargo.toml` and `package.json` version sync via `package.metadata.associated` array or value
- Update local path dependency versions

### Verifying crates

This supports features not present under `cargo publish --dry-run`

- Verifies all required keys exist
- Recommends on missing keys
- Checks categories and keywords are valid
- Errors for local `path` dependencies outside of workspace and `git` dependencies

## Implementation

Both features use [simple-toml-parser](https://github.com/kaleidawave/simple-toml-parser) and [simple-json-parser](https://github.com/kaleidawave/simple-json-parser) libraries for parsing command output and package manifests.

### Publishing

- Cargo manifests are found through parsing JSON output of `cargo manifest` command
- `Cargo.toml` and `package.json` are scanned and *spans* (pointer offsets into a `str`) are found of places of *information* along with the original content in the file
- A first pass looks at the results of parsing the manifests (name and existing version) and calculates new versions and stores them in a map
- A second pass computes a list of changes/rewrites to content in those spans
- Then the existing and new parts are written to a file
- The version changes map is returned to the CLI for reporting information