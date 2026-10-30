"""The compile-cost ladder (tools/ladder): the generator's inputs, the fit and the IR count."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import fit  # noqa: E402
import ladder_gen as gen  # noqa: E402
import ladder  # noqa: E402


def clang() -> str:
    found = shutil.which("clang") or str(Path.home() / "scratch/llvm-build/bin/clang")
    if not Path(found).exists():
        pytest.skip("no clang")
    return found


def asm_instructions(shape: str, n: int) -> int:
    done = subprocess.run([clang(), "-w", "-m32", "-O2", "-S", "-x", "c", "-", "-o", "-"], input=gen.generate(shape, n),
                          capture_output=True, text=True, check=True)
    return sum(1 for line in done.stdout.splitlines() if line.startswith("\t") and line[1].isalpha() and not line.startswith("\t."))


@pytest.mark.parametrize("shape", gen.SHAPES)
def test_a_shape_is_c89_and_the_same_text_every_time(shape: str) -> None:
    text = gen.generate(shape, 50)
    assert text == gen.generate(shape, 50)
    assert "for (unsigned" not in text, "wccq takes no C99 declaration in a for"
    assert gen.generate(shape, 100).count("\n") > text.count("\n")


def test_arith_is_not_folded_to_a_constant() -> None:
    """The first draft mixed `&` and `|` with constants and an even multiplier: at N=400 clang -O2 returned one constant (2
    instructions), so a ladder of it measured an empty function on both sides."""
    assert asm_instructions("arith", 400) > 400


@pytest.mark.parametrize("shape", gen.SHAPES)
def test_no_shape_collapses_at_the_top_of_the_ladder(shape: str) -> None:
    assert asm_instructions(shape, 200) > 50


def test_the_fit_recovers_a_fixed_cost_and_a_slope() -> None:
    rows = [{"shape": "s", "n": n, "ir": 10 * n, "opt": int((3 + 0.04 * 10 * n) * 1e6), "llc": 10**6, "clang": 10**6, "llrm": 10**6,
             "llrm_phases": {"optimizer": 1 + 0.1 * 10 * n, "backend": 2.0}} for n in (25, 50, 100, 200, 400)]
    got = fit.fit(rows)["s"]
    fixed, slope, exponent = got["LLVM optimizer"]
    assert abs(fixed - 3) < 0.01 and abs(slope - 40) < 0.1
    assert abs(got["llrm optimizer"][1] - 100) < 0.1
    assert 0.9 < exponent < 1.0


def test_the_ir_count_counts_instructions_and_not_labels_or_comments() -> None:
    text = "define i32 @f() {\nentry:\n  %a = add i32 1, 2\n  ; a comment\n  ret i32 %a\n}\n"
    assert ladder.ir_count(text) == 2
