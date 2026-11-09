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
    row = sweep.sweep(["x86_32-linux"], progs, stub, 1)["x86_32-linux"]["O0"]
    assert (row["programs"], row["compiled"], list(row["refused"])) == (2, 1, ["bad"])
    assert "Unmapped" in row["refused"]["bad"]


def test_a_program_that_does_not_apply_is_n_a_not_compiled(tmp_path, stub):
    progs = programs(tmp_path, dos="// targets: dos\nint main(void){return 0;}\n", any="int main(void){return 0;}\n")
    row = sweep.sweep(["x86_64-linux"], progs, stub, 1)["x86_64-linux"]["O0"]
    assert (row["programs"], row["na"], row["compiled"]) == (1, 1, 1)


def test_exit_zero_without_an_object_is_a_refusal(tmp_path, stub):
    """A compiler that exits 0 and writes nothing read as a clean compile: the sweep counted the exit code, not the object."""
    progs = programs(tmp_path, good="int main(void){return 0;}\n")
    row = sweep.sweep(["silent"], progs, stub, 1)["silent"]["O0"]
    assert (row["compiled"], len(row["refused"])) == (0, 1)


def level(programs, compiled, refused=None, applicable=("bad", "good")):
    return {"programs": programs, "na": 0, "compiled": compiled, "refused": refused or {}, "applicable": list(applicable)}


def test_a_target_with_no_applicable_program_is_red_and_so_is_one_that_compiled_nothing():
    """0 applicable programs is a broken applies filter, 0 compiled a broken compiler: both loud, and told apart."""
    none = sweep.verdict({"z80-sdcc": {"O1": level(0, 0)}}, set())
    nothing = sweep.verdict({"z80-sdcc": {"O1": level(5, 0)}}, set())
    assert "0 applicable" in none[0] and "compiled 0 of 5" in nothing[0]
    assert none[0].startswith("z80-sdcc O1:")


def test_a_baseline_entry_needs_an_owner_an_exit_date_and_the_error_and_expires():
    good = "wasm32 O1,O2 builtins_table | wasm owner | EXIT 2026-11-20 | llvm.is.constant unsupported"
    assert sweep.parse_baseline(good, "2026-11-10") == ({("wasm32", "O1", "builtins_table"), ("wasm32", "O2", "builtins_table")}, [])
    assert len(sweep.parse_baseline(good.replace("O1,O2", "all"), "2026-11-10")[0]) == 5
    assert sweep.parse_baseline("wasm32 O1 builtins_table\n", "2026-11-10")[1], "no owner, date or error: red"
    assert sweep.parse_baseline(good.replace("O1,O2", "O9"), "2026-11-10")[1], "an unknown level: red"
    assert sweep.parse_baseline(good.replace("O1,O2 ", ""), "2026-11-10")[1], "the old two-word key has no level: red"
    assert sweep.parse_baseline(good, "2026-11-21")[1], "past its exit date: red"


def test_a_refusal_outside_the_baseline_is_red_and_one_inside_is_not():
    """Removing a row from the baseline turns the sweep red: the ratchet's fail-first. A plain refusal and a Legalize one."""
    row = level(2, 1, {"bad": "m"})
    assert sweep.verdict({"wasm32": {"O2": row}}, set()) == ["wasm32 O2 bad: m"]
    assert sweep.verdict({"wasm32": {"O2": row}}, {("wasm32", "O2", "bad")}) == []
    legal = level(2, 1, {"bad": 'llrm-llc: Legalize("Unsupported { op: Rotr, ty: Simple(I8), what: \\"promote result\\" }")'})
    assert sweep.verdict({"arm64-linux": {"O2": legal}}, set()) == ["arm64-linux O2 bad: Legalize Unsupported Rotr I8 (promote result)"]
    assert sweep.verdict({"arm64-linux": {"O2": legal}}, {("arm64-linux", "O2", "bad")}) == []


def test_a_refusal_at_one_level_is_not_excused_by_a_row_for_another():
    """rotate_small refused at -O2 and -Os on arm64 but compiled at -O0: a row for -O0 must not cover them."""
    row = level(2, 1, {"bad": "m"})
    assert sweep.verdict({"arm64-linux": {"O2": row}}, {("arm64-linux", "O0", "bad")}) == ["arm64-linux O2 bad: m"]


def test_a_baseline_row_that_compiled_or_never_ran_is_red_and_an_unswept_level_is_left_alone():
    """A listed row is a refusal only if the program ran and the compiler refused it; one that compiled or does not apply is stale."""
    fine = level(2, 2)
    assert sweep.verdict({"wasm32": {"O1": fine}}, {("wasm32", "O1", "bad")}) == ["wasm32 O1 bad: in the baseline but compiled: remove the row"]
    assert "never ran" in sweep.verdict({"wasm32": {"O1": fine}}, {("wasm32", "O1", "absent")})[0]
    assert sweep.verdict({"wasm32": {"O1": fine}}, {("wasm32", "O3", "bad")}) == []


def test_only_feature_flags_of_the_first_configuration_are_kept():
    """Dropping -mlong-double-80 made every long_double program a false refusal at m16 (ms_struct layouts)."""
    assert sweep.flags_of("// flags: -O2 -march=i486 -m32 -DREAL_MODE -fno-inline -mlong-double-80 | -O1 -DOTHER\nint x;\n") == [
        "-DREAL_MODE", "-fno-inline", "-mlong-double-80"]


@pytest.mark.skipif(not __import__("os").environ.get("LLRM_O0_SWEEP"), reason="the corpus gate: set LLRM_O0_SWEEP=1 (hours: every target and level)")
def test_the_corpus_compiles_at_every_level_on_every_target():
    assert sweep.main(["o0_sweep.py"]) == 0


def test_feature_m_flags_are_given_to_x86_targets_only():
    """-mlong-double-80 on m68k, arm64 and wasm is a clang error: a refusal made by the sweep, not by the compiler."""
    source = "// flags: -O0 -mlong-double-80 -DX\nint x;\n"
    assert sweep.flags_of(source, "x86_16-wat") == ["-mlong-double-80", "-DX"]
    assert sweep.flags_of(source, "m68k-68020") == ["-DX"]


def test_f_flags_of_the_shared_line_are_given_to_x86_targets_only():
    """int_compared_with_float and tail_calls_edges were listed as refused on arm64 and wasm32 ("llrm-arm64-llc: unrecognized argument -fexcess-precision=standard"): flags the sweep gave, not the compiler's refusal."""
    source = "// flags: -O2 -DX -fexcess-precision=standard -fno-inline=sum5\nint x;\n"
    assert sweep.flags_of(source, "x86_32-linux") == ["-DX", "-fexcess-precision=standard", "-fno-inline=sum5"]
    for target in ("arm64-linux", "wasm32", "m68k-68020", "6502-sim65", "z80-sdcc"):
        assert sweep.flags_of(source, target) == ["-DX"]


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


def test_candidate_rows_merge_the_levels_of_one_refusal_and_keep_owners_to_fill():
    """arm64 rotate_small refused at O1, O2, Os, O3 but not O0: one baseline line `O1,O2,Os,O3`, not four; a line is not an excuse until a person fills OWNER? and EXIT?."""
    refused = {"rotate_small": 'llrm-llc: Legalize("Unsupported { op: Rotr, ty: Simple(I8), what: \\"promote result\\" }")'}
    result = {"arm64-linux": {lv: level(2, 1, dict(refused)) for lv in ("O1", "O2", "Os", "O3")} | {"O0": level(2, 2)}}
    rows = sweep.candidate_rows(result)
    assert rows == ["arm64-linux O1,O2,Os,O3 rotate_small | OWNER? | EXIT? | Legalize Unsupported Rotr I8 (promote result)"]
    assert sweep.parse_baseline(rows[0], "2026-11-10")[1], "an unfilled row is red"
    keys, bad = sweep.parse_baseline(rows[0].replace("OWNER?", "w15-arm64").replace("EXIT?", "EXIT 2026-12-01"), "2026-11-10")
    assert not bad and len(keys) == 4


def test_a_compiler_that_outlasts_the_timeout_is_a_timeout_not_a_refusal(tmp_path, monkeypatch):
    """Under host load 60-90 eight x86_32-wat O1 cells timed out at 180 s and were listed as refusals: untested cells read as compiler faults."""
    slow = tmp_path / "slow-cc"
    slow.write_text("#!/bin/sh\nsleep 5\n")
    slow.chmod(0o755)
    monkeypatch.setattr(sweep, "TIMEOUT", 1)
    program = tmp_path / "p.c"
    program.write_text("int main(void) { return 0; }\n")
    assert sweep.compile_one(str(slow), "x86_32-linux", program, tmp_path)[0] == "timeout"
    result = sweep.sweep(["x86_32-linux"], [program], str(slow), 1, ["O1"])
    row = result["x86_32-linux"]["O1"]
    assert row["refused"] == {} and row["timeouts"] == ["p"] and row["compiled"] == 0
    problems = sweep.verdict(result, set())
    assert "x86_32-linux O1 p: timed out, not tested (not a refusal): rerun it" in problems


def test_a_baseline_row_whose_cell_timed_out_is_neither_compiled_nor_never_ran():
    row = {"programs": 2, "compiled": 1, "refused": {}, "timeouts": ["slow"], "applicable": ["slow", "other"]}
    problems = sweep.verdict({"wasm32": {"O1": row}}, {("wasm32", "O1", "slow")})
    assert problems == ["wasm32 O1 slow: timed out, not tested (not a refusal): rerun it"]
