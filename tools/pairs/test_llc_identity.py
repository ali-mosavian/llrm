import stat
import sys
import tempfile
from pathlib import Path

import llc_identity as c

DATALAYOUT_OURS = "e-p:32:32"


def stub(directory: Path, name: str, body: str) -> str:
    """A fake llc: `body` is shell with $out the -o target and $in the input."""
    path = directory / name
    path.write_text('#!/bin/sh\nwhile [ $# -gt 0 ]; do case "$1" in -o) out="$2"; shift;; -S|-O2|-m32) ;; *) in="$1";; esac; shift; done\n' + body + "\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    return str(path)


def inputs(directory: Path, layout: str) -> list[Path]:
    path = directory / "a.ll"
    path.write_text(f'target datalayout = "{layout}"\ndefine void @f() {{ ret void }}\n')
    return [path]


def test_identical_outputs_compare_clean():
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        same = stub(d, "same", 'cat "$in" > "$out"')
        assert c.compare(same, same, [], inputs(d, DATALAYOUT_OURS)) == (1, [], 0)


def test_a_changed_output_is_a_difference():
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        pre, post = stub(d, "pre", 'cat "$in" > "$out"'), stub(d, "post", 'echo changed > "$out"')
        assert c.compare(pre, post, [], inputs(d, DATALAYOUT_OURS))[1] == ["a.ll (rc 0/0)"]


def test_an_input_both_refuse_is_not_a_comparison():
    """The m32 leg of the comdat run: clang's i386 layout is not the compiler's own, so both binaries refused every input with the same text; the script printed 'differing 0' for 124 programs it never compiled."""
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        picky = stub(d, "picky", "grep -q 'datalayout = \"e-p:32:32\"' \"$in\" || { echo not-our-layout >&2; exit 1; }; cat \"$in\" > \"$out\"")
        assert c.compare(picky, picky, [], inputs(d, "e-m:e-p:32:32-S128")) == (0, [], 1)
        assert c.compare(picky, picky, [], inputs(d, "e-m:e-p:32:32-S128"), datalayout=DATALAYOUT_OURS) == (1, [], 0)


def test_an_empty_comparison_exits_two():
    """Nothing compared is not a pass: exit 2, the premise of the run."""
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        refuse = stub(d, "refuse", "exit 1")
        src = inputs(d, DATALAYOUT_OURS)[0]
        argv, sys.argv = sys.argv, ["llc_identity.py", "--pre", refuse, "--post", refuse, str(src)]
        try:
            assert c.main() == 2
        finally:
            sys.argv = argv
