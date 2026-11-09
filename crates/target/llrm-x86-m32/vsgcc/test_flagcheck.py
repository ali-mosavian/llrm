"""vsgcc flag parity: a flag on one side only is refused, not measured (arm64 shipped 24 of 72 cells off by 4..16 B that way)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import flagcheck as f  # noqa: E402
import external_flags  # noqa: E402

BASE = [*external_flags.flags("x86", ("-fno-pic",)), "-m32", "-march=i486", "-Dfar=", "-O1"]
GCC = [*BASE, "-mpreferred-stack-boundary=2"]
CLANG = [*BASE, "-mstack-alignment=4"]
# What llrm-cc's clang ran for an m32 -O1 cell (logged through a clang wrapper, 2026-11-09, x_fir).
OURS = ("-target i486-unknown-linux-gnu -fhosted -fshort-wchar -mlong-double-64 -fno-pic -fno-stack-protector -fno-asynchronous-unwind-tables "
        "-std=gnu99 -Xclang -fdefault-calling-conv=regcall -fdeclspec -Wno-unknown-attributes -nostdinc -isystem /h -idirafter /i "
        "-mlong-double-80 -O1 -march=i486 -funroll-loops -S -emit-llvm -Xclang -disable-llvm-passes -o /tmp/x.ll x.c")
EXT = [*GCC, "-mlong-double-80"]


def test_the_real_columns_agree():
    assert f.check_external("gcc", "O1", GCC) == ""
    assert f.check_external("clang", "O1", CLANG) == ""


def test_a_flag_on_the_external_only_is_refused():
    assert "ffast-math" in f.check_external("gcc", "O1", GCC + ["-ffast-math"])
    assert "fno-inline" in f.check_external("gcc", "O1", GCC + ["-fno-inline"])


def test_a_wrong_level_or_march_is_refused():
    assert f.check_external("gcc", "O2", GCC)
    assert f.check_external("gcc", "O1", [t for t in GCC if t != "-march=i486"])


def test_ours_and_an_external_agree_at_o1_where_only_documented_settings_differ():
    assert f.check_ours("O1", OURS, EXT) == ""


def test_ours_with_a_flag_the_external_lacks_is_refused():
    assert "ffast-math" in f.check_ours("O1", OURS + " -ffast-math", EXT)
    assert "funroll-loops" in f.check_ours("O2", OURS, EXT), "-funroll-loops is documented at -O1 only"
