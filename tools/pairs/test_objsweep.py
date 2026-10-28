"""objsweep's split of the changed objects: larger, smaller and same-size-different-bytes, per target and level."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import objsweep


def test_changed_objects_split_per_target_and_level_with_the_largest_growths():
    """A sweep printed 110 changed objects and no split: the reader hand-parsed the list and got it wrong."""
    changed = [("a m32 -O1", 100, 90), ("b m32 -O1", 100, 130), ("c m16 -Os", 50, 50), ("d m16 -Os", 10, 12), ("e m32 -O2", 7, 8)]
    lines = objsweep.summary(changed, top=2)
    assert "m32 -O1: larger 1, smaller 1, same-size 0" in lines
    assert "m16 -Os: larger 1, smaller 0, same-size 1" in lines
    assert lines.index("largest growths:") < lines.index("  b m32 -O1: +30")
    assert "  d m16 -Os: +2" in lines and "  e m32 -O2: +1" not in lines


"""objsweep took the old selector by default: a DAG change read 0 of 568 objects changed where `--isel=dag` showed 38 of 284."""
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import objsweep  # noqa: E402


def test_a_diff_touching_the_dag_is_swept_under_both_selectors_when_none_is_named():
    assert objsweep.selectors(None, ["crates/codegen/llrm-codegen/src/backend/selectiondag/dag_function.rs"]) == ["old", "dag"]


def test_a_diff_touching_a_machine_pass_or_an_x86_action_is_swept_under_both():
    assert objsweep.selectors(None, ["crates/codegen/llrm-codegen/src/backend/peephole.rs"]) == ["old", "dag"]
    assert objsweep.selectors(None, ["crates/target/llrm-x86/src/select.rs"]) == ["old", "dag"]


def test_a_diff_that_touches_none_keeps_the_old_selector_alone():
    assert objsweep.selectors(None, ["docs/working/x.md", "tools/pairs/pair.sh"]) == ["old"]


def test_an_unreadable_diff_is_refused_when_no_selector_is_named():
    with pytest.raises(SystemExit):
        objsweep.selectors(None, None)


def test_a_named_selector_is_kept_and_both_means_old_and_dag():
    assert objsweep.selectors("dag", None) == ["dag"]
    assert objsweep.selectors("old", ["crates/codegen/llrm-codegen/src/backend/selectiondag.rs"]) == ["old"]
    assert objsweep.selectors("both", None) == ["old", "dag"]


def test_the_selector_reaches_the_compile(tmp_path):
    stub = tmp_path / "llrm-c"
    stub.write_text('#!/bin/sh\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && { printf "%s" "$*" > "$a"; next=; };; esac; done; exit 0\n')
    stub.chmod(0o755)
    data, why = objsweep.compile_one(str(stub), tmp_path / "x.c", "m32", "O2", tmp_path / "x.o", "dag")
    assert why == "" and b"--isel=dag" in data
