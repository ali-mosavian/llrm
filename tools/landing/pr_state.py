#!/usr/bin/env python3
"""The runner's PR reads, from GitHub once per cycle instead of once per PR per stage.

  pr_state.py fill N...        one GraphQL call (up to 40 PRs): state, draft, base, mergeable, head, last commit date, checks -> $LAND_TMP/pr-state.json
  pr_state.py cached N FIELD   FIELD of PR N when the fill is younger than PR_STATE_MAX_AGE (default 90 s); nothing otherwise (the caller asks GitHub itself)
  pr_state.py parity           the sha of feat/parity; an If-None-Match request, so an unchanged ref (a 304) costs no quota
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import parity_squash as ps  # noqa: E402

TMP = Path(os.environ.get("LAND_TMP") or "/tmp/landing")
CACHE = TMP / "pr-state.json"
PARITY = TMP / "parity.etag"


def fill(numbers: list[str], run=None, now=time.time) -> None:
    prs = ps.graphql_prs(numbers, run=run)
    TMP.mkdir(parents=True, exist_ok=True)
    CACHE.write_text(json.dumps({"fetched": now(), "prs": prs}))


def cached(number: str, field: str, now=time.time) -> str:
    try:
        data = json.loads(CACHE.read_text())
    except (OSError, ValueError):
        return ""
    if now() - data.get("fetched", 0) > float(os.environ.get("PR_STATE_MAX_AGE", "90")):
        return ""
    value = data["prs"].get(number, {}).get(field, "")
    if isinstance(value, bool):
        return str(value).lower()
    return "" if value is None else str(value)


def parity_sha(run=subprocess.run) -> str:
    repo = os.environ.get("R", "ali-mosavian/llrm")
    try:
        known = json.loads(PARITY.read_text())
    except (OSError, ValueError):
        known = {}
    cmd = ["gh", "api", "-i", f"repos/{repo}/git/ref/heads/feat/parity"] + (["-H", f"If-None-Match: {known['etag']}"] if known.get("etag") else [])
    done = run(cmd, capture_output=True, text=True, errors="replace")
    text = done.stdout
    first = (text.splitlines() or [""])[0]
    if known.get("sha") and (" 304" in first or "HTTP 304" in done.stderr):
        return known["sha"]
    head, _, body = text.partition("\r\n\r\n") if "\r\n\r\n" in text else text.partition("\n\n")
    etag = next((line.split(":", 1)[1].strip() for line in head.splitlines() if line.lower().startswith("etag:")), "")
    try:
        sha = json.loads(body)["object"]["sha"]
    except (ValueError, KeyError):
        return ""
    TMP.mkdir(parents=True, exist_ok=True)
    PARITY.write_text(json.dumps({"etag": etag, "sha": sha}))
    return sha


def main(argv: list[str]) -> int:
    match argv[:1]:
        case ["fill"]:
            fill(argv[1:])
        case ["cached"] if len(argv) == 3:
            print(cached(argv[1], argv[2]), end="")
        case ["parity"]:
            print(parity_sha(), end="")
        case _:
            print(__doc__, file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
