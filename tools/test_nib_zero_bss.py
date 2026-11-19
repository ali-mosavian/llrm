"""A Nib array of zeros is uninitialised data (`_BSS`), as the same array is in C: `getKindForGlobal` makes a variable of zeros BSS whatever the
frontend spelled (`[0] * N` is `zeroinitializer`). Before, the frontend's data layout stored N zero bytes in the object (bench histo, scroll,
grep, jpeg_fixed ...: data 4608 / 8000 / 3157 / 16518 B where the C twin had them in bss).

Needs llrm-nib (tools/llrmbin.py finds the built binaries).
"""

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "tools"))
import llrmbin  # noqa: E402
import sizes  # noqa: E402

BIN = llrmbin.bin_dir()

SOURCE = """var zeros: u8[4096] = [0] * 4096
var filled: u8[64] = [7] * 64
var wide: i16[256] = [0] * 256

fn bump(seed: i16) -> i32:
    let mut sum: i32 = 0
    for i in 0..64:
        zeros[i] += u8(seed)
        filled[i] += u8(seed)
        wide[i] += seed
        sum += i32(zeros[i]) + i32(filled[i]) + i32(wide[i])
    return sum

fn main() -> i16:
    print(bump(3))
    print(bump(5))
    return 0
"""


def build(tmp_path: Path) -> tuple[int, int]:
    source, obj = tmp_path / "z.nib", tmp_path / "z.obj"
    source.write_text(SOURCE)
    run = subprocess.run([str(BIN / "llrm-nib"), str(source), "-o", str(obj), "-O2", "-m16", "-march=i386", "--procedure-segments"], capture_output=True, text=True)
    assert run.returncode == 0, run.stderr
    return sizes.data_bytes(obj)


def test_zero_arrays_are_bss_and_a_filled_array_is_data(tmp_path):
    data, bss = build(tmp_path)
    assert bss >= 4096 + 512, f"the zero arrays are stored: data {data}, bss {bss}"
    assert data < 4096, f"the zero arrays are stored: data {data}, bss {bss}"
    assert data >= 64, f"the array of sevens is not data: data {data}"
