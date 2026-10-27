"""llrm-c writes the object only for an `.o` or `.obj` output; `-o /dev/null` gets the assembly text and the encoder never runs. A tool that checks a compile
succeeded with `-o /dev/null` is blind past isel (QCport's ui/console.c died in the encoder for days under a sweep that read 0 failed)."""
from __future__ import annotations

import re
from pathlib import Path

TOOLS = Path(__file__).resolve().parent

# Tools that time or count the compiler's own work, not a verdict on whether it compiled. They leave out the encoder's cost too; moving them
# to an object moves their baselines, so each is listed with its owner's reason rather than silently.
MEASURING = {
    "test_compile_gap.py": "asserts the gap tool does not print to /dev/null",
    "parity_smoke.py": "compile_ok swaps the null device for an object (test_parity_smoke)",
}
# Not llrm-c: the pin's llc and clang, or a front end that writes its object whatever the name (llrm-qb, llrm-nib).
OTHER_TOOLS = re.compile(r"LLC|llc|clang|gcc|llrm-qb|llrm-nib|\bll\b|perf|valgrind")


def devnull_compiles(path: Path) -> list[int]:
    return [
        number
        for number, line in enumerate(path.read_text().splitlines(), 1)
        if re.search(r"os\.devnull|/dev/null", line)
        and '"-o"' in line
        and '"-S"' not in line
        and "2>" not in line
        and not OTHER_TOOLS.search(line)
        and ("llrm-c" in path.read_text() or "compiler" in path.read_text())
    ]


def test_no_verdict_tool_compiles_to_the_null_device():
    found = {}
    for path in sorted(TOOLS.rglob("*.py")):
        name = path.relative_to(TOOLS).as_posix()
        if name in MEASURING or name == Path(__file__).name:
            continue
        if lines := devnull_compiles(path):
            found[name] = lines
    assert not found, f"compiled to /dev/null (an object path reaches the encoder): {found}"
