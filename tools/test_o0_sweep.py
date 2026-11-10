"""o0_sweep counts the compiles that succeeded. A stub compiler stands in for llrm-cc: it fails on a source holding REFUSE, writes an object otherwise."""
import stat
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import o0_sweep as sweep  # noqa: E402

STUB = """#!/bin/sh
out=; for a in "$@"; do if [ "$prev" = -o ]; then out=$a; fi; prev=$a; src=$a; done
if grep -q REFUSE "$src"; then echo "llrm-llc: @f: --isel=dag: Emit(Unmapped(\\"JMP32r\\"))" >&2; exit 1; fi
[ "${LLRM_CC_TARGET}" = silent ] && exit 0
echo obj > "$out"
"""


@pytest.fixture
def stub(tmp_path):
    cc = tmp_path / "cc"
    cc.write_text(STUB)
    cc.chmod(cc.stat().st_mode | stat.S_IEXEC)
    return str(cc)


def programs(tmp_path, **bodies):
    paths = []
    for name, body in bodies.items():
        p = tmp_path / f"{name}.c"
        p.write_text(body)
        paths.append(p)
    return paths


def test_refusals_are_counted_with_their_message_and_successes_are_counted(tmp_path, stub):
    progs = programs(tmp_path, good="int main(void){return 0;}\n", bad="/*REFUSE*/ int main(void){return 0;}\n")
    row = sweep.sweep(["x86_32-linux"], progs, stub, 1)["x86_32-linux"]
    assert (row["programs"], row["compiled"], list(row["refused"])) == (2, 1, ["bad"])
    assert "Unmapped" in row["refused"]["bad"]


def test_a_program_that_does_not_apply_is_n_a_not_compiled(tmp_path, stub):
    progs = programs(tmp_path, dos="// targets: dos\nint main(void){return 0;}\n", any="int main(void){return 0;}\n")
    row = sweep.sweep(["x86_64-linux"], progs, stub, 1)["x86_64-linux"]
    assert (row["programs"], row["na"], row["compiled"]) == (1, 1, 1)


def test_exit_zero_without_an_object_is_a_refusal(tmp_path, stub):
    """A compiler that exits 0 and writes nothing read as a clean compile: the sweep counted the exit code, not the object."""
    progs = programs(tmp_path, good="int main(void){return 0;}\n")
    row = sweep.sweep(["silent"], progs, stub, 1)["silent"]
    assert (row["compiled"], len(row["refused"])) == (0, 1)


def test_a_target_with_no_applicable_program_is_red_and_so_is_one_that_compiled_nothing():
    """0 applicable programs is a broken applies filter, 0 compiled a broken compiler: both loud, and told apart."""
    none = sweep.verdict({"z80-sdcc": {"programs": 0, "na": 5, "compiled": 0, "refused": {}}}, set())
    nothing = sweep.verdict({"z80-sdcc": {"programs": 5, "na": 0, "compiled": 0, "refused": {}}}, set())
    assert "0 applicable" in none[0] and "compiled 0 of 5" in nothing[0]


def test_a_baseline_entry_needs_an_owner_an_exit_date_and_the_error_and_expires():
    good = "wasm32 builtins_table | wasm owner | EXIT 2026-11-20 | llvm.is.constant unsupported"
    assert sweep.parse_baseline(good, "2026-11-10") == ({"wasm32 builtins_table"}, [])
    assert sweep.parse_baseline("wasm32 builtins_table\n", "2026-11-10")[1], "no owner, date or error: red"
    assert sweep.parse_baseline(good, "2026-11-21")[1], "past its exit date: red"


def test_a_refusal_outside_the_baseline_is_red_and_one_inside_is_not():
    row = {"programs": 2, "na": 0, "compiled": 1, "refused": {"bad": "m"}}
    assert sweep.verdict({"wasm32": row}, set()) == ["wasm32 bad: m"]
    assert sweep.verdict({"wasm32": row}, {"wasm32 bad"}) == []


def test_only_feature_flags_of_the_first_configuration_are_kept():
    """Dropping -mlong-double-80 made every long_double program a false refusal at m16 (ms_struct layouts)."""
    assert sweep.flags_of("// flags: -O2 -march=i486 -m32 -DREAL_MODE -fno-inline -mlong-double-80 | -O1 -DOTHER\nint x;\n") == [
        "-DREAL_MODE", "-fno-inline", "-mlong-double-80"]


@pytest.mark.skipif(not __import__("os").environ.get("LLRM_O0_SWEEP"), reason="the corpus gate: set LLRM_O0_SWEEP=1 (minutes, every target)")
def test_the_corpus_compiles_at_o0_on_every_target():
    assert sweep.main(["o0_sweep.py"]) == 0


def test_feature_m_flags_are_given_to_x86_targets_only():
    """-mlong-double-80 on m68k, arm64 and wasm is a clang error: a refusal made by the sweep, not by the compiler."""
    source = "// flags: -O0 -mlong-double-80 -DX\nint x;\n"
    assert sweep.flags_of(source, "x86_16-wat") == ["-mlong-double-80", "-DX"]
    assert sweep.flags_of(source, "m68k-68020") == ["-DX"]


def test_the_family_flags_line_is_given_to_that_family_only():
    """memcpy_runtime carries `// flags-m68k: -mcmodel=medium`; the sweep dropped it and m68k refused the program (a pc-relative overflow)."""
    source = "// flags: -O0 -DX\n// flags-m68k: -mcmodel=medium\nint x;\n"
    assert sweep.flags_of(source, "m68k-68020") == ["-DX", "-mcmodel=medium"]
    assert sweep.flags_of(source, "arm64-linux") == ["-DX"]
    real = (sweep.ROOT / "tests/run/c/memcpy_runtime.c").read_text()
    assert "-mcmodel=medium" in sweep.flags_of(real, "m68k-68000")


def test_warning_flags_of_the_program_are_kept():
    """long_double_union_pun needs -Wno-incompatible-ms-struct; dropping it made the m16 -O0 sweep list a clang error (ms_struct layout) as a refusal."""
    assert "-Wno-incompatible-ms-struct" in sweep.flags_of("// flags: -O0 -mlong-double-80 -Wno-incompatible-ms-struct\nint x;\n", "x86_16-wat")


def test_a_program_with_two_flag_configurations_builds_the_one_of_its_targets_word_size():
    """os_execute reads `-DREAL_MODE | -m32`: x86_32-wat took the first, built the 16-bit configuration and was refused for an undeclared `llrm_os_block_resize`."""
    source = "// flags: -O2 -march=i486 -DREAL_MODE | -O2 -march=i486 -m32 -DPROTECTED\nint x;\n"
    assert sweep.flags_of(source, "x86_32-wat") == ["-DPROTECTED"]
    assert sweep.flags_of(source, "x86_16-wat") == ["-DREAL_MODE"]
    assert sweep.flags_of(source, "x86_64-linux") == ["-DREAL_MODE"]
    assert sweep.flags_of(source, "m68k-68020") == ["-DREAL_MODE"], "a target with no word-size flag takes the first"
    assert "-DREAL_MODE" not in sweep.flags_of((sweep.ROOT / "tests/run/c/os_execute.c").read_text(), "x86_32-wat")
