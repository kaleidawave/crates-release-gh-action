"""
An updater of Rust crates according to input arguments for use in the GitHub Action.
Can be as simple as just one crate. However also works in monorepos. 
Has several steps:
- Finds local `Cargo.toml`s using `cargo metadata` command
- (for monorepos) Figures out flat dependencies
- (for monorepos) Sorts crates in order of which they import each other
- Parses command line argument
	- (for monorepos) A JSON object relates to crate-name to argument pairs. 
	- Else just a single argument for
- For package(s)
	- Applies arguments
	- (for monorepos) Updates changes local dependencies to their updated version
	- Saves changes to filesystem
"""

import sys
import subprocess
import json
from os import environ
from tomlkit import dumps, parse
from semver import bump_major, bump_minor, bump_patch, VersionInfo
from functools import cmp_to_key

command = "cargo metadata --offline --format-version 1 --no-deps".split(" ")
output = subprocess.run(command, capture_output=True).stdout
packages = json.loads(output)["packages"]

flat_packages = dict()
upgraded_crates, manifest_cache = dict(), dict()
local_pkgs = set(map(lambda pkg: pkg["name"], packages))
awaiting = dict()

for package in packages:
    dependency_names = list(map(lambda pkg: pkg["name"], package["dependencies"]))
    for dependency_name in dependency_names.copy():
        if dependency_name in local_pkgs:
            if dependency_name in flat_packages:
                dependency_names.extend(flat_packages[dependency_name])
            else:
                awaiting.setdefault(dependency_name, []).append(package["name"])

    for awaitee in awaiting.pop(package["name"], []):
        flat_packages[awaitee].extend(dependency_names)

    flat_packages[package["name"]] = dependency_names


def package_includes_other(package1, package2):
    global flat_packages

    if package1 == package2:
        return 0
    elif package1["name"] in flat_packages[package2["name"]]:
        return -1
    elif package2["name"] in flat_packages[package1["name"]]:
        return 1
    else:
        return 0

packages = sorted(packages, key=cmp_to_key(package_includes_other))

def toml_file_to_dict(path):
	with open(path, "r") as f:
		toml = parse(f.read())
		f.close()
		return toml

def bump_version(existing_version, behavior):
	match behavior:
		case "patch":
			return bump_patch(existing_version)
		case "minor":
			return bump_minor(existing_version)
		case "major":
			return bump_major(existing_version)
		case _:
			raise ValueError(f"Expected 'patch', 'minor', 'major' or 'none'. Found '{behavior}'")

def write_toml(path, toml):
	with open(path, "w") as f:
		f.write(dumps(toml))
		f.close()

def update_cargo_toml(crate_cargo_toml, argument):
	if argument == "none":
		return crate_cargo_toml["package"]["version"]
	if VersionInfo.isvalid(argument):
		new_version = argument
	else:
		new_version = bump_version(crate_cargo_toml["package"]["version"], argument)

	crate_cargo_toml["package"]["version"] = new_version

	return new_version

def format_change_list(iter):
	msg = ""
	for iteration, (name, value) in enumerate(iter, 1):
		if not iteration == 1:
			if len(iter) == iteration:
				msg += " and "
			else:
				msg += ", "

		msg += f"{name} to {value}"
	return msg

argument = sys.argv[1]

if argument.startswith("{"):
	arguments = json.loads(argument)
	updated_crates = dict()

	for package in packages:
		argument = arguments.pop(package["name"], "none")

		if argument == "none":
			for depedency in flat_packages[package["name"]]:
				if depedency in updated_crates:
					error = f"'{package['name']}' needs to be updated as '{depedency}' is updated";
					raise ValueError(error)

			continue

		crate_cargo_toml = toml_file_to_dict(package["manifest_path"])
		new_version = update_cargo_toml(crate_cargo_toml, argument)
		updated_crates[package["name"]] = new_version

		for dependency_name, argument in crate_cargo_toml["dependencies"].items():
			if dependency_name in local_pkgs and dependency_name in updated_crates:
				new_version = updated_crates[dependency_name]
				argument["version"] = new_version

		write_toml(package["manifest_path"], crate_cargo_toml)

	changes = ",".join(map(lambda t: f"\"{t[0]}-{t[1]}\"", upgraded_crates.items()))
	updated_cargo_toml_paths = ",".join(map(lambda pkg: f"\"{pkg['manifest_path']}\"", packages))

	with open(environ["GITHUB_OUTPUT"], 'w') as f:
		print(f"new-versions-description={format_change_list(upgraded_crates.items())}", file=f)
		print(f"new-versions=[{changes}]", file=f)
		print(f"updated-cargo-toml-paths=[{updated_cargo_toml_paths}]", file=f)

else:
	if len(packages) == 1:
		crate_cargo_toml = toml_file_to_dict(package["manifest_path"])
		new_version = update_cargo_toml(packages[0]["path"], argument)
		write_toml(package[0]["manifest_path"], crate_cargo_toml)

		with open(environ["GITHUB_OUTPUT"], 'w') as f:
			print(f"new-versions-description={new_version}", file=f)
			print(f"new-versions=[\"{new_version}\"]", file=f)
			print(f"updated-cargo-toml-paths=[\"{packages[0]['name']}\"]", file=f)
	else:
		package_names = list(map(lambda pkg: pkg['name'], packages))
		error = f"Expected single package in workspace for non-JSON '{argument}' argument, found: {package_names}"
		raise ValueError(error)
