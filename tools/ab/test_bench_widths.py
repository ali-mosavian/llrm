"""A bench that assumes a 32-bit `long` is a different program at LP64, so no x86_64 or arm64 external column compares like with like."""
import re
from pathlib import Path

BENCH = Path(__file__).resolve().parents[2] / "bench"
LONG = r"\b(?:unsigned\s+)?long\b(?!\s+long)"
# `typedef long sfi32;` and a union that casts a float through a `long`.
ASSUMES_32 = (
    re.compile(rf"typedef\s+{LONG}\s+\w*(?:32)\b"),
    re.compile(rf"union\s*\{{[^}}]*(?:{LONG}[^}}]*\bfloat\b|\bfloat\b[^}}]*{LONG})[^}}]*\}}"),
)


def assuming_32_bit_long(text: str) -> list[str]:
    return [found.group(0)[:60] for pattern in ASSUMES_32 for found in pattern.finditer(text)]


def test_the_patterns_see_a_long_that_stands_for_32_bits():
    assert assuming_32_bit_long("typedef unsigned long sfu32;")
    assert assuming_32_bit_long("union { unsigned long u; float f; } x;")
    assert assuming_32_bit_long("union { float f; long b; } x;")
    assert not assuming_32_bit_long("typedef __UINT32_TYPE__ sfu32; typedef unsigned long long sfu64;")
    assert not assuming_32_bit_long("union { unsigned long long u; double f; } x;")


def test_no_bench_stands_a_32_bit_value_for_a_long():
    """sfm16_dbg.c typedef'd `sfu32` as `unsigned long`: 64 bits at x86_64, so gcc and ours compiled a different soft float than m32 (the +243 B Os card)."""
    found = {path.name: hit for path in sorted(BENCH.glob("*/*.c")) if (hit := assuming_32_bit_long(path.read_text()))}
    assert not found, found
