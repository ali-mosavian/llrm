"""`extload` widens the compare of a loaded byte where the load's extension is made: x_life's neighbour count tests the
byte it loaded in the next block (`icmp ne i8`), and the byte register then had to be extended again.

x_life's m32 -O2 object was 727 bytes without the pass and 711 with it (m16 -O1 668 -> 661, -O2 644 -> 641)."""
import subprocess
from pathlib import Path

import pytest

import harness
import wrap

KERNEL = Path(__file__).parent / "kernels" / "x_life" / "x_life.c"
WITHOUT_THE_PASS = 727  # parity 065b78779a, the same build line


@pytest.mark.skipif(not harness.LLRM.exists(), reason="needs a built llrm-cc")
def test_x_lifes_compare_in_a_dominated_block_is_widened_and_the_object_shrinks(tmp_path):
    source = tmp_path / "x_life.c"
    source.write_text(wrap.wrapped("x_life", KERNEL.read_text()))
    sizes = {}
    for level in ("-O2", "-O3"):
        obj = tmp_path / f"x_life{level}.obj"
        done = subprocess.run(
            [str(harness.LLRM), *harness.llrmbin.c_args(32), "-mabi=sysv", level, "-c", "-o", str(obj), str(source)],
            capture_output=True,
            text=True,
            env=harness.cc_env(),
        )
        assert done.returncode == 0, done.stderr
        sizes[level] = obj.stat().st_size
    assert all(size < WITHOUT_THE_PASS for size in sizes.values()), sizes
