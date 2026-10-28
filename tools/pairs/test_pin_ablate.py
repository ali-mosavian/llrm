import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pin
import pin_ablate

MAIN = """target datalayout = "e-p:16:16-i32:16"
@blk = internal global [4 x i8] zeroinitializer
declare void @fill(ptr)
define internal cc 1000 void @__main() {
  call void @fill(ptr @blk)
  %v = load i16, ptr @blk
  ret void
}
"""


def test_a_qb_entry_without_a_caller_stays_a_root_for_the_pin():
    """`__main` is internal and uncalled in the QB stage IRs; the pin dropped it and the program body
    (bench/stft_fixed read 34 instructions as 9), so every QB ablation count was measured on dead code."""
    assert "define internal" not in pin_ablate.pin_text_of(MAIN)
    assert "@__main" in pin_ablate.pin_text_of(MAIN)


def test_the_pin_keeps_the_entry_it_is_handed():
    if not shutil.which("opt") and not Path("/opt/llrm-pin/bin/opt").exists():
        return
    done = pin.opt("default<O2>", pin_ablate.pin_text_of(MAIN), pin.target("m16"))
    assert done.returncode == 0, done.stderr
    assert "@fill" in done.stdout.split("define", 1)[1]
