"""
Hermetic checks on the fuzz oracle itself: determinism, the trap filter, and
the constant-fold-overflow invariant a real BC compile run caught (see
fuzzgen.py's module docstring) -- so a regression there is a fast local
failure, not a rediscovery three DOSBox launches later.
"""

from collections.abc import Iterator

import pytest
import fuzzgen
from qbprint import s16
from qbprint import s32

SEEDS = range(60)


def all_nodes(node: fuzzgen.Expr) -> Iterator[fuzzgen.Expr]:
    yield node
    match node:
        case fuzzgen.UnaryOp(_, _, operand):
            yield from all_nodes(operand)
        case fuzzgen.BinOp(_, _, left, right) | fuzzgen.Cmp(_, left, right):
            yield from all_nodes(left)
            yield from all_nodes(right)


def test_wrap_matches_measured_boundaries() -> None:
    # measured on VBDOS directly: see docs/handover.md's probe. 32767 + 1
    # wraps to -32768 and 300 * 300 wraps to 24464, with no /D in play.
    assert s16(32767 + 1) == -32768
    assert s16(300 * 300) == 24464
    assert s32(2147483647 + 1) == -2147483648


@pytest.mark.parametrize("seed", SEEDS)
def test_generation_is_deterministic(seed: int) -> None:
    a = fuzzgen.generate_program(seed=seed)
    b = fuzzgen.generate_program(seed=seed)
    assert fuzzgen.render_program(a) == fuzzgen.render_program(b)
    assert fuzzgen.golden_lines(a) == fuzzgen.golden_lines(b)


@pytest.mark.parametrize("seed", SEEDS)
def test_no_generated_program_traps(seed: int) -> None:
    # a program that hits the idiv fault case is not a golden -- it is a gap
    # in the divisor-safety filter
    program = fuzzgen.generate_program(seed=seed)
    fuzzgen.golden_lines(program)


@pytest.mark.parametrize("seed", range(300))
def test_every_binop_width_matches_its_own_natural_type(seed: int) -> None:
    # BASIC decides a BinOp's width from its own two immediate operands, not
    # from what the generator wanted several levels up -- found the hard way
    # (see natural_width()'s own docstring and fuzzgen.py's module docstring):
    # a LONG-tagged node whose real leaves were both INTEGER-typed is INTEGER
    # arithmetic in the BASIC BC actually compiles, wrapped at 16 bits, not
    # the 32 this generator's AST would otherwise wrap it at. 185 of a first
    # 200-program sample violated this before _ensure_valid_operand existed.
    program = fuzzgen.generate_program(seed=seed)
    declared = dict(program.declared)
    for stmt in program.stmts:
        expr = getattr(stmt, "expr", None) or getattr(stmt, "cond", None)
        if expr is None:
            continue
        for node in all_nodes(expr):
            if isinstance(node, fuzzgen.BinOp | fuzzgen.UnaryOp):
                assert fuzzgen.natural_width(node, declared) == node.width


@pytest.mark.parametrize("seed", SEEDS)
def test_no_if_condition_contains_not(seed: int) -> None:
    # measured on VBDOS under /O: "IF <expr> THEN..ELSE.." swaps branches on
    # <expr>'s own truthiness whenever NOT appears anywhere in it, not the
    # arithmetic NOT's -- a real BC finding (see AGENTS.md's "A BC compiler
    # behavior, not a qbopt one"), not something this grammar can fuzz safely
    program = fuzzgen.generate_program(seed=seed)
    for stmt in program.stmts:
        if isinstance(stmt, fuzzgen.IfPrint):
            assert not fuzzgen._contains_not(stmt.cond)


@pytest.mark.parametrize("seed", SEEDS)
def test_every_binop_has_a_variable_operand(seed: int) -> None:
    # BC constant-folds a BinOp of two pure literals at compile time and that
    # fold is overflow-checked, unlike the runtime instruction -- a generated
    # program that violates this fails to compile with "Math overflow" on an
    # unrelated later line (BC's own recovery attributes it there). Measured:
    # `(-1970530648 * -13199)` alone rejects; the same values via variables
    # (suite/divmod.bas's MULOVF) wrap silently at runtime.
    program = fuzzgen.generate_program(seed=seed)
    for stmt in program.stmts:
        expr = getattr(stmt, "expr", None) or getattr(stmt, "cond", None)
        if expr is None:
            continue
        for node in all_nodes(expr):
            if isinstance(node, fuzzgen.BinOp):
                assert fuzzgen._contains_var(node.left) or fuzzgen._contains_var(node.right)


@pytest.mark.parametrize("seed", SEEDS)
def test_golden_ends_with_done(seed: int) -> None:
    lines = fuzzgen.golden_lines(fuzzgen.generate_program(seed=seed))
    assert lines[-1] == "DONE"
    assert len(lines) == sum(1 for s in fuzzgen.generate_program(seed=seed).stmts if not isinstance(s, fuzzgen.Assign))


def test_render_never_emits_a_bare_min_literal() -> None:
    assert fuzzgen.render_lit(fuzzgen.Width.INT, -32768) == "(-32767 - 1)"
    assert fuzzgen.render_lit(fuzzgen.Width.LNG, -2147483648) == "(-2147483647 - 1)"
    assert fuzzgen.render_lit(fuzzgen.Width.INT, -5) == "-5"
    assert fuzzgen.render_lit(fuzzgen.Width.LNG, 5) == "5"


def test_unary_minus_never_glues_into_a_double_dash() -> None:
    # "--22904" confused BC's parser into misattributing a later line's error
    # (see fuzzgen.py's module docstring); a leading space after unary "-"
    # keeps the two operators apart no matter what its operand renders as.
    node = fuzzgen.UnaryOp(fuzzgen.Width.LNG, "-", fuzzgen.Lit(fuzzgen.Width.LNG, -22904))
    assert "--" not in fuzzgen.render_expr(node)


@pytest.mark.parametrize("seed", SEEDS)
def test_source_has_dos_line_endings_and_ends_in_done(seed: int) -> None:
    text = fuzzgen.render_program(fuzzgen.generate_program(seed=seed))
    assert text.count("\r\n") == text.count("\n")
    assert text.rstrip("\r\n").splitlines()[-1] == 'PRINT "DONE"'
