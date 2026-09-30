"""The readme's dot listings are what the compilers print now, less the comments.

    uv run --project tools --with pytest python -m pytest tests/test_readme.py
"""

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "release"
DOT = ROOT / "examples" / "dot"

CASES = [
    ("C", [str(BIN / "llrm-c"), "dot.c", "--cpu", "486"], "_dot"),
    ("Nib", [str(BIN / "llrm-nib"), "dot.nib", "--entry", "dot", "--cpu", "486"], "_dot"),
    ("BASIC", [str(BIN / "llrm-qb"), "dot.bas", "--dialect", "qb45", "--runtime", "qb45", "--cpu", "486"], "DOT"),
]


def _uncommented(text: str) -> list[str]:
    return [re.sub(r"\s*;.*$", "", line).rstrip() for line in text.splitlines()]


def _listing(command: list[str], name: str, out: Path) -> str:
    subprocess.run([*command, "-S", "-o", str(out)], cwd=DOT, check=True, capture_output=True)
    text = out.read_text().replace("\r", "")
    return re.search(rf"^{re.escape(name)} proc.*?^{re.escape(name)} endp$", text, re.S | re.M).group(0)


@pytest.mark.parametrize(("language", "command", "name"), CASES, ids=[one[0] for one in CASES])
def test_a_readme_listing_is_what_the_compiler_prints(language, command, name, tmp_path):
    """The readme's first C listing was written by hand, and nobody noticed it stop being the output."""
    if not Path(command[0]).exists():
        pytest.skip("build the compilers first: cargo build --release --bins")
    blocks = re.findall(r"```asm\n(.*?)\n```", (ROOT / "readme.md").read_text(), re.S)
    shown = blocks[[one[0] for one in CASES].index(language)]
    assert _uncommented(shown) == _uncommented(_listing(command, name, tmp_path / "out.asm"))
