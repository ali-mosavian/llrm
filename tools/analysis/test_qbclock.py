"""The clock bars were built with their own flag list, at m32 against gcc's i686 and clang's pentium4 (cmov, SSE2) while ours is an i486,
and a module llrm-cc refused was compiled by gcc, so 'ours' read as gcc's code. Every column carries external_flags' set; a refusal fails."""

import os
import shutil
import subprocess

HERE = os.path.dirname(__file__)
QBCC = os.path.join(HERE, "qbcc")
REAL_GCC = shutil.which("gcc")


def rig(tmp_path, refuse=False):
    dist, pin, bindir, log = tmp_path / "dist", tmp_path / "pin", tmp_path / "bin", tmp_path / "log"
    for one in (dist, pin, bindir):
        one.mkdir()
    spy = f'#!/bin/sh\ncase " $* " in *" -c "*) echo "$0 $*" >> {log};; esac\nexit 0\n'
    (bindir / "gcc-15").write_text(f'#!/bin/sh\ncase " $* " in *" -c "*) echo "$0 $*" >> {log};; esac\ncase "$1" in -dumpversion) exec {REAL_GCC} "$@";; esac\nexit 0\n')
    (pin / "clang").write_text(spy)
    (dist / "llrm-cc").write_text(spy.replace("exit 0", "echo refused >&2; exit 1") if refuse else spy)
    for one in (bindir / "gcc-15", pin / "clang", dist / "llrm-cc"):
        one.chmod(0o755)
    env = dict(os.environ, QBDIST=str(dist), PIN_BIN=str(pin), PATH=f"{bindir}:" + os.environ["PATH"])
    return env, log


def compile_line(env, arch, variant):
    env = dict(env, QBA=arch, QBV=variant, QBL="Os")
    return subprocess.run([QBCC, "-std=c23", "-O2", "-g", "-Wall", "-c", "x.c", "-o", "x.o"], env=env, capture_output=True, text=True)


def test_every_column_carries_the_shared_flags(tmp_path):
    env, log = rig(tmp_path)
    want = {
        "m64": ["-fomit-frame-pointer", "-fno-stack-protector", "-fcf-protection=none", "-fno-stack-clash-protection", "-fPIE"],
        "m32": ["-fomit-frame-pointer", "-fno-stack-protector", "-fcf-protection=none", "-fno-pic", "-fno-pie", "-march=i486"],
    }
    for arch, flags in want.items():
        log.write_text("")
        for variant in ("gcc", "clang", "ours"):
            assert compile_line(env, arch, variant).returncode == 0
        lines = log.read_text().splitlines()
        assert len(lines) == 3, lines
        for line in lines:
            for flag in flags:
                assert flag in line.split(), f"{arch}: {flag} missing in {line}"


def test_a_refused_module_fails_and_is_not_compiled_by_gcc(tmp_path):
    env, log = rig(tmp_path, refuse=True)
    run = compile_line(env, "m64", "ours")
    assert run.returncode != 0 and "refused" in run.stderr, run.stderr
    assert "bin/gcc-15" not in log.read_text(), "gcc compiled the refused module"


def test_gcc_at_m32_takes_fast_excess_precision(tmp_path):
    """pfpu.c's constexpr doubles do not compile for gcc -m32 under x87 excess precision (the gcc column built nothing)."""
    env, log = rig(tmp_path)
    assert compile_line(env, "m32", "gcc").returncode == 0
    assert "-fexcess-precision=fast" in log.read_text().split()


def test_the_gcc_bar_is_gcc_15_not_the_distro_gcc(tmp_path):
    """`gcc` is the distro's 13.4 here: it left hot helpers out of line and read 2.6x the cycles of ours at m32 Os (arrays.bas)."""
    env, log = rig(tmp_path)
    (tmp_path / "bin/gcc").write_text(f'#!/bin/sh\necho "$0 distro" >> {log}\n')
    (tmp_path / "bin/gcc").chmod(0o755)
    assert compile_line(env, "m64", "gcc").returncode == 0
    assert "bin/gcc-15" in log.read_text() and "distro" not in log.read_text()


def test_the_level_under_test_replaces_the_perf_builds_own(tmp_path):
    """build/hostperf appends -O2 and -g to every compile: a column built at -O2 whatever level was asked."""
    env, log = rig(tmp_path)
    for variant in ("gcc", "clang", "ours"):
        assert compile_line(env, "m64", variant).returncode == 0
    for line in log.read_text().splitlines():
        words = line.split()
        assert "-Os" in words and "-O2" not in words and "-g" not in words, line
