#!/usr/bin/env python3
"""Generate deterministic QBasic parameter coercion integration cases."""

from __future__ import annotations

import sys
import struct
from pathlib import Path
from dataclasses import dataclass

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from rstest_codegen import render_generated_file
from rstest_codegen import render_rstest_function

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests" / "generated" / "param_coercion_cases.rs"


@dataclass(frozen=True, slots=True)
class ParamType:
    name: str
    stem: str
    as_type: str
    fn_suffix: str
    param_name: str


@dataclass(frozen=True, slots=True)
class NumericExpr:
    name: str
    source: str
    value: float


@dataclass(frozen=True, slots=True)
class StringExpr:
    name: str
    source: str
    value: str


@dataclass(frozen=True, slots=True)
class Case:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]


NUMERIC_TYPES: tuple[ParamType, ...] = (
    ParamType("integer", "Int", "INTEGER", "%", "x%"),
    ParamType("long", "Long", "LONG", "&", "x&"),
    ParamType("single", "Single", "SINGLE", "!", "x!"),
    ParamType("double", "Double", "DOUBLE", "#", "x#"),
)

STRING_TYPE = ParamType("string", "String", "STRING", "$", "x$")

NUMERIC_EXPRS: tuple[NumericExpr, ...] = (
    NumericExpr("int_literal", "2", 2.0),
    NumericExpr("positive_fraction", "1.6#", 1.6),
    NumericExpr("negative_fraction", "-1.6#", -1.6),
    NumericExpr("single_literal", "2.25!", 2.25),
    NumericExpr("double_literal", "2.75#", 2.75),
    NumericExpr("abs_intrinsic", "ABS(-5.5#)", 5.5),
    NumericExpr("sgn_intrinsic", "SGN(-7.25#)", -1.0),
    NumericExpr("len_intrinsic", 'LEN("abcd")', 4.0),
    NumericExpr("asc_intrinsic", 'ASC("C")', 67.0),
    NumericExpr("val_intrinsic", 'VAL("8.5")', 8.5),
    NumericExpr("fix_intrinsic", "FIX(-2.9#)", -2.0),
    NumericExpr("int_intrinsic", "INT(-2.1#)", -3.0),
)

STRING_EXPRS: tuple[StringExpr, ...] = (
    StringExpr("literal", '"abc"', "abc"),
    StringExpr("left_intrinsic", 'LEFT$("abcdef", 2)', "ab"),
    StringExpr("right_intrinsic", 'RIGHT$("abcdef", 3)', "def"),
    StringExpr("mid_intrinsic", 'MID$("abcdef", 2, 3)', "bcd"),
    StringExpr("chr_intrinsic", "CHR$(65)", "A"),
    StringExpr("lcase_intrinsic", 'LCASE$("QBASIC")', "qbasic"),
    StringExpr("hex_intrinsic", "HEX$(255)", "FF"),
)


def round_ties_to_even(value: float) -> int:
    return int(round(value))


def coerce_numeric(value: float, target: ParamType) -> float | int:
    if target.name in {"integer", "long"}:
        return round_ties_to_even(value)
    if target.name == "single":
        return struct.unpack("f", struct.pack("f", value))[0]
    return value


def format_number(value: float | int) -> str:
    if isinstance(value, int):
        return str(value)
    if value.is_integer():
        return str(int(value))
    return str(value)


def fn_name(target: ParamType, prefix: str) -> str:
    return f"{prefix}{target.stem}{target.fn_suffix}"


def sub_name(target: ParamType, prefix: str) -> str:
    return f"{prefix}{target.stem}"


def unary_function_case(target: ParamType, expr: NumericExpr) -> Case:
    name = fn_name(target, "Echo")
    expected = format_number(coerce_numeric(expr.value, target))
    source = f"""
FUNCTION {name} (x AS {target.as_type})
  {name} = x
END FUNCTION
PRINT {name}({expr.source})
""".strip()
    return Case("unary_function", f"function_{target.name}_from_{expr.name}", source, (expected,))


def unary_sub_case(target: ParamType, expr: NumericExpr) -> Case:
    name = sub_name(target, "Show")
    expected = format_number(coerce_numeric(expr.value, target))
    source = f"""
SUB {name} (x AS {target.as_type})
  PRINT x
END SUB
CALL {name}({expr.source})
""".strip()
    return Case("unary_sub", f"sub_{target.name}_from_{expr.name}", source, (expected,))


def unary_string_function_case(expr: StringExpr) -> Case:
    name = fn_name(STRING_TYPE, "Echo")
    source = f"""
FUNCTION {name} (x AS STRING)
  {name} = x
END FUNCTION
PRINT {name}({expr.source})
""".strip()
    return Case("unary_function", f"function_string_from_{expr.name}", source, (expr.value,))


def unary_string_sub_case(expr: StringExpr) -> Case:
    name = sub_name(STRING_TYPE, "Show")
    source = f"""
SUB {name} (x AS STRING)
  PRINT x
END SUB
CALL {name}({expr.source})
""".strip()
    return Case("unary_sub", f"sub_string_from_{expr.name}", source, (expr.value,))


def pairwise_function_cases() -> list[Case]:
    cases: list[Case] = []
    for left_idx, left in enumerate(NUMERIC_TYPES):
        for right_idx, right in enumerate(NUMERIC_TYPES):
            expr_a = NUMERIC_EXPRS[(left_idx + right_idx) % len(NUMERIC_EXPRS)]
            expr_b = NUMERIC_EXPRS[(left_idx * 3 + right_idx + 5) % len(NUMERIC_EXPRS)]
            a = coerce_numeric(expr_a.value, left)
            b = coerce_numeric(expr_b.value, right)
            expected = format_number(float(a) * 1000.0 + float(b))
            name = f"Mix{left.stem}{right.stem}#"
            source = f"""
FUNCTION {name} (a AS {left.as_type}, b AS {right.as_type})
  {name} = a * 1000# + b
END FUNCTION
PRINT {name}({expr_a.source}, {expr_b.source})
""".strip()
            cases.append(
                Case(
                    "pairwise_function",
                    f"function_pair_{left.name}_{right.name}_{expr_a.name}_{expr_b.name}",
                    source,
                    (expected,),
                )
            )
    return cases


def intrinsic_argument_cases() -> list[Case]:
    return [
        Case(
            "intrinsic_argument",
            "left_count_auto_coerces_fractional_double",
            'PRINT LEFT$("abcdef", 2.6#)',
            ("ab",),
        ),
        Case(
            "intrinsic_argument",
            "mid_start_and_len_auto_coerce_fractional_doubles",
            'PRINT MID$("abcdef", 2.4#, 2.6#)',
            ("bc",),
        ),
        Case(
            "intrinsic_argument",
            "chr_code_auto_coerces_fractional_double",
            "PRINT CHR$(64.6#)",
            ("@",),
        ),
        Case(
            "intrinsic_argument",
            "hex_value_auto_coerces_fractional_double",
            "PRINT HEX$(15.9#)",
            ("F",),
        ),
    ]


def intrinsic_consumes_function_result_cases() -> list[Case]:
    return [
        Case(
            "intrinsic_function_result",
            "left_count_consumes_double_function_result",
            """
FUNCTION MakeD# (x AS DOUBLE)
  MakeD# = x
END FUNCTION
PRINT LEFT$("abcdef", MakeD#(2.6#))
""".strip(),
            ("ab",),
        ),
        Case(
            "intrinsic_function_result",
            "mid_start_consumes_integer_function_result",
            """
FUNCTION MakeI% (x AS INTEGER)
  MakeI% = x
END FUNCTION
PRINT MID$("abcdef", MakeI%(2.4#), 2)
""".strip(),
            ("bc",),
        ),
        Case(
            "intrinsic_function_result",
            "chr_code_consumes_integer_function_result",
            """
FUNCTION MakeI% (x AS INTEGER)
  MakeI% = x
END FUNCTION
PRINT CHR$(MakeI%(64.6#))
""".strip(),
            ("A",),
        ),
        Case(
            "intrinsic_function_result",
            "len_consumes_string_function_result",
            """
FUNCTION MakeText$ (x AS STRING)
  MakeText$ = x
END FUNCTION
PRINT LEN(MakeText$("abcdef"))
""".strip(),
            ("6",),
        ),
    ]


def sub_receives_function_result_cases() -> list[Case]:
    cases: list[Case] = []
    for target, expr in zip(NUMERIC_TYPES, NUMERIC_EXPRS[1:5], strict=True):
        maker = "MakeD#"
        show = sub_name(target, "ShowFn")
        expected = format_number(coerce_numeric(expr.value, target))
        source = f"""
FUNCTION {maker} (x AS DOUBLE)
  {maker} = x
END FUNCTION
SUB {show} (x AS {target.as_type})
  PRINT x
END SUB
CALL {show}({maker}({expr.source}))
""".strip()
        cases.append(
            Case(
                "sub_function_result",
                f"sub_{target.name}_receives_function_result_{expr.name}",
                source,
                (expected,),
            )
        )
    maker = "MakeText$"
    show = sub_name(STRING_TYPE, "ShowFn")
    source = f"""
FUNCTION {maker} (x AS STRING)
  {maker} = x
END FUNCTION
SUB {show} (x AS STRING)
  PRINT x
END SUB
CALL {show}({maker}(LEFT$("abcdef", 3)))
""".strip()
    cases.append(
        Case(
            "sub_function_result",
            "sub_string_receives_function_result_left_intrinsic",
            source,
            ("abc",),
        )
    )
    return cases


def all_cases() -> dict[str, list[Case]]:
    unary_function = [unary_function_case(target, expr) for target in NUMERIC_TYPES for expr in NUMERIC_EXPRS]
    unary_function.extend(unary_string_function_case(expr) for expr in STRING_EXPRS)

    unary_sub = [unary_sub_case(target, expr) for target in NUMERIC_TYPES for expr in NUMERIC_EXPRS]
    unary_sub.extend(unary_string_sub_case(expr) for expr in STRING_EXPRS)

    return {
        "UNARY_FUNCTION_CASES": unary_function,
        "UNARY_SUB_CASES": unary_sub,
        "PAIRWISE_FUNCTION_CASES": pairwise_function_cases(),
        "INTRINSIC_ARGUMENT_CASES": intrinsic_argument_cases(),
        "INTRINSIC_FUNCTION_RESULT_CASES": intrinsic_consumes_function_result_cases(),
        "SUB_FUNCTION_RESULT_CASES": sub_receives_function_result_cases(),
    }


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_string_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_case(case: Case) -> str:
    return (
        "    ParamCoercionCase {\n"
        f'        name: "{case.name}",\n'
        f"        source: {rust_string(case.source)},\n"
        f"        expected: {rust_string_array(case.expected)},\n"
        "    },"
    )


def render_param_coercion_case_literal(case: Case) -> str:
    return (
        "ParamCoercionCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_string_array(case.expected)},\n"
        "}"
    )


def render_param_coercion_cases_const(name: str, cases: list[Case]) -> str:
    rendered = "\n".join(render_case(case) for case in cases)
    return f"pub const {name}: &[ParamCoercionCase] = &[\n{rendered}\n];"


def main() -> None:
    groups = all_cases()
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct ParamCoercionCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "}",
        ]
    )
    const_blocks = [
        render_param_coercion_cases_const("INTRINSIC_ARGUMENT_CASES", groups["INTRINSIC_ARGUMENT_CASES"]),
    ]
    rstest_blocks = [
        render_rstest_function(
            "generated_unary_function_param_coercion_cases",
            "ParamCoercionCase",
            [(case.name, render_param_coercion_case_literal(case)) for case in groups["UNARY_FUNCTION_CASES"]],
            "super::assert_param_coercion_case(case);",
        ),
        render_rstest_function(
            "generated_unary_sub_param_coercion_cases",
            "ParamCoercionCase",
            [(case.name, render_param_coercion_case_literal(case)) for case in groups["UNARY_SUB_CASES"]],
            "super::assert_param_coercion_case(case);",
        ),
        render_rstest_function(
            "generated_pairwise_function_param_coercion_cases",
            "ParamCoercionCase",
            [(case.name, render_param_coercion_case_literal(case)) for case in groups["PAIRWISE_FUNCTION_CASES"]],
            "super::assert_param_coercion_case(case);",
        ),
        render_rstest_function(
            "generated_intrinsic_argument_coercion_cases",
            "ParamCoercionCase",
            [(case.name, render_param_coercion_case_literal(case)) for case in groups["INTRINSIC_ARGUMENT_CASES"]],
            "super::assert_param_coercion_case(case);",
        ),
        render_rstest_function(
            "generated_intrinsic_function_result_coercion_cases",
            "ParamCoercionCase",
            [
                (case.name, render_param_coercion_case_literal(case))
                for case in groups["INTRINSIC_FUNCTION_RESULT_CASES"]
            ],
            "super::assert_param_coercion_case(case);",
        ),
        render_rstest_function(
            "generated_sub_function_result_coercion_cases",
            "ParamCoercionCase",
            [(case.name, render_param_coercion_case_literal(case)) for case in groups["SUB_FUNCTION_RESULT_CASES"]],
            "super::assert_param_coercion_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_param_coercion_cases.py",
            struct_block,
            rstest_blocks,
            const_blocks=const_blocks,
        )
    )


if __name__ == "__main__":
    main()
