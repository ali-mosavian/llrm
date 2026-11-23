"""The bars header named cc65 "2.18", the string /usr/bin/cc65 prints; the package is Ubuntu 2.19-2 (sim65-insns comes from the 2.19 source): a reader took 2.18 for a release no bar was built with."""
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import prog_bars  # noqa: E402


def test_the_header_names_the_cc65_package_not_only_its_banner():
    text = " ".join(prog_bars.CAVEATS)
    assert "cc65 2.19-2 (Ubuntu package; self-reports V2.18)" in text, text


def test_the_header_says_the_level_tables_are_per_compiler_not_external_flags():
    text = " ".join(prog_bars.CAVEATS)
    assert "per-compiler level tables" in text and "not tools/external_flags.py" in text, text
