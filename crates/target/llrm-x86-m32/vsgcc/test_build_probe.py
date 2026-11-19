"""build.sh's long double probe files are per program.

Two builds at once (`xargs -P 2`) shared `$O/b/ld_probe.*`; one read the other's half-written object, saw no size and
refused `hanoi`: the fresh m32 sweep of 2026-11-05 ran 70 of 71 programs.
"""
import re
from pathlib import Path


def test_the_long_double_probe_files_are_named_for_the_program():
    text = (Path(__file__).parent / "build.sh").read_text()
    names = re.findall(r"\$O/b/(ld_probe[^\s\"')]*)", text)
    assert names, "premise: build.sh writes ld_probe files"
    assert all("$P" in name or "${P}" in name for name in names), names
