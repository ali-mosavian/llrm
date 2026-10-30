"""The kernels that were written for a 32-bit `int` say so, and none is undefined behaviour at m16.

`int` and `unsigned` are 16 bits at m16 and 32 at m32. Twenty-one kernels shifted a 16-bit `unsigned` by 16 or more
(`s >> 16`, `h >> 24`, `1u << 30`) or overflowed an `int`: their m16 builds came out `poison` or `unreachable` and their
objects were noise (a third of the m16 bench coverage). Each now says the width it was written for (`u32`, `i32`).
"""
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

import pytest

import harness

KERNELS = Path(__file__).parent / "kernels"
FIXED = (
    "x_adler x_base64 x_bigadd x_binsearch x_bubble x_crc32 x_fir x_hash x_heapsort x_histo2 x_iir x_insertion "
    "x_isqrt x_mergesort x_popcount x_prefix x_radix x_switch x_switch_dense x_switch_ranges x_switch_sparse"
).split()


def code(name: str) -> str:
    """The kernel's source less comments and the `int main` line."""
    text = re.sub(r"/\*.*?\*/|//[^\n]*", "", (KERNELS / name / f"{name}.c").read_text(), flags=re.S)
    return text.replace("int main", "")


@pytest.mark.parametrize("name", FIXED)
def test_a_width_fixed_kernel_names_no_int_unsigned_or_16_bit_literal(name):
    """A bare `int`, `unsigned` or `1u` is 16 bits at m16: `s >> 16` on a 16-bit `unsigned` is poison (x_adler,
    x_binsearch, x_hash and 17 more were wrong at m16 and right at m32)."""
    text = code(name)
    assert not re.search(r"\bint\b", text), name
    assert not re.search(r"\bunsigned\b(?!\s+(?:char|short|long))", text), name
    assert not re.search(r"\b(?:\d+|0x[0-9A-Fa-f]+)u\b", text), name
    assert "typedef unsigned long u32;" in text and "typedef long i32;" in text, name


def unreachables(text: str) -> int:
    """The `unreachable` instructions outside a switch's `default.unreachable` block: that block is the exhaustive
    switch a table of functions becomes, as many as the unrolling and inlining of a width makes copies of it (x_funcptr:
    three at m16, six at m32), not undefined behaviour. The word in a label or a `switch` line is not counted."""
    count, label = 0, ""
    for line in text.splitlines():
        if re.match(r"^[^\s;][^\s]*:", line):
            label = line.split(":")[0]
        elif line.strip() == "unreachable" and not label.startswith("default.unreachable"):
            count += 1
    return count


def marks(source: Path, *flags: str, tmp: Path) -> tuple[int, int, int]:
    """`unreachable`, `undef` and `poison` in the body once the dead functions are gone (the `globaldce` step): what a
    shift by the width or more leaves. The walk's own dump still holds an inlined static function as an `unreachable`
    stub at m32 (x_dct, x_funcptr, x_heapsort read 1, 6, 1 against 0, 3, 0 at m16 for that alone). The dump goes to the
    work directory: `--dump` refuses a tmpfs for it."""
    harness.OUT.mkdir(parents=True, exist_ok=True)
    dump = Path(tempfile.mkdtemp(prefix="widths-", dir=harness.OUT))
    done = subprocess.run(
        [str(harness.LLRM), *flags, "-O2", "--dump", str(dump), "-c", "-o", str(tmp / "k.obj"), str(source)],
        capture_output=True,
        text=True,
        env=harness.cc_env(16 if "--target=x86_16-wat" in flags else 32),
    )
    assert done.returncode == 0, done.stderr
    text = next(dump.glob("*-globaldce.ll")).read_text()
    shutil.rmtree(dump, ignore_errors=True)
    return unreachables(text), len(re.findall(r"\bundef\b", text)), len(re.findall(r"\bpoison\b", text))


@pytest.mark.skipif(not harness.LLRM.exists() or "LLRM_CLANG_M16" not in os.environ, reason="needs a built llrm-cc and LLRM_CLANG_M16")
@pytest.mark.parametrize("name", sorted(p.name for p in KERNELS.iterdir() if p.is_dir()))
def test_a_kernel_has_no_more_poison_or_unreachable_at_m16_than_at_m32(name, tmp_path):
    """The signature of undefined behaviour at one width: x_hash's `h >> 24` left `unreachable` and a `poison` key at m16
    and a defined hash at m32, so its m16 objects and clocks were noise. A kernel that reads so is refused here."""
    source = KERNELS / name / f"{name}.c"
    at16 = marks(source, *harness.llrmbin.c_args(16), tmp=tmp_path)
    at32 = marks(source, *harness.llrmbin.c_args(32), "-mabi=sysv", tmp=tmp_path)
    assert at16 == at32, f"{name}: (unreachable, undef, poison) {at16} at m16, {at32} at m32"


@pytest.mark.skipif(not harness.LLRM.exists() or "LLRM_CLANG_M16" not in os.environ, reason="needs a built llrm-cc and LLRM_CLANG_M16")
def test_a_shift_by_the_width_still_reads_as_poison_at_m16(tmp_path):
    """The instrument must still see what it exists for: x_hash's `h >> 24` on a 16-bit `unsigned`. Counting only
    non-switch `unreachable` instructions after `globaldce` must not make the signature disappear."""
    source = tmp_path / "shift.c"
    source.write_text("extern void report(long value);\nlong bench_shift(unsigned h) { return h >> 24; }\nint main(void) { report(bench_shift(7)); return 0; }\n")
    assert marks(source, *harness.llrmbin.c_args(16), tmp=tmp_path) != (0, 0, 0)
