import sys
from pathlib import Path
from collections import Counter

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import llrmbin  # noqa: E402
import widths  # noqa: E402

C = "int sum(int n)\n{\n    int total = 0, i = 0;\n    while (i < n) {\n        total = total + i * 3 + (i & 7);\n        i++;\n    }\n    return total;\n}\n"


def nib(width: str) -> str:
    return (
        '@export("cdecl16")\n'
        f"fn sum(n: {width}) -> {width}:\n"
        f"    let mut total: {width} = 0\n"
        f"    let mut i: {width} = 0\n"
        "    while i < n:\n"
        "        total = total + i * 3 + (i & 7)\n"
        "        i += 1\n"
        "    return total\n\n"
        "fn main() -> i16:\n"
        "    print(sum(5))\n"
        "    return 0\n"
    )


def measured(tmp_path: Path, width: str) -> tuple[Counter, Counter]:
    here = llrmbin.target_dir() / "widths-test"  # a dump is thousands of files: not on tmpfs
    here.mkdir(parents=True, exist_ok=True)
    (tmp_path / "sum.nib").write_text(nib(width))
    (tmp_path / "sum.c").write_text(C)
    import tempfile

    with tempfile.TemporaryDirectory(dir=here) as where:
        return widths.pair(tmp_path / "sum.nib", tmp_path / "sum.c", Path(where), llrmbin.bin_dir())


def test_a_nib_program_typed_i32_where_c_has_int_is_flagged(tmp_path):
    """jpeg_fixed typed every int i32 where C's `int` is 16 bits: 80% of its integer operations against C's 20%, 838 bytes larger, and nothing said so."""
    nib_widths, c_widths = measured(tmp_path, "i32")
    assert widths.flagged({"sum": (nib_widths, c_widths)}) != []


def test_a_nib_program_with_c_s_widths_is_not_flagged(tmp_path):
    nib_widths, c_widths = measured(tmp_path, "i16")
    assert widths.flagged({"sum": (nib_widths, c_widths)}) == []


def test_library_and_runtime_functions_do_not_count():
    ir = "define i32 @std.io.read(i32 %a) {\n  %b = add i32 %a, 1\n}\ndefine i16 @f(i16 %a) {\n  %b = add i16 %a, 1\n  %c = add i16 %b, 1\n}\n"
    assert widths.widths(ir) == Counter({"i16": 2})
