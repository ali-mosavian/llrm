"""A program tagged `// targets: dos` read as FAIL on z80, wasm and m68k: three harnesses kept their own lists of the programs
they cannot build (far, huge, watcom, regparm, fs/gs, os_*), and each cell counted as a hole."""
import subprocess
import sys
from pathlib import Path

import corpus_targets as ct

HERE = Path(__file__).resolve().parent
RUN = HERE.parent / "tests/run/c"


def test_a_dos_program_is_not_applicable_elsewhere():
    source = "// flags: -O2 -mabi=watcom\n// targets: dos\nint main(void){return 0;}\n"
    assert ct.applies("x86_16-wat", source) and ct.applies("x86_32-dos", source)
    assert not any(ct.applies(t, source) for t in ("z80", "wasm32", "m68k-68020", "x86_64-linux", "6502-sim65", "arm64-linux"))
    assert ct.reason(source) == "n/a, DOS-only"


def test_an_untagged_program_is_for_every_target():
    assert ct.applies("wasm32", "int main(void){return 0;}\n")


def test_a_family_and_a_full_name_both_select():
    source = "// targets: x86-m16 wasm32\n"
    assert ct.applies("x86_16-dos", source) and ct.applies("wasm32", source) and not ct.applies("x86_32-dos", source)


def test_the_command_prints_na_and_exits_3_not_a_failure(tmp_path):
    program = tmp_path / "p.c"
    program.write_text("// targets: dos\n")
    done = subprocess.run([sys.executable, str(HERE / "corpus_targets.py"), "check", "z80", str(program)], capture_output=True, text=True)
    assert (done.returncode, done.stdout.strip()) == (3, "n/a, DOS-only")


def test_the_shipped_dos_programs_are_tagged():
    for name in ("watcom_far_sum", "huge_fill", "regparm3_matrix_4", "fsgs_call", "os_screen", "far_return"):
        assert not ct.applies("z80", (RUN / f"{name}.c").read_text()), name
