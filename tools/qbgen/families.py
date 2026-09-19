"""Adapted qbasic-port source-semantic generator families.

The source project had ten generator families.  qbopt keeps those families
but narrows each to high-information witnesses: values differ by enough that
an incorrect type, binding, addressing calculation, or calling convention
cannot accidentally give the same result.  The former scan-pcode family is
strictly lexer/parser source coverage here.
"""

from __future__ import annotations

from tools.qbgen import ported
from tools.qbgen.model import GeneratedCase
from tools.qbgen.model import DialectOutcome

ORIGIN = "adapted from qbasic-port/tools/generate_"


def _case(family: str, name: str, source: str, **facts: object) -> GeneratedCase:
    facts.setdefault("origin", f"{ORIGIN}{family}_cases.py")
    return GeneratedCase(family, name, source, **facts)


def smoke_cases() -> tuple[GeneratedCase, ...]:
    """Fast stage witnesses in addition to—not instead of—the full corpus."""
    return (
        _case(
            "name_binding",
            "defint_and_explicit_as_are_distinct",
            "defint a-a\ndim apple\ndim amount as long\napple = 1.6#\namount = 300001\nprint apple\nprint amount",
            ast=("def_type", "dim", "assign", "print"),
            bindings=(("APPLE", "integer"), ("AMOUNT", "long")),
            hir_ops=("store",),
            expected_output=("2", "300001"),
        ),
        _case(
            "name_binding",
            "suffix_overrides_default_type",
            "defstr a-z\ndim count as integer\ncount% = 1.6#\nprint count%",
            ast=("def_type", "dim", "assign"),
            bindings=(("COUNT%", "integer"),),
            hir_ops=("store",),
            expected_output=("2",),
        ),
        _case(
            "name_binding",
            "procedure_local_shadows_module",
            "dim score as integer\nscore = 11\nsub showScore\n  dim score as integer\n  score = 29\n  print score\nend sub\ncall showScore\nprint score",
            ast=("dim", "procedure", "call"),
            bindings=(("SCORE", "integer"),),
            hir_ops=("call",),
            expected_output=("29", "11"),
        ),
        _case(
            "expression_promotion",
            "integer_plus_single_promotes",
            "dim leftValue as integer\ndim rightValue as single\nleftValue = 7\nrightValue = 2.25\nprint leftValue + rightValue",
            ast=("binary:add",),
            bindings=(("LEFTVALUE", "integer"), ("RIGHTVALUE", "single")),
            hir_ops=("fadd",),
            expected_output=("9.25",),
        ),
        _case(
            "expression_promotion",
            "long_multiply_is_whole_value",
            "dim factor as long\nfactor = 50001\nprint factor * 3",
            ast=("binary:multiply",),
            bindings=(("FACTOR", "long"),),
            hir_ops=("mul",),
            expected_output=("150003",),
        ),
        _case(
            "expression_promotion",
            "comparison_yields_basic_true",
            "dim value as integer\nvalue = 8\nprint value < 9",
            ast=("binary:less",),
            bindings=(("VALUE", "integer"),),
            hir_ops=("sub",),
            expected_output=("-1",),
        ),
        _case(
            "assignment_coercion",
            "integer_rounds_away_from_zero",
            "dim positive as integer\ndim negative as integer\npositive = 1.6#\nnegative = -1.6#\nprint positive\nprint negative",
            ast=("dim", "assign"),
            bindings=(("POSITIVE", "integer"), ("NEGATIVE", "integer")),
            hir_ops=("store",),
            expected_output=("2", "-2"),
        ),
        _case(
            "assignment_coercion",
            "string_number_is_semantic_error",
            "dim text as string\ntext = 14",
            outcomes=(
                DialectOutcome("qb45", "semantic-error"),
                DialectOutcome("pds71", "semantic-error"),
                DialectOutcome("vbdos", "semantic-error"),
            ),
            ast=("dim", "assign"),
        ),
        _case(
            "assignment_coercion",
            "number_string_is_semantic_error",
            'dim number as integer\nnumber = "14"',
            outcomes=(
                DialectOutcome("qb45", "semantic-error"),
                DialectOutcome("pds71", "semantic-error"),
                DialectOutcome("vbdos", "semantic-error"),
            ),
            ast=("dim", "assign"),
        ),
        _case(
            "array_access",
            "bounded_two_dimensional_distinguishes_cells",
            "option base 1\ndim cells(1 to 2, 1 to 3) as integer\ncells(1, 3) = 17\ncells(2, 1) = 29\nprint cells(1, 3)\nprint cells(2, 1)",
            ast=("option_base", "dim:array", "index"),
            bindings=(("CELLS", "array"),),
            hir_ops=("store", "load"),
            expected_output=("17", "29"),
        ),
        _case(
            "array_access",
            "computed_index_is_not_constant_slot",
            "dim cells(0 to 4) as integer\ndim index as integer\nindex = 1\ncells(index + 2) = 71\nprint cells(3)",
            ast=("dim:array", "index", "binary:add"),
            bindings=(("CELLS", "array"), ("INDEX", "integer")),
            hir_ops=("add", "store", "load"),
            expected_output=("71",),
        ),
        _case(
            "array_access",
            "array_subscript_count_is_semantic_error",
            "dim cells(1 to 2, 1 to 2) as integer\ncells(1) = 7",
            outcomes=(
                DialectOutcome("qb45", "semantic-error"),
                DialectOutcome("pds71", "semantic-error"),
                DialectOutcome("vbdos", "semantic-error"),
            ),
            ast=("dim:array", "index"),
        ),
        _case(
            "procedure_call",
            "byref_sub_mutates_caller",
            "sub increase (value as integer)\n  value = value + 3\nend sub\ndim total as integer\ntotal = 41\ncall increase(total)\nprint total",
            ast=("procedure", "call", "binary:add"),
            bindings=(("TOTAL", "integer"),),
            hir_ops=("call", "address"),
            expected_output=("44",),
        ),
        _case(
            "procedure_call",
            "function_return_and_argument_order",
            "function combine (leftValue as integer, rightValue as integer) as long\n  combine = leftValue * 100 + rightValue\nend function\nprint combine(12, 34)",
            ast=("procedure:function", "call", "binary:multiply", "binary:add"),
            bindings=(("COMBINE", "long"),),
            hir_ops=("call", "mul", "add"),
            expected_output=("1234",),
        ),
        _case(
            "procedure_call",
            "implicit_call_is_distinct_syntax",
            "sub showValue (value as integer)\n  print value\nend sub\nshowValue 73",
            ast=("procedure", "call:implicit"),
            hir_ops=("call",),
            expected_output=("73",),
        ),
        _case(
            "udt_access",
            "field_store_and_load_keep_offsets",
            "type PointValue\n  x as integer\n  y as long\nend type\ndim point as PointValue\npoint.x = 17\npoint.y = 900001\nprint point.x\nprint point.y",
            ast=("type_decl", "dim", "field"),
            bindings=(("POINT", "PointValue"),),
            hir_ops=("store", "load"),
            expected_output=("17", "900001"),
        ),
        _case(
            "udt_access",
            "nested_field_path",
            "type InnerValue\n  amount as integer\nend type\ntype OuterValue\n  inner as InnerValue\nend type\ndim record as OuterValue\nrecord.inner.amount = 37\nprint record.inner.amount",
            ast=("type_decl", "field"),
            bindings=(("RECORD", "OuterValue"),),
            hir_ops=("store", "load"),
            expected_output=("37",),
        ),
        _case(
            "builtin_arg",
            "abs_and_sqr_are_typed_intrinsics",
            "dim amount as double\namount = -81#\nprint abs(amount)\nprint sqr(abs(amount))",
            ast=("call:intrinsic",),
            bindings=(("AMOUNT", "double"),),
            hir_ops=("fneg", "call"),
            expected_output=("81", "9"),
        ),
        _case(
            "builtin_arg",
            "string_intrinsic_arity_is_semantic_error",
            'print left$("abc")',
            outcomes=(
                DialectOutcome("qb45", "semantic-error"),
                DialectOutcome("pds71", "semantic-error"),
                DialectOutcome("vbdos", "semantic-error"),
            ),
            ast=("call:intrinsic",),
        ),
        _case(
            "builtin_arg",
            "inline_trigonometry_has_no_runtime_helper",
            "dim angle as double\nangle = 0#\nprint sin(angle) + cos(angle)",
            ast=("call:intrinsic", "binary:add"),
            bindings=(("ANGLE", "double"),),
            hir_ops=("fadd",),
            expected_output=("1",),
        ),
        _case(
            "control_flow",
            "nested_loop_and_branch_checksum",
            "dim total as integer\ndim row as integer\ndim column as integer\nfor row = 1 to 3\n  for column = 1 to 4\n    if row = 2 and column = 3 then\n      total = total + 100\n    else\n      total = total + row * 10 + column\n    end if\n  next column\nnext row\nprint total",
            ast=("for", "if", "binary:and"),
            bindings=(("TOTAL", "integer"),),
            hir_ops=("add", "mul"),
            expected_output=("317",),
        ),
        _case(
            "control_flow",
            "do_until_updates_before_test",
            "dim count as integer\ndo\n  count = count + 7\nloop until count = 21\nprint count",
            ast=("do", "binary:eq"),
            bindings=(("COUNT", "integer"),),
            hir_ops=("add",),
            expected_output=("21",),
        ),
        _case(
            "control_flow",
            "select_cases_choose_one_arm",
            "dim keyValue as integer\nkeyValue = 4\nselect case keyValue\ncase 1 to 3\n  print 11\ncase 4\n  print 29\ncase else\n  print 47\nend select",
            ast=("select",),
            bindings=(("KEYVALUE", "integer"),),
            hir_ops=("sub",),
            expected_output=("29",),
        ),
        _case(
            "ambiguous_id",
            "function_name_without_parentheses_is_call",
            "function answer as integer\n  answer = 83\nend function\nprint answer",
            ast=("procedure:function", "call"),
            bindings=(("ANSWER", "integer"),),
            hir_ops=("call",),
            expected_output=("83",),
        ),
        _case(
            "ambiguous_id",
            "array_name_with_indices_is_index",
            "dim answer(1) as integer\nanswer(1) = 83\nprint answer(1)",
            ast=("dim:array", "index"),
            bindings=(("ANSWER", "array"),),
            hir_ops=("store", "load"),
            expected_output=("83",),
        ),
        _case(
            "scan_syntax",
            "colon_and_apostrophe_comments",
            "dim count as integer: count = 7 ' comment hides : count = 99\nrem this line is a deliberate REM semantic witness\nprint count",
            ast=("dim", "assign", "comment"),
            bindings=(("COUNT", "integer"),),
            hir_ops=("store",),
            expected_output=("7",),
        ),
        _case(
            "scan_syntax",
            "line_continuation_keeps_expression",
            "dim result as integer\nresult = 10 _\n  + 20\nprint result",
            ast=("assign", "binary:add"),
            bindings=(("RESULT", "integer"),),
            hir_ops=("add",),
            expected_output=("30",),
        ),
        _case(
            "dialect_gate",
            "underscore_outcomes_are_explicit",
            "dim pos_x as integer\npos_x = 17\nprint pos_x",
            outcomes=(
                DialectOutcome("qb45", "syntax-error"),
                DialectOutcome("pds71", "syntax-error"),
                DialectOutcome("vbdos"),
            ),
            ast=("dim", "assign"),
            origin="measured QB45/PDS71/VBDOS identifier gate",
        ),
        _case(
            "dialect_gate",
            "local_error_outcomes_are_explicit",
            "on local error goto handler\nhandler:\nprint 17",
            outcomes=(DialectOutcome("qb45", "syntax-error"), DialectOutcome("pds71"), DialectOutcome("vbdos")),
            ast=("on_error:local", "label"),
            origin="PDS/VBDOS local-error syntax",
        ),
    )


def cases() -> tuple[GeneratedCase, ...]:
    """The complete ported matrix followed by small direct AST/HIR witnesses."""
    return (*ported.cases(), *smoke_cases())
