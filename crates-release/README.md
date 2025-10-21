This contains a WIP CLI project (written in Rust) for the management of publishing Rust crates.

## Current features

### Publishing crates

- Publishing multiple crates though comma delimeted `package_name=argument` syntax
- Ability to change version to absolute or relative (`patch`, `minor`, `major`)
- Associate `Cargo.toml` and `package.json` version sync via `package.metadata.associated` array or value
- Update local path dependency versions

> Detecting workspaces is done through parsing JSON output of `cargo metadata --offline --format-version 1 --no-deps`.
>
> While this JSON output could be used for name and version information etc. The program wants the actual string offsets of the these fields, so it knows where to update. This updating based on a position, 
> ensures only the value in the source is changed and not formatting or comments. There may be alternatives. The program could perform `cargo metadata` itself, but for now this seems to be the most in-keeping way.

#### Version sync

Sometimes you may have two or more crates/packages in a workspace which have close functionality and you want to keep their versions in-sync. There are several ways to do this.

To keep all crates in a `Cargo.toml` workspace you can do

```toml
[workspace]
members = [
	"..."
]
#
metadata.version_sync = true
#
```

Or in individual `Cargo.toml`s, you can specify paths to `Cargo.toml` or `package.json` (or their parent path). This can be as a single path or array of paths

```toml
...
```

This can include references to `package.json` (the node/JavaScript manifest standard). 

```json
...
```

> Note that it does not update local dependency versions and you can only specify `version_sync` in `Cargo.toml`

### Verifying crates

This supports features not present under `cargo publish --dry-run`

- Verifies all required keys exist
- Recommends on missing keys
- Checks categories and keywords are valid
- Errors for local `path` dependencies outside of workspace and `git` dependencies

## Implementation of features

Both features use [simple-toml-parser](https://github.com/kaleidawave/simple-toml-parser) and [simple-json-parser](https://github.com/kaleidawave/simple-json-parser) libraries for parsing command output and package manifests.

> Again, parsing the TOML gives position information in the source so that (once supported) the tool can give positional diagnostics of the error

### Publishing

- Cargo manifests are found through parsing JSON output of `cargo manifest` command
- `Cargo.toml` and `package.json` are scanned and *spans* (pointer offsets into a `str`) are found of places of *information* along with the original content in the file
- A first pass looks at the results of parsing the manifests (name and existing version) and calculates new versions and stores them in a map
- A second pass computes a list of changes/rewrites to content in those spans
- Then the existing and new parts are written to a file
- The version changes map is returned to the CLI for reporting information