# Crates release GitHub action

Action for automatic incrementing of crate version and publishing to crates.io

Inputs: 
- `version`, the new version can be major/minor/patch or semver. Defaults to "patch"
- `crates-token`, A crates.io publishing token
- `working-directory`, The directory which `Cargo.toml` is in. Defaults to "."

Outputs:
- `new-version`, the new version in semver form

### Example usage

The following example is a [dispatch_workflow](https://docs.github.com/en/actions/managing-workflow-runs/manually-running-a-workflow) for updating updating the crate version, releasing on crates.io, creating a git tag and pushing updated `Cargo.toml` to the repository.

```yml
name: Release crate

on:
  workflow_dispatch:
    inputs:
      version:
        description: "major/minor/patch or semver"
        required: false
        default: "patch"

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Set git credentials
        run: |
          git config user.name github-actions
          git config user.email github-actions@github.com
      - name: Crates publish
        uses: kaleidawave/crates-release-gh-action@v1
        with:
          version: ${{ github.event.inputs.version }}
          crates-token: ${{ secrets.CARGO_REGISTRY_TOKEN }}
          working-directory: .
      - name: Push updated Cargo.toml
        run: |
          git tag "v${{ steps.release.outputs.new-version }}"
          git add .
          git commit -m "Release: ${{ steps.release.outputs.new-version }}"
          git push --tags origin main
```

This can then be run either from the web gui or using the [GitHub cli](https://cli.github.com/):

```
gh workflow run crates.yml -f version=patch
```