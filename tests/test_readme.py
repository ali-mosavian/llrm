"""The readme's dot listings are what the compilers print now, less the comments.

    uv run --project tools --with pytest python -m pytest tests/test_readme.py

A listing that moved because the compiler improved is rewritten by `PYTHONPATH=. uv run --project tools python tests/test_readme.py --update`.
"""

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

from tools import linkrecipe, llrmbin

ROOT = Path(__file__).resolve().parents[1]
BIN = llrmbin.bin_dir()
DOT = ROOT / "examples" / "dot"

# (target row, the function's symbol, the backend program it needs)
CASES = [
    ("x86_64-linux", "dot", "llrm-llc"),
    ("arm64-linux", "dot", "llrm-arm64-llc"),
    ("m68k-68020", "dot", "llrm-m68k-llc"),
    ("x86_16-dos", "_dot", "llrm-llc"),
]


def _uncommented(text: str) -> list[str]:
    return [re.sub(r"\s*;.*$", "", line).rstrip() for line in text.splitlines()]


def _renumbered(lines: list[str]) -> list[str]:
    """`lines` with each local label (`L0_27`) named by where it first appears: its number counts the code laid out before it,
    so it moves with an unrelated change that adds or drops an instruction; an instruction that changed still shows."""
    names: dict[str, str] = {}
    return [re.sub(r"\bL\d+_\d+\b", lambda found: names.setdefault(found.group(0), f"L{len(names)}"), line) for line in lines]


def test_local_labels_are_compared_by_where_they_first_appear():
    """Train 3 failed on `L0_27` against `L0_30` in a listing whose instructions had not changed."""
    shown = ["_f proc", "L0_0:", "    jne L0_27", "L0_27:", "    jmp L0_25", "L0_25:"]
    moved = ["_f proc", "L0_0:", "    jne L0_30", "L0_30:", "    jmp L0_23", "L0_23:"]
    assert _renumbered(shown) == _renumbered(moved)
    # An instruction that changed, or a jump that goes to another label, is not hidden by it.
    assert _renumbered(shown) != _renumbered([*moved[:-2], "    jne L0_23", "L0_23:"])
    assert _renumbered(shown) != _renumbered(["_f proc", "L0_0:", "    jne L0_30", "L0_30:", "    jmp L0_30", "L0_23:"])


def _listing(target: str, name: str, out: Path) -> str:
    command = [str(BIN / "llrm-cc"), "-target", target, "-O2", "-S", "-o", str(out), str(DOT / "dot.c")]
    subprocess.run(command, cwd=DOT, check=True, capture_output=True, env={**os.environ, **llrmbin.cc_environment()})
    text = out.read_text().replace("\r", "")
    found = re.search(rf"^{re.escape(name)} proc.*?^{re.escape(name)} endp$", text, re.S | re.M)
    return found.group(0) if found else text.strip("\n")


@pytest.mark.parametrize(("target", "name", "backend"), CASES, ids=[one[0] for one in CASES])
def test_a_readme_listing_is_what_the_compiler_prints(target, name, backend, tmp_path):
    """The readme's first C listing was written by hand, and nobody noticed it stop being the output."""
    env = {**os.environ, **llrmbin.cc_environment()}
    needs = "LLRM_CLANG_M16" if target == "x86_16-dos" else "LLRM_PIN_CLANG"
    if not (BIN / "llrm-cc").exists() or not (BIN / backend).exists() or needs not in env:
        pytest.skip(f"llrm-cc, {backend} or {needs} is missing (cargo build --release --bins)")
    text = (ROOT / "readme.md").read_text()
    found = re.search(rf"`-target {re.escape(target)}`:\n\n```asm\n(.*?)\n```", text, re.S)
    assert found, f"the readme shows no listing for -target {target}"
    now = _listing(target, name, tmp_path / "out.asm")
    assert _renumbered(_uncommented(found.group(1))) == _renumbered(_uncommented(now)), "the listing moved; run PYTHONPATH=. uv run --project tools python tests/test_readme.py --update"


def test_the_readme_source_is_the_example_file():
    """The BASIC source shown was the Frontends block's, and the example kept an older source over a newer listing."""
    text = (ROOT / "readme.md").read_text()
    shown = re.search(r"```c\n(.*?)\n```", text, re.S).group(1)
    assert shown.strip() == (DOT / "dot.c").read_text().replace("\r", "").strip()


def _update() -> None:
    """Rewrites each `-target` listing in the readme to what the compiler prints now."""
    text = (ROOT / "readme.md").read_text()
    for target, name, _backend in CASES:
        with tempfile.TemporaryDirectory() as tmp:
            now = _listing(target, name, Path(tmp) / "out.asm")
        text = re.sub(rf"(`-target {re.escape(target)}`:\n\n```asm\n).*?(\n```)", lambda found: found.group(1) + now + found.group(2), text, count=1, flags=re.S)
    (ROOT / "readme.md").write_text(text)


if __name__ == "__main__":
    if sys.argv[1:] == ["--update"]:
        _update()
