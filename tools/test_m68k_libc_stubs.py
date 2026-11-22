"""The libc stubs of the executed m68k sweep (tools/m68k_exec/mem.c) behave and cost like a platform libc's."""
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m68k_clk  # noqa: E402
import m68k_exec_sweep as sweep  # noqa: E402

GCC = os.environ.get("GCC_M68K")
QEMU = shutil.which("qemu-m68k")
CYCLES = os.environ.get("CYCLES", str(HERE.parent / "target/release/examples/cycles"))
needs_tools = pytest.mark.skipif(not (GCC and QEMU and Path(CYCLES).exists()), reason="GCC_M68K, qemu-m68k and the cycles example are needed")

# What mem.c was before: one byte a trip.
BYTE_LOOPS = """
void *memset(void *p, int c, unsigned long n){ unsigned char *q=p; while(n--) *q++=(unsigned char)c; return p; }
void *memcpy(void *d, const void *s, unsigned long n){ unsigned char *a=d; const unsigned char *b=s; while(n--) *a++=*b++; return d; }
void *memmove(void *d, const void *s, unsigned long n){ unsigned char *a=d; const unsigned char *b=s; if(a<b) while(n--) *a++=*b++; else { a+=n; b+=n; while(n--) *--a=*--b; } return d; }
"""

CHECK = """
extern void *memset(void *, int, unsigned long);
extern void *memcpy(void *, const void *, unsigned long);
extern void *memmove(void *, const void *, unsigned long);
static unsigned char got[96], want[96];
static void fill(unsigned char *p) { int i; for (i = 0; i < 96; i++) p[i] = (unsigned char)(i * 7 + 3); }
int main(void)
{
    unsigned long n, at, from;
    for (n = 0; n <= 40; n++) for (at = 0; at < 4; at++) {
        unsigned long i;
        fill(got); fill(want);
        memset(got + at, 0x5a, n);
        for (i = 0; i < n; i++) want[at + i] = 0x5a;
        for (i = 0; i < 96; i++) if (got[i] != want[i]) return 1;
        for (from = 0; from < 4; from++) {
            fill(got); fill(want);
            memcpy(got + 48 + at, got + from, n);
            for (i = 0; i < n; i++) want[48 + at + i] = want[from + i];
            for (i = 0; i < 96; i++) if (got[i] != want[i]) return 2;
        }
        for (from = 0; from < 8; from++) {
            fill(got); fill(want);
            memmove(got + 8 + at, got + from + 4, n);
            if (at + 8 < from + 4) for (i = 0; i < n; i++) want[8 + at + i] = want[from + 4 + i];
            else for (i = n; i > 0; i--) want[8 + at + i - 1] = want[from + 4 + i - 1];
            for (i = 0; i < 96; i++) if (got[i] != want[i]) return 3;
        }
    }
    return 0;
}
"""

COST = """
extern void *memset(void *, int, unsigned long);
extern void *memcpy(void *, const void *, unsigned long);
static unsigned char a[1024], b[1024];
int main(void) { memset(a, 7, 1024); memcpy(b, a, 1024); return b[1023] != 7; }
"""


def build(tmp_path, stub_source, program, cpu="68000"):
    """The program linked with the stub source and the sweep's crt, as the sweep links them."""
    tmp_path.mkdir(parents=True, exist_ok=True)
    gcc = f"{GCC}/usr/bin/m68k-linux-gnu-gcc"
    flags = [f"-m{cpu}", "-msoft-float", "-O1", "-ffreestanding", "-fno-builtin", "-fno-tree-loop-distribute-patterns", "-w"]
    objects = []
    for name, text in (("crt", (sweep.STUBS / "crt.c").read_text()), ("mem", stub_source), ("prog", program)):
        (tmp_path / f"{name}.c").write_text(text)
        done = subprocess.run([gcc, *flags, "-c", "-o", str(tmp_path / f"{name}.o"), str(tmp_path / f"{name}.c")], capture_output=True, text=True, env=sweep.tool_env(GCC))
        assert done.returncode == 0, done.stderr
        objects.append(str(tmp_path / f"{name}.o"))
    exe = tmp_path / "t.exe"
    done = subprocess.run([gcc, f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-o", str(exe), *objects], capture_output=True, text=True, env=sweep.tool_env(GCC))
    assert done.returncode == 0, done.stderr
    return exe


def priced(exe, cpu="68000"):
    """(instructions, clocks) of the program under the traced qemu. The clocks cell is a bound (`N*`: the stubs' loops have no known count)."""
    run = subprocess.run([CYCLES, "--exe", str(exe)], env=m68k_clk.cycles_env(cpu, GCC), capture_output=True, text=True, timeout=900)
    for line in run.stdout.splitlines():
        if line.startswith("| ") and ".exe" in line:
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            return int(cells[1].rstrip("*")), int(cells[2].rstrip("*"))
    raise AssertionError(run.stdout + run.stderr)


@needs_tools
@pytest.mark.parametrize("cpu", ["68000", "68020"])
def test_the_stubs_fill_copy_and_move_what_a_byte_loop_does_at_every_length_and_alignment(tmp_path, cpu):
    """A long-wise stub that mishandled an odd address or an overlap would corrupt every program that calls it; the byte stubs could not."""
    exe = build(tmp_path, (sweep.STUBS / "mem.c").read_text(), CHECK, cpu)
    run = subprocess.run(["qemu-m68k", "-cpu", f"m{cpu}", str(exe)], capture_output=True, timeout=120)
    assert run.returncode == 0, f"exit {run.returncode}: 1 memset, 2 memcpy, 3 memmove"


@needs_tools
def test_a_kilobyte_fill_and_copy_cost_a_quarter_of_the_byte_stubs(tmp_path):
    """Byte-loop stubs priced every memset and memcpy call at four times a platform libc's: mem_length_boundaries_m32 called the stub
    for 20000 bytes and its clocks did not move against gcc's inline long loops (4,242,970 -> 4,243,306)."""
    byte = priced(build(tmp_path / "byte", BYTE_LOOPS, COST))
    longs = priced(build(tmp_path / "long", (sweep.STUBS / "mem.c").read_text(), COST))
    assert longs[0] * 3 < byte[0], f"long {longs} against byte {byte}"
    assert longs[1] * 2.5 < byte[1], f"long {longs} against byte {byte}"


def test_the_cache_follows_the_stub_sources(tmp_path, monkeypatch):
    """The cache directory ignored tools/m68k_exec: an edited stub was served from the objects of the old one."""
    stubs = tmp_path / "m68k_exec"
    stubs.mkdir()
    (stubs / "mem.c").write_text("one")
    cc = tmp_path / "bin" / "llrm-cc"
    cc.parent.mkdir()
    cc.write_text("")
    monkeypatch.setattr(sweep, "STUBS", stubs)
    monkeypatch.setenv("M68K_EXEC_CACHE", str(tmp_path / "cache"))
    before = sweep.cache_dir(str(cc))
    (stubs / "mem.c").write_text("two")
    assert sweep.cache_dir(str(cc)) != before
