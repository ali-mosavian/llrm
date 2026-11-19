"""m64 externals are built -fPIE like ours (flagless PIE): the relocation kinds of the probe must be ours, and -fno-pic must differ.

-fno-pic gave gcc/clang absolute `R_X86_64_32S` data references against our pc-relative ones and faked a +10.6% hanoi2 gap."""
import os
import re
import shutil
import subprocess

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
