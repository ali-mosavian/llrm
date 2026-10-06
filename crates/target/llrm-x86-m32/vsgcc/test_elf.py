"""llrm's ELF objects, linked by GNU ld and run: alone, and with gcc's in one link."""
import subprocess
import sys
from pathlib import Path

import pytest

import harness

sys.path.insert(0, str(harness.REPO / "tools"))
import linkrecipe  # noqa: E402

EMULATION = linkrecipe.ld_emulation("x86-m32")
LLRM = harness.REPO / "target/release/llrm-c"
STUB = Path(__file__).parent / "stub.s"


def sh(*argv):
    done = subprocess.run([str(a) for a in argv], capture_output=True, text=True)
    assert done.returncode == 0, f"{argv}: {done.stderr}"
    return done.stdout


@pytest.fixture
def work(tmp_path, monkeypatch):
    """A work directory shaped as harness.run reads it, with the stub built."""
    (tmp_path / "b").mkdir()
    sh("gcc", "-m32", "-c", STUB, "-o", tmp_path / "stub.o")
    sh("ld", "-m", EMULATION, "-static", "-e", "0", "-Ttext=0x8000", "-o", tmp_path / "stub.elf", tmp_path / "stub.o")
    monkeypatch.setattr(harness, "OUT", tmp_path)
    return tmp_path


def link(work, name, *objects):
    sh("ld", "-m", EMULATION, "-static", "-e", "0", "-Ttext=0x10000", f"--just-symbols={work / 'stub.elf'}", "-o", work / "b" / f"{name}.elf", *objects)


def llrm_object(work, source, out, *flags):
    sh(LLRM, "-m32", "-march=i486", "-fno-inline-functions", "-fobject-format=elf", *flags, "-o", out, source)
    return out


@pytest.mark.parametrize("prog", ["fib", "sieve", "crc"])
@pytest.mark.parametrize("level", ["O2", "Os"])
def test_a_bench_program_linked_by_ld_prints_its_expected_output(work, prog, level):
    """A bench program's ELF object, linked by GNU ld alone, reports what bench/NAME.out says."""
    object_ = llrm_object(work, harness.BENCH / prog / f"{prog}.c", work / f"{prog}.o", f"-{level}")
    link(work, f"{prog}.llrmElf{level}", object_)
    expected = [int(x) for x in (harness.BENCH / prog / f"{prog}.out").read_text().split()]
    assert harness.run(prog, f"llrmElf{level}")["reports"] == expected


def defined_name(path, base):
    """The symbol `base` is in an object: decorated or not, as the ABI has it."""
    names = sh("nm", "-g", "--defined-only", path).split()
    return next(name for name in names if name.lstrip("_") == base)


def test_an_llrm_object_and_a_gcc_object_link_into_one_program(work):
    """llrm's `bench_mix` calls gcc's `gcc_scale`, and gcc's `main` calls `bench_mix`: one ld link, the stub's report."""
    llrm_c = work / "sum.c"
    llrm_c.write_text("extern int gcc_scale(int);\nint bench_mix(int *a, int n) { int i, s = 0; for (i = 0; i < n; i++) s += gcc_scale(a[i]); return s; }\n")
    first = llrm_object(work, llrm_c, work / "sum.o", "-O2")
    pre = defined_name(first, "bench_mix")[: -len("bench_mix")]
    gcc_c = work / "main.c"
    # gcc names a symbol as the llrm object does, whatever that decoration is
    gcc_c.write_text(
        f'extern int bench_mix(int *, int) __asm__("{pre}bench_mix");\n'
        f'int gcc_scale(int x) __asm__("{pre}gcc_scale");\nint gcc_scale(int x) {{ return x * 3; }}\n'
        'extern void report(int);\nint v[4] = {1, 2, 3, 4};\n'
        f'int main(void) __asm__("{pre}main");\nint main(void) {{ report(bench_mix(v, 4)); return 0; }}\n'
    )
    second = work / "main.o"
    sh("gcc", "-m32", "-march=i486", "-fno-pic", "-fno-stack-protector", "-fcf-protection=none", "-fno-asynchronous-unwind-tables", "-O2", "-c", "-o", second, gcc_c)
    link(work, "mix.gccO2", first, second)
    assert harness.run("mix", "gccO2")["reports"] == [30]
