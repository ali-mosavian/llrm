"""m64 externals are built -fPIE like ours (flagless PIE): the relocation kinds of the probe must be ours, and -fno-pic must differ.

-fno-pic gave gcc/clang absolute `R_X86_64_32S` data references against our pc-relative ones and faked a +10.6% hanoi2 gap."""
import os
import re
import shutil
import sys
import subprocess
from pathlib import Path

import pytest

import external_flags

PROBE = "extern int use(int);\nstatic int table[8] = {1, 2, 3};\nint f(int n) { return use(table[n & 7]) + table[1]; }\n"
LLRM_CC = os.environ.get("LLRM_CC")  # a dist's llrm-cc; the probe is skipped without one


def kinds(compiler_args, tmp_path, name):
    (tmp_path / "p.c").write_text(PROBE)
    obj = tmp_path / f"{name}.o"
    env = {**os.environ, "LLRM_CC_TARGET": "x86_64-linux"}
    done = subprocess.run([*compiler_args, "-Os", "-c", "-o", str(obj), str(tmp_path / "p.c")], capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr
    dump = subprocess.run(["objdump", "-r", str(obj)], capture_output=True, text=True, check=True).stdout
    return set(re.findall(r"R_X86_64_\w+", dump))


@pytest.mark.skipif(not LLRM_CC, reason="LLRM_CC (a dist's llrm-cc) is not set")
@pytest.mark.parametrize("compiler", [c for c in ("gcc", "clang") if shutil.which(c)])
def test_a_fpie_external_has_the_relocation_kinds_of_ours(compiler, tmp_path):
    ours = kinds([LLRM_CC, "-m64"], tmp_path, "ours")
    theirs = kinds([compiler, *external_flags.flags("x86", ("-fPIE",))], tmp_path, compiler)
    assert theirs == ours, (compiler, theirs, ours)


@pytest.mark.skipif(not LLRM_CC, reason="LLRM_CC (a dist's llrm-cc) is not set")
@pytest.mark.parametrize("compiler", [c for c in ("gcc", "clang") if shutil.which(c)])
def test_the_probe_sees_that_fno_pic_differs(compiler, tmp_path):
    """The control: the probe must fire on the build that gave the false gap, or a match above proves nothing."""
    ours = kinds([LLRM_CC, "-m64"], tmp_path, "ours")
    absolute = kinds([compiler, "-fno-pie", "-no-pie", *external_flags.flags("x86", ("-fno-pic",))], tmp_path, compiler)
    assert absolute != ours, (compiler, absolute, ours)


EXTERN_DATA = "extern int use(int);\nextern int shared[8];\nint f(int n) { return use(shared[n & 7]) + shared[1]; }\n"


@pytest.mark.skipif(not LLRM_CC, reason="LLRM_CC (a dist's llrm-cc) is not set")
@pytest.mark.skipif(not shutil.which("gcc"), reason="no gcc")
def test_the_bench_ours_column_reads_an_external_variable_as_the_fpie_bar_does(tmp_path):
    """#10709: with -fPIE ours reads an external variable rip-relative like gcc (a copy relocation); flagless it went through the GOT, so a column built flagless was not the one
    the bars are compared with. The command is the bench's own: its ours column must have gcc -fPIE's relocation kinds for `extern int shared[8]`."""
    sys.path.insert(0, str(Path(__file__).parent / "bench"))
    import native

    (tmp_path / "e.c").write_text(EXTERN_DATA)
    command, env = native.compile_command(native.OURS, Path(LLRM_CC).parent, tmp_path / "e.c", "Os", tmp_path / "ours.o", tmp_path)
    done = subprocess.run(command, capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr
    dump = lambda obj: set(re.findall(r"R_X86_64_\w+", subprocess.run(["objdump", "-r", str(obj)], capture_output=True, text=True, check=True).stdout))
    gcc = subprocess.run(["gcc", *external_flags.flags("x86", ("-fPIE",)), "-Os", "-c", "-o", str(tmp_path / "gcc.o"), str(tmp_path / "e.c")], capture_output=True, text=True)
    assert gcc.returncode == 0, gcc.stderr
    assert dump(tmp_path / "ours.o") == dump(tmp_path / "gcc.o")
