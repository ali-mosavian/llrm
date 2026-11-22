"""The z80 size-fit tool once took the pin's clang from a default path of one machine, and the PR path scan
did not look for that directory name. It reads the pin through `pin_clang` now."""
import re
from pathlib import Path

SOURCE = Path(__file__).with_name("z80_size_fit.py").read_text()
LEAK = re.compile("|".join(["/ho" + "me/", r"\.cla" + "ude", "scr" + "atch"]))


def test_the_tool_names_no_path_of_one_machine():
    assert not LEAK.findall(SOURCE)


def test_the_tool_finds_the_pin_through_pin_clang():
    assert "pin_clang.path()" in SOURCE
