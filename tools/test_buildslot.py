"""Every cargo that parity_smoke and prepush spawn runs under buildslot; BUILDSLOT_BYPASS=1 is the landing script's way out."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "gate"))
import buildslot as bs  # noqa: E402
import parity_smoke as ps  # noqa: E402
import prepush  # noqa: E402


def test_a_cargo_argv_or_shell_string_is_wrapped_and_anything_else_is_not(monkeypatch):
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    tool = bs.tool()
    assert bs.slotted(["cargo", "test", "-q"]) == [tool, "cargo", "test", "-q"]
    assert bs.slotted(["git", "status"]) == ["git", "status"]
    assert bs.slotted("cargo fmt --check && cargo test -q") == f"{tool} cargo fmt --check && {tool} cargo test -q"
    assert bs.slotted("python3 x.py") == "python3 x.py"
    monkeypatch.setenv("BUILDSLOT_BYPASS", "1")
    assert bs.slotted(["cargo", "test"]) == ["cargo", "test"] and bs.slotted("cargo test") == "cargo test"


def test_the_cargo_commands_parity_smoke_spawns_are_all_wrapped(monkeypatch, tmp_path):
    """The smoke ran `cargo check --workspace --all-targets` and the ratchets, unslotted, from several sessions at once: the host ran out of cores."""
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    monkeypatch.setattr(ps.subprocess, "run", lambda command, *a, **k: (seen.append(command), Done())[1])
    ps.workspace_check(tmp_path)
    assert ps.ratchets(root=tmp_path) == ""
    assert seen and all(Path(c[0]).name == "buildslot" and c[1] == "cargo" for c in seen), [c[:3] for c in seen]


def test_prepush_runs_its_cargo_steps_under_buildslot(monkeypatch):
    monkeypatch.delenv("BUILDSLOT_BYPASS", raising=False)
    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    monkeypatch.setattr(prepush.subprocess, "run", lambda command, *a, **k: (seen.append(command), Done())[1])
    prepush.run_all({"test x": "cargo test --release -q --test x", "fmt": "python3 tools/fmt.sh"}, {}, jobs=1)
    assert any(isinstance(c, str) and "buildslot cargo test" in c for c in seen) and "python3 tools/fmt.sh" in seen, seen
