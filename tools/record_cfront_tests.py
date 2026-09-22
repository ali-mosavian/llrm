"""Record the wccq streams the Rust ports of tests/test_cfront_frontend.py read.

    uv run python tools/record_cfront_tests.py

Each source is the one its Python test compiles; the stream lands in
fixtures/c/tests/<test name>.cgs.
"""

import tempfile
from pathlib import Path

from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
BORLAND_H = Path.home() / "work" / "other" / "d32x" / "toolchains" / "bcpp31" / "INCLUDE"

SOURCES = {
    "test_aggregate_copy_through_pointers_reaches_mir": (
        "typedef struct { float x, y, z; } Vec;\nvoid copy(Vec *a, Vec *b, Vec *src) { *a = *b = *src; }\n"
    ),
    "test_aggregate_argument_is_pushed_by_value": (
        "typedef struct { float x, y, z; } Vec;\nextern void take(Vec v);\nvoid pass(Vec *src) { take(*src); }\n"
    ),
    "test_restrict_reaches_mir_as_distinct_noalias_roots": (
        "void add(int *__restrict out, const int *__restrict a, const int *__restrict b) {\n  *out = *a + *b;\n}\n"
    ),
    "test_standard_allocator_return_has_fresh_object_identity": (
        "extern void *malloc(unsigned n);\nint *make(void) { return (int *)malloc(8); }\n"
    ),
}


def main() -> None:
    out = ROOT / "fixtures" / "c" / "tests"
    out.mkdir(parents=True, exist_ok=True)
    for name, source in SOURCES.items():
        with tempfile.TemporaryDirectory() as scratch:
            unit = Path(scratch) / "unit.c"
            unit.write_text(source)
            (out / f"{name}.cgs").write_text(cfront.recorded(unit, [str(BORLAND_H)]))


if __name__ == "__main__":
    main()
