"""The readme's dot listings are what the compilers print now, less the comments.

    uv run --project tools --with pytest python -m pytest tests/test_readme.py
"""

import os
import re
import subprocess
from pathlib import Path

import pytest

from tools import linkrecipe, llrmbin

ROOT = Path(__file__).resolve().parents[1]
BIN = llrmbin.bin_dir()
DOT = ROOT / "examples" / "dot"

CASES = [
    # C's function is spelled as the default ABI of m16 spells it (`_dot@3`); Nib's own procedures are not decorated.
    ("C", [str(llrmbin.compiler(BIN)), "dot.c", llrmbin.c_args(16)[0], "-march=i486"], linkrecipe.symbol("x86-m16", "dot")),
    ("Nib", [str(BIN / "llrm-nib"), "dot.nib", "--entry", "dot", "-march=i486"], "_dot"),
    ("BASIC", [str(BIN / "llrm-qb"), "dot.bas", "--dialect", "pds71", "--runtime", "qb45", "-march=i486", "-Omax", "--whole-program", "-fno-inline=DOT"], "DOT"),
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


def _listing(command: list[str], name: str, out: Path) -> str:
    subprocess.run([*command, "-S", "-o", str(out)], cwd=DOT, check=True, capture_output=True, env={**os.environ, **llrmbin.cc_environment()})
    text = out.read_text().replace("\r", "")
    return re.search(rf"^{re.escape(name)} proc.*?^{re.escape(name)} endp$", text, re.S | re.M).group(0)


@pytest.mark.parametrize(("language", "command", "name"), CASES, ids=[one[0] for one in CASES])
def test_a_readme_listing_is_what_the_compiler_prints(language, command, name, tmp_path):
    """The readme's first C listing was written by hand, and nobody noticed it stop being the output."""
    if not Path(command[0]).exists():
        pytest.skip(f"{Path(command[0]).name} is not built (cargo build --release --bins)")
    blocks = re.findall(r"```asm\n(.*?)\n```", (ROOT / "readme.md").read_text(), re.S)
    shown = blocks[[one[0] for one in CASES].index(language)]
    assert _renumbered(_uncommented(shown)) == _renumbered(_uncommented(_listing(command, name, tmp_path / "out.asm")))


@pytest.mark.parametrize(("heading", "source"), [("C, `llrm-cc", "dot.c"), ("Nib, `llrm-nib", "dot.nib"), ("BASIC, `llrm-qb", "dot.bas")])
def test_a_readme_source_is_the_example_file(heading, source):
    """The BASIC source shown was the Frontends block's, and the example kept an older source over a newer listing."""
    text = (ROOT / "readme.md").read_text()
    shown = re.search(r"```[a-z]*\n(.*?)\n```", text[text.index(f"\n{heading}") :], re.S).group(1)
    assert shown.strip() == (DOT / source).read_text().replace("\r", "").strip()
