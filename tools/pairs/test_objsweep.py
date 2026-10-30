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


def test_the_debug_leg_compiles_with_each_debug_format_of_the_target(tmp_path):
    """The sweep never passed -g, so a changed DW_FORM byte read 0 objects changed."""
    (tmp_path / "x.c").write_text("")
    log = tmp_path / "log"
    stub = tmp_path / "cc"
    stub.write_text(f'#!/bin/sh\necho "$*" >> {log}\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && {{ printf obj > "$a"; next=; }};; esac; done\n')
    stub.chmod(0o755)
    args = type("A", (), {"old": str(stub), "new": str(stub), "targets": "m16", "levels": "O2", "jobs": 1, "debug": True})()
    assert objsweep.sweep(args, [tmp_path / "x.c"], "old") == 0
    flags = {flag for line in log.read_text().splitlines() for flag in line.split() if flag.startswith("-g")}
    assert flags == {"-g", "-gtd", "-gcodeview", "-gdwarf-5"}


def test_a_changed_debug_byte_is_a_changed_object(tmp_path):
    (tmp_path / "x.c").write_text("")

    def cc(name, byte):
        stub = tmp_path / name
        stub.write_text(f'#!/bin/sh\ng=0\nfor a in "$@"; do case $a in -g*) g=1;; esac; done\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && {{ [ $g = 1 ] && printf {byte} > "$a" || printf obj > "$a"; next=; }};; esac; done\nexit 0\n')
        stub.chmod(0o755)
        return str(stub)

    args = type("A", (), {"old": cc("old", "A"), "new": cc("new", "B"), "targets": "m16", "levels": "O2", "jobs": 1, "debug": True})()
    assert objsweep.sweep(args, [tmp_path / "x.c"], "old") == 1


def test_an_llrm_cc_is_asked_for_an_object_and_given_its_target(tmp_path, monkeypatch):
    """objsweep ran llrm-cc as it ran llrm-c: m64 linked ("ld returned 1") and m32/m16 had no LLRM_CC_TARGET, so all 852 compiles failed on both sides and the sweep read "0 objects changed"."""
    stub = tmp_path / "llrm-cc"
    stub.write_text('#!/bin/sh\nfor a in "$@"; do case $a in -o) next=1;; *) [ -n "$next" ] && { printf "%s|%s|%s" "$*" "$LLRM_CC_TARGET" "$LLRM_CLANG_M16" > "$a"; next=; };; esac; done; exit 0\n')
    stub.chmod(0o755)
    monkeypatch.setenv("LLRM_CLANG_M16", "m16clang")
    monkeypatch.setenv("LLRM_CC_TARGET", "inherited")
    seen = {target: objsweep.compile_one(str(stub), tmp_path / "x.c", target, "O2", tmp_path / "x.o")[0].decode() for target in ("m64", "m32", "m16")}
    assert all(" -c " in text.split("|")[0] + " " for text in seen.values()), seen
    assert [text.split("|")[1:] for text in seen.values()] == [["", ""], ["x86_32-wat", ""], ["x86_16-wat", "m16clang"]], seen
