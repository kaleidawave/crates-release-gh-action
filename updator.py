from os import environ
from tomlkit import dumps, parse
from glob import glob
from os import path
from semver import bump_major, bump_minor, bump_patch, VersionInfo
import json
import sys

cargo_tomls = [path for path in glob("**/Cargo.toml", recursive=True) if not path.startswith("target")]

upgraded_crates, manifest_cache = dict(), dict()

def toml_to_dict(path, cache):
	if path in cache:
		return cache[path]
	with open(path, "r") as f:
		toml = parse(f.read())
		cache[path]=toml
		f.close()
		return toml

def try_get_name(toml):
	try:
		return toml["package"]["name"]
	except:
		return None

def try_get_version(toml):
	try:
		return toml["package"]["version"]
	except:
		return None

def bump_version(version, behavior):
	match behavior:
		case "patch":
			return bump_patch(version)
		case "minor":
			return bump_minor(version)
		case "major":
			return bump_major(version)
		case _:
			raise ValueError(f"Expected 'patch', 'minor' or 'major'. Found '{behavior}'")

def write(path, toml):
	with open(path, "w") as f:
		f.write(dumps(toml))
		f.close()

def update_cargo_toml(crate_manifest_path, argument, manifest_cache):
	crate_cargo_toml = toml_to_dict(crate_manifest_path, manifest_cache)

	if VersionInfo.isvalid(argument):
		new_version = argument
	else:
		new_version = bump_version(try_get_version(crate_cargo_toml), argument)

	crate_cargo_toml["package"]["version"] = new_version

	write(crate_manifest_path, crate_cargo_toml)

	return crate_cargo_toml, new_version

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
	for crate_name, argument in arguments.items():
		if argument is None:
			continue

		path_finder = (path for path in cargo_tomls if try_get_name(toml_to_dict(path, manifest_cache)) == crate_name)

		try:
			crate_manifest_path = next(path_finder)
		except StopIteration:
			raise ValueError(f"Could not find a crate with name '{crate_name}'") from None

		crate_cargo_toml, new_version = update_cargo_toml(crate_manifest_path, argument, manifest_cache)
		for other_path in cargo_tomls:
			if other_path == crate_manifest_path:
				continue

			other_manifest = toml_to_dict(other_path, manifest_cache)
			depenencies = other_manifest.get("dependencies", {}).values()
			for dependency in filter(lambda d: "path" in d, depenencies):
				crate_manifest_dir = path.dirname(crate_manifest_path)
				relative_path = path.normpath(path.join(crate_manifest_dir, dependency["path"], "Cargo.toml"))
				if relative_path == crate_manifest_path:
					name = try_get_name(other_manifest)
					if arguments.get(name, None) is None:
						parent_name = crate_cargo_toml["package"]["name"]
						raise ValueError(f"'{name}' has updated dependency '{parent_name}' but it itself is not updated")

					dependency["version"] = new_version

					write(other_path, other_manifest)

		upgraded_crates[crate_name] = new_version

	changes = "[" + ",".join(map(lambda t: f"\"{t[0]}-{t[1]}\"", upgraded_crates.items())) + "]"

	with open(environ["GITHUB_OUTPUT"], 'w') as f:
		print(f"new-versions-description={format_change_list(upgraded_crates.items())}", file=f)
		print(f"new-versions={changes}", file=f)

else:
	if len(cargo_tomls) == 1:
		_, new_version = update_cargo_toml(cargo_tomls[0], argument, manifest_cache)
		with open(environ["GITHUB_OUTPUT"], 'w') as f:
			print(f"new-versions-description={new_version}", file=f)
			print(f"new-versions=[\"{new_version}\"]", file=f)
	else:
		raise ValueError(f"Expected single 'Cargo.toml' for non-JSON '{argument}' argument, found: {cargo_tomls}")
