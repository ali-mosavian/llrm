"""Floating storage formats and rounding boundaries survive the raise."""

from pathlib import Path

import corpus
import pytest

from qbopt.model import mir


def test_negation_and_absolute_value_have_distinct_mir_meanings():
    """FABS and FCHS both raised as FNEG, concealing abs(2)=2 versus -2."""
    from qbopt.model import ir
    argument = ir.St(0)
    absolute = ir.Semantics(ir.Operation.FLOAT_UNARY, name="fabs", sources=(argument,), dests=(argument,))
    negate = ir.Semantics(ir.Operation.FLOAT_UNARY, name="fchs", sources=(argument,), dests=(argument,))
    assert mir._kind_of(absolute, (), ()) != mir._kind_of(negate, (), ())


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_single_store_is_a_rounding_boundary(tag):
    """FPCSE stores p and q as SINGLE; replacing their reloads by extended intermediates changes semantics."""
    path = Path(f"fixtures/omf/fpcse-{tag}.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    ops = [op for block in body.blocks for op in block.ops]
    load = next(op for op in ops if op.kind is mir.Kind.FLOAD)
    multiply = next(op for op in ops if op.kind is mir.Kind.FMUL)
    store = next(op for op in ops if op.kind is mir.Kind.FSTORE)
    assert all(getattr(op, "floating", None) is not None for op in (load, multiply, store))
    assert tuple(load.floating.inputs) == ("binary32",)
    assert load.floating.result == "extended80"
    assert load.floating.rounding == "none"
    assert multiply.floating.precision == "dynamic"
    assert multiply.floating.rounding == "dynamic"
    assert store.floating.inputs == ("extended80",)
    assert store.floating.result == "binary32"
    assert store.floating.rounding == "dynamic"
    assert all(op.floating.exceptions == "strict" for op in (load, multiply, store))


@pytest.mark.parametrize("name,width,expected", [
    ("fld", 4, "binary32"), ("fld", 8, "binary64"), ("fld", 10, "extended80"),
    ("fild", 2, "signed16"), ("fild", 4, "signed32"), ("fild", 8, "signed64"),
    ("fstp", 4, "binary32"), ("fstp", 8, "binary64"), ("fstp", 10, "extended80"),
    ("fistp", 2, "signed16"), ("fistp", 4, "signed32"), ("fistp", 8, "signed64"),
    ("fld", 2, None), ("fild", 10, None), ("unknown", 4, None),
])
def test_conversion_formats_do_not_confuse_integer_and_real(name, width, expected):
    """The same four bytes mean signed integer for FILD, binary32 for FLD."""
    from qbopt.model import ir
    from qbopt.frontend import raising_floats
    store = name in ("fstp", "fistp")
    ref = mir.MemRef(None, width)
    op = mir.Op(0, ir.Operation.FLOAT_STORE if store else ir.Operation.FLOAT_LOAD, name, (), (),
                kind=mir.Kind.FSTORE if store else mir.Kind.FLOAD,
                loads=() if store else (ref,), stores=(ref,) if store else ())
    rule = raising_floats.semantics(op)
    if expected is None:
        assert rule is None
        return
    assert (rule.result if store else rule.inputs[0]) == expected
    assert rule.exceptions == "strict"
    assert rule.rounding == ("dynamic" if store and width != 10 else "none")


@pytest.mark.parametrize("name,precision,rounding", [
    ("fchs", "exact", "none"), ("fabs", "exact", "none"), ("fsqrt", "dynamic", "dynamic")
])
def test_unary_precision_is_explicit(name, precision, rounding):
    from qbopt.model import ir
    from qbopt.frontend import raising_floats
    op = mir.Op(0, ir.Operation.FLOAT_UNARY, name, (), ())
    rule = raising_floats.semantics(op)
    assert rule.precision == precision and rule.rounding == rounding
    assert rule.exceptions == "strict"


def test_dump_exposes_single_rounding(capsys):
    from tools import stages
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    body = mir.bodies(corpus.loaded(path), corpus.partitioned(path))[0][1]
    stages._mir([("main", body)])
    assert "extended80 -> binary32; precision=destination rounding=dynamic exceptions=strict" in capsys.readouterr().out
