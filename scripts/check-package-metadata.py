#!/usr/bin/env python3
"""Check metadata inheritance and the workspace's shared release version."""

from pathlib import Path
import sys
import tomllib


root = Path(__file__).resolve().parent.parent
workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
defaults = workspace["package"]
dependencies = workspace["dependencies"]
errors = []
packages = {}

for pattern in workspace["members"]:
    for directory in sorted(root.glob(pattern)):
        manifest = directory / "Cargo.toml"
        data = tomllib.loads(manifest.read_text())
        package = data["package"]
        name = package["name"]
        packages[name] = package
        upstream = package.get("metadata", {}).get("upstream", {})
        for field in defaults:
            # Forks retain upstream authorship, licensing, and edition semantics.
            if upstream and field in ("authors", "license", "edition"):
                continue
            if package.get(field) != {"workspace": True}:
                errors.append(f"{name}: package.{field} must be inherited")
        if not package.get("description", "").strip():
            errors.append(f"{name}: missing description")
        if not (directory / package.get("readme", "README.md")).is_file():
            errors.append(f"{name}: missing README")
        if package.get("publish") is not False and not package.get("documentation"):
            errors.append(f"{name}: missing documentation URL")

        tables = [data, *data.get("target", {}).values()]
        for table in tables:
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                for dep, spec in table.get(kind, {}).items():
                    if not isinstance(spec, dict) or spec.get("workspace") is not True:
                        errors.append(f"{name}: {kind}.{dep} must be inherited")

for name, package in packages.items():
    if package.get("publish") is False:
        continue
    spec = next(
        (spec for key, spec in dependencies.items()
         if isinstance(spec, dict) and spec.get("package", key) == name),
        {},
    )
    if not isinstance(spec, dict) or spec.get("version") != defaults["version"]:
        errors.append(f"{name}: workspace dependency version must match {defaults['version']}")

if errors:
    sys.exit("\n".join(errors))

print(f"Package metadata and release versions agree for {len(packages)} workspace crates.")
