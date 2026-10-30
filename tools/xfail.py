"""The XFAIL list (tools/xfail.toml): tests expected to fail, each with its issue, owner and date.

A listed test that fails is not a new failure; a listed test that passes fails the gate (its fix removes its entry). ids:
`unit <crate>::<test>` (a cargo lib test, by name), `integration <target>::<test>` (a root tests/TARGET.rs test) and `run <program name>` (a tests/run program as run_tests prints it).
"""
from __future__ import annotations

import tomllib
from pathlib import Path

PATH = Path(__file__).resolve().parent / "xfail.toml"
KEYS = ("id", "issue", "owner", "date")


def load(path: Path = PATH) -> dict[str, dict]:
    """id -> entry. An entry lacking a key, with a non-numeric issue, or listed twice is refused: an XFAIL without an owner and an issue is a silenced test."""
    if not path.exists():
        return {}
    found: dict[str, dict] = {}
    for entry in tomllib.loads(path.read_text()).get("xfail", []):
        if missing := [key for key in KEYS if not entry.get(key)]:
            raise SystemExit(f"{path.name}: {entry.get('id', entry)} lacks {', '.join(missing)}")
        if not isinstance(entry["issue"], int):
            raise SystemExit(f"{path.name}: {entry['id']}: issue must be a number")
        if entry["id"] in found:
            raise SystemExit(f"{path.name}: {entry['id']} is listed twice")
        found[entry["id"]] = entry
    return found


def load_for(root: Path) -> dict[str, dict]:
    """The list of the tree under test: `root`/tools/xfail.toml, else the one beside this script. The landing runs from its own checkout, whose list is parity's: a PR that fixes a listed
    test and deletes its row was refused as an xpass because the row it deleted was still read (#4681, #4946, #4936, #4945)."""
    beside = Path(root) / "tools" / "xfail.toml"
    return load(beside if beside.exists() else PATH)


def issue_of(test_id: str, listed: dict[str, dict] | None = None) -> str | None:
    """`#N` for a listed id, else None (run_tests' `known` mark: a listed program that passes is its XPASS)."""
    entry = (load() if listed is None else listed).get(test_id)
    return f"#{entry['issue']}" if entry else None


def classify(failing: set[str], ran: set[str], listed: dict[str, dict]) -> tuple[set[str], set[str], set[str]]:
    """(unexpected failures, expected failures, unexpected passes) of the tests `failing` among those that `ran`. A listed test that did not run says nothing."""
    expected = failing & listed.keys()
    return failing - listed.keys(), expected, {one for one in ran & listed.keys() if one not in failing}
