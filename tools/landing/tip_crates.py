#!/usr/bin/env python3
"""The `cargo test` selection for the changed files of a tip: each changed workspace package and every package that depends on it.

usage: tip_crates.py METADATA_JSON < changed-paths   (METADATA_JSON: `cargo metadata --no-deps --format-version 1`)
Prints `-p a -p b ...`, or `--workspace` when a root manifest, the lockfile or a path outside every package changed.
"""
import json
import sys
from pathlib import Path

ROOT_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml"}


def selection(metadata: dict, changed: list[str]) -> str:
    root = Path(metadata["workspace_root"])
    packages = {p["name"]: Path(p["manifest_path"]).parent.relative_to(root).as_posix() for p in metadata["packages"]}
    users: dict[str, set[str]] = {name: set() for name in packages}
    for p in metadata["packages"]:
        for dep in p["dependencies"]:
            if dep["name"] in users:
                users[dep["name"]].add(p["name"])
    hit: set[str] = set()
    for path in changed:
        if path in ROOT_FILES:
            return "--workspace"
        owner = [n for n, d in packages.items() if path == d or path.startswith(d + "/")]
        if owner:
            hit.add(max(owner, key=lambda n: len(packages[n])))
        elif path.startswith(("crates/", "runtime/")) and path.endswith((".rs", ".toml")):
            return "--workspace"
    todo = list(hit)
    while todo:
        for user in users[todo.pop()]:
            if user not in hit:
                hit.add(user)
                todo.append(user)
    return " ".join(f"-p {name}" for name in sorted(hit))


if __name__ == "__main__":
    print(selection(json.load(open(sys.argv[1])), [line.strip() for line in sys.stdin if line.strip()]))
