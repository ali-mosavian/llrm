"""`extload` widens the compare of a loaded byte where the load's extension is made: x_life's neighbour count tests the
byte it loaded in the next block (`icmp ne i8`), and the byte register then had to be extended again.

x_life's m32 -O2 object was 727 bytes without the pass and 711 with it (m16 -O1 668 -> 661, -O2 644 -> 641). Object size drifts with
unrelated changes (728 at 17510c4f7d), so the test reads the symptom: no `movzx r32, r8` of a register in the listing."""
import re
import subprocess
from pathlib import Path

import pytest

import harness
import wrap

KERNEL = Path(__file__).parent / "kernels" / "x_life" / "x_life.c"
REEXTENDED = re.compile(r"^\s*movzx\s+e?[a-d]x,\s*[a-d][lh]\s*$", re.M)


def test_the_detector_sees_a_byte_register_extended_again():
    assert REEXTENDED.search("    mov al, byte ptr [esi]\n    movzx eax, al\n")
    assert not REEXTENDED.search("    movzx eax, byte ptr [esi]\n")


@pytest.mark.skipif(not harness.LLRM.exists(), reason="needs a built llrm-cc")
def test_x_lifes_compare_in_a_dominated_block_is_widened_and_no_byte_register_is_extended_again(tmp_path):
    source = tmp_path / "x_life.c"
    source.write_text(wrap.wrapped("x_life", KERNEL.read_text()))
    for level in ("-O2", "-O3"):
        listing = tmp_path / f"x_life{level}.s"
        done = subprocess.run(
            [str(harness.LLRM), *harness.llrmbin.c_args(32), "-mabi=sysv", level, "-S", "-o", str(listing), str(source)],
            capture_output=True,
            text=True,
            env=harness.cc_env(),
        )
        assert done.returncode == 0, done.stderr
        assert not REEXTENDED.search(listing.read_text()), level
