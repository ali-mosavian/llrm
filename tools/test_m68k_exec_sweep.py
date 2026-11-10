"""The executed m68k sweep must not turn an instrument fault into a pass or a failure."""
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m68k_exec_sweep as sweep  # noqa: E402
import o0_sweep  # noqa: E402

GCC = os.environ.get("GCC_M68K")
QEMU = shutil.which("qemu-m68k")


def test_a_program_without_reference_output_is_noref_not_ok():
    """The sweep counted every exit-0 cell with no `.out` as ok: 6 cells per CPU passed with nothing compared."""
    assert sweep.verdict(0, b"anything\n", None)[0] == "noref"


def test_crlf_output_matches_an_lf_reference():
    """os_console writes "ab\\r\\ncd\\r\\n" (the OS layer puts exactly its bytes out); its .out has LF. The sweep read ours at every level
    and both CPUs as a miscompile, where the oracle comparison (corpus_targets.oracle_verdict) ignores line ends."""
    assert sweep.verdict(0, b"ab\r\ncd\r\n", b"ab\ncd\n") == ("ok", "")
    assert sweep.verdict(0, b"ab\r\ncx\r\n", b"ab\ncd\n")[0] == "output"


def test_a_failing_exit_is_run_even_when_stdout_matches():
    assert sweep.verdict(1, b"1\n", b"1\n")[0] == "run"
    assert sweep.verdict(-11, b"", None)[0] == "run"


def test_output_is_compared_with_the_reference():
    assert sweep.verdict(0, b"1\n", b"1\n") == ("ok", "")
    assert sweep.verdict(0, b"2\n", b"1\n")[0] == "output"


def test_x87_only_flags_do_not_reach_the_m68k_compile():
    """long_double_decimal_literal failed to compile at m68k (clang: unsupported option '-mlong-double-80'): the flag is x87's."""
    source = (HERE.parent / "tests/run/c/long_double_decimal_literal.c").read_text()
    assert "-mlong-double-80" not in o0_sweep.flags_of(source, "m68k-68020")


def test_one_flag_configuration_is_used_not_all():
    """idioms at -O2 got the flags of every `|` variant, `-fsanitize=bounds` included, and failed at m68k."""
    source = "// flags: -O2 -march=i486 | -O2 -march=i486 -fsanitize=bounds\n"
    assert "-fsanitize=bounds" not in o0_sweep.flags_of(source, "m68k-68020")


@pytest.mark.skipif(not GCC or not QEMU, reason="needs GCC_M68K and qemu-m68k")
def test_llrm_os_exit_ends_the_program_under_qemu(tmp_path):
    """os.c exited by syscall 248, io_setup on m68k, so llrm_os_exit never ended the program: os_exit timed out in every cell."""
    main = tmp_path / "main.c"
    main.write_text("void report(long);\nvoid llrm_os_exit(unsigned char);\nint main(void){ report(1); llrm_os_exit(0); report(2); return 1; }\n")
    env = {**os.environ, "LD_LIBRARY_PATH": f"{GCC}/usr/lib/x86_64-linux-gnu"}
    exe = tmp_path / "exe"
    built = subprocess.run(
        [f"{GCC}/usr/bin/m68k-linux-gnu-gcc", "-m68020", "-msoft-float", "-O1", "-ffreestanding", "-fno-builtin", "-nostdlib", "-static", "-w",
         "-o", str(exe), str(main), str(sweep.STUBS / "crt.c"), str(sweep.STUBS / "os.c")],
        capture_output=True, text=True, env=env,
    )
    assert built.returncode == 0, built.stderr
    ran = subprocess.run([QEMU, "-cpu", "m68020", str(exe)], capture_output=True, timeout=10)
    assert (ran.returncode, ran.stdout) == (0, b"1\n")
