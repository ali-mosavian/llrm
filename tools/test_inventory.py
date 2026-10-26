#!/usr/bin/env python3
"""Every test target of the workspace, which of them the landing gate runs per batch, and the seconds each unrun one takes.

    cargo test --release --workspace --no-run --message-format=json > build.json
    tools/test_inventory.py build.json [--run] [--markdown OUT.md]

The gate runs `cargo test -p PACKAGE --lib` for the packages a batch touches and their dependants, the root scanners (parity_smoke.SCANNERS, the always-on
ones every batch) and a root test file the batch changes. It runs no other integration target (`tests/*.rs` of any package) and no bin's unit tests; `greedy_inline`
sat red on parity for a day for that. `--run` executes each unrun target once, with the cwd cargo gives it, and records seconds and pass/fail.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke  # noqa: E402


def targets(build: str) -> list[dict]:
    """One row per test executable in cargo's JSON: package, kind (lib, bin, test), name, path, and where its source lives."""
    rows = []
    for line in build.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if message.get("reason") != "compiler-artifact" or not message.get("profile", {}).get("test") or not message.get("executable"):
            continue
        target = message["target"]
        package = message["package_id"].split("#")[-1].split("@")[0] if "#" in message["package_id"] else message["package_id"].split()[0]
        rows.append({"package": target.get("name") if False else package, "kind": target["kind"][0], "name": target["name"], "exe": message["executable"], "src": target["src_path"]})
    return rows


def package_name(row: dict) -> str:
    """The package of a row: the directory under crates/ or the root crate `llrm`."""
    parts = Path(row["src"]).relative_to(ROOT).parts if Path(row["src"]).is_relative_to(ROOT) else ()
    manifest = ROOT
    for depth in range(len(parts) - 1, 0, -1):
        if (ROOT.joinpath(*parts[:depth]) / "Cargo.toml").exists():
            manifest = ROOT.joinpath(*parts[:depth])
            break
    for line in (manifest / "Cargo.toml").read_text().splitlines():
        if line.startswith("name") and "=" in line:
            return line.split("=")[1].strip().strip('"')
    return manifest.name


def gate_runs(row: dict, package: str) -> str:
    """How the per-batch gate treats the target: `lib` (every batch that touches the package or a dependant), `scanner` (always or with lib tests), or `never`."""
    if row["kind"] == "lib":
        return "lib"
    if package == "llrm" and row["kind"] == "test":
        if row["name"] in parity_smoke.NIGHTLY_ONLY:
            return "nightly"
        if row["name"] in parity_smoke.SCANNERS:
            return "scanner"
    return "never"


def run_one(row: dict) -> tuple[float, bool]:
    started = time.monotonic()
    cwd = Path(row["src"]).parents[1] if row["kind"] == "test" else Path(row["src"]).parents[1]
    done = subprocess.run(["nice", "-n", "10", row["exe"], "-q"], cwd=cwd, capture_output=True, text=True)
    return round(time.monotonic() - started, 1), done.returncode == 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("build")
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--skip", default="", help="comma-separated target names not to run (turbo, cv4 and run: nightly only, or covered by the run groups)")
    parser.add_argument("--markdown")
    args = parser.parse_args()
    rows = targets(Path(args.build).read_text())
    seen = set()
    table = []
    for row in rows:
        package = package_name(row)
        key = (package, row["kind"], row["name"])
        if key in seen:
            continue
        seen.add(key)
        how = gate_runs(row, package)
        entry = {"package": package, "kind": row["kind"], "name": row["name"], "gate": how}
        if args.run and how in ("never", "nightly") and row["name"] not in args.skip.split(","):
            entry["seconds"], entry["ok"] = run_one(row)
            print(f"{package}/{row['name']}: {entry['seconds']} s, {'ok' if entry['ok'] else 'RED'}", flush=True)
        table.append(entry)
    if args.run:
        ran = [e for e in table if "ok" in e]
        print(f"inventory: {len(ran)} unrun targets run, {sum(not e['ok'] for e in ran)} red", flush=True)
    out = json.dumps(table, indent=1)
    if args.markdown:
        lines = ["| package | kind | target | gate | seconds | ok |", "|---|---|---|---|---|---|"]
        lines += [f"| {e['package']} | {e['kind']} | {e['name']} | {e['gate']} | {e.get('seconds', '')} | {e.get('ok', '')} |" for e in sorted(table, key=lambda e: (e["gate"], e["package"], e["name"]))]
        Path(args.markdown).write_text("\n".join(lines) + "\n")
    Path(args.build).with_suffix(".inventory.json").write_text(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
