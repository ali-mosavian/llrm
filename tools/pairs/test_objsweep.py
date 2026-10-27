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


def _stub(tmp_path, name, elf_reloc):
    """A compiler whose primary object is fixed and whose ELF writer result carries `elf_reloc` as its relocation type."""
    stub = tmp_path / name
    stub.write_text(
        '#!/bin/sh\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && { printf "obj" > "$a"; next=; };; esac; done\n'
        f'[ -n "$LLRM_OBJECT_ALL_WRITERS" ] && printf "{elf_reloc}" > "$LLRM_OBJECT_ALL_WRITERS/elf.bin"\nexit 0\n'
    )
    stub.chmod(0o755)
    return str(stub)


def test_a_reloc_type_changed_in_a_writer_the_target_does_not_choose_is_reported(tmp_path):
    """`R_386_PLT32` -> `PC32` in the ELF writer left the OMF object identical, so a sweep of the default format read 0 changed."""
    (tmp_path / "x.c").write_text("")
    old, new = _stub(tmp_path, "old", "R_386_PLT32"), _stub(tmp_path, "new", "R_386_PC32")
    args = type("A", (), {"old": old, "new": new, "targets": "m32", "levels": "O2", "jobs": 1})()
    assert objsweep.sweep(args, [tmp_path / "x.c"], "old") == 1


def test_identical_writers_are_not_a_change(tmp_path):
    (tmp_path / "x.c").write_text("")
    old, new = _stub(tmp_path, "old", "R_386_PLT32"), _stub(tmp_path, "new", "R_386_PLT32")
    args = type("A", (), {"old": old, "new": new, "targets": "m32", "levels": "O2", "jobs": 1})()
    assert objsweep.sweep(args, [tmp_path / "x.c"], "old") == 0
