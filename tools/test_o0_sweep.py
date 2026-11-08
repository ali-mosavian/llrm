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


def test_a_target_that_compiled_nothing_is_red_even_with_no_refusals():
    assert sweep.verdict({"z80-sdcc": {"programs": 0, "na": 5, "compiled": 0, "refused": {}}}, set())


def test_a_refusal_outside_the_baseline_is_red_and_one_inside_is_not():
    row = {"programs": 2, "na": 0, "compiled": 1, "refused": {"bad": "m"}}
    assert sweep.verdict({"wasm32": row}, set()) == ["wasm32 bad: m"]
    assert sweep.verdict({"wasm32": row}, {"wasm32 bad"}) == []


def test_only_dash_d_and_dash_f_flags_of_the_first_configuration_are_kept():
    assert sweep.flags_of("// flags: -O2 -march=i486 -DREAL_MODE -fno-inline | -O1 -DOTHER\nint x;\n") == ["-DREAL_MODE", "-fno-inline"]


@pytest.mark.skipif(not __import__("os").environ.get("LLRM_O0_SWEEP"), reason="the corpus gate: set LLRM_O0_SWEEP=1 (minutes, every target)")
def test_the_corpus_compiles_at_o0_on_every_target():
    assert sweep.main(["o0_sweep.py"]) == 0
