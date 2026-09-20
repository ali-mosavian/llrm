#!/usr/bin/env python3
"""Generate expression promotion pcode and behavior cases."""

from __future__ import annotations

import sys
from pathlib import Path
from dataclasses import dataclass

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from rstest_codegen import render_generated_file
from rstest_codegen import render_rstest_function

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests" / "generated" / "expression_promotion_cases.rs"


@dataclass(frozen=True, slots=True)
class ExpressionCase:
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


def cases() -> list[ExpressionCase]:
    generated: list[ExpressionCase] = [
        ExpressionCase(
            "add_integer_single_promotes_to_single",
            "DIM A AS INTEGER\nDIM B AS SINGLE\nA = 1\nB = 2.25\nPRINT A + B",
            ("3.25",),
            "OP_ADD",
            3,
        ),
        ExpressionCase(
            "add_string_concat_promotes_to_string",
            'S$ = "a"\nT$ = "b"\nPRINT S$ + T$',
            ("ab",),
            "OP_ADD",
            5,
        ),
        ExpressionCase(
            "sub_double_integer_promotes_to_double",
            "DIM A AS DOUBLE\nDIM B AS INTEGER\nA = 5#\nB = 2\nPRINT A - B",
            ("3",),
            "OP_SUB",
            4,
        ),
        ExpressionCase(
            "mul_long_integer_promotes_to_long",
            "DIM A AS LONG\nDIM B AS INTEGER\nA = 2\nB = 3\nPRINT A * B",
            ("6",),
            "OP_MUL",
            2,
        ),
        ExpressionCase(
            "div_integer_integer_promotes_to_single",
            "DIM A AS INTEGER\nDIM B AS INTEGER\nA = 5\nB = 2\nPRINT A / B",
            ("2.5",),
            "OP_DIV",
            3,
        ),
        ExpressionCase(
            "mod_integer_integer_stays_integer",
            "DIM A AS INTEGER\nDIM B AS INTEGER\nA = 5\nB = 2\nPRINT A MOD B",
            ("1",),
            "OP_MOD",
            1,
        ),
        ExpressionCase(
            "power_integer_integer_promotes_to_single",
            "DIM A AS INTEGER\nDIM B AS INTEGER\nA = 2\nB = 3\nPRINT A ^ B",
            ("8",),
            "OP_PWR",
            3,
        ),
        ExpressionCase(
            "comparison_result_is_integer",
            "DIM A AS INTEGER\nDIM B AS SINGLE\nA = 1\nB = 2.25\nPRINT A < B",
            ("-1",),
            "OP_LT",
            0,
        ),
        ExpressionCase(
            "unary_minus_preserves_double",
            "DIM A AS DOUBLE\nA = 1.5#\nPRINT -A",
            ("-1.5",),
            "OP_UMI",
            4,
        ),
        ExpressionCase(
            "abs_preserves_single",
            "DIM A AS SINGLE\nA = -2.25\nPRINT ABS(A)",
            ("2.25",),
            "OP_FN_ABS",
            3,
        ),
        ExpressionCase(
            "fix_preserves_double",
            "DIM A AS DOUBLE\nA = -2.9#\nPRINT FIX(A)",
            ("-2",),
            "OP_FN_FIX",
            4,
        ),
        ExpressionCase(
            "sqr_returns_double",
            "DIM A AS INTEGER\nA = 4\nPRINT SQR(A)",
            ("2",),
            "OP_FN_SQR",
            4,
        ),
    ]

    numeric_types = (
        ("integer", "INTEGER", "I", 1, "2", "3", 1),
        ("long", "LONG", "L", 2, "20000", "3", 2),
        ("single", "SINGLE", "S", 3, "2.5", "3.5", 3),
        ("double", "DOUBLE", "D", 4, "2.5#", "3.5#", 4),
    )
    ops = (
        ("add", "+", "OP_ADD", lambda a, b: a + b),
        ("sub", "-", "OP_SUB", lambda a, b: a - b),
        ("mul", "*", "OP_MUL", lambda a, b: a * b),
    )
    for left_idx, (left_name, left_type, left_prefix, left_et, left_src, _, _) in enumerate(numeric_types):
        for right_idx, (
            right_name,
            right_type,
            right_prefix,
            right_et,
            _,
            right_src,
            _,
        ) in enumerate(numeric_types):
            for op_name, symbol, opcode, func in ops:
                left_value = float(left_src.rstrip("#!"))
                right_value = float(right_src.rstrip("#!"))
                value = func(left_value, right_value)
                expected = str(int(value)) if value.is_integer() else str(value)
                left_var = f"L{left_prefix}{left_idx}{right_idx}"
                right_var = f"R{right_prefix}{left_idx}{right_idx}"
                generated.append(
                    ExpressionCase(
                        f"matrix_{op_name}_{left_name}_{right_name}",
                        f"DIM {left_var} AS {left_type}\nDIM {right_var} AS {right_type}\n{left_var} = {left_src}\n{right_var} = {right_src}\nPRINT {left_var} {symbol} {right_var}",
                        (expected,),
                        opcode,
                        0,
                    )
                )

    for idx, (name, as_type, prefix, et, left_src, right_src, _) in enumerate(numeric_types):
        generated.append(
            ExpressionCase(
                f"nested_parentheses_{name}_chain",
                f"DIM {prefix}N{idx} AS {as_type}\n{prefix}N{idx} = {left_src}\nPRINT (({prefix}N{idx} + {right_src}) * ({prefix}N{idx} - 1))",
                (
                    str(
                        int(
                            (float(left_src.rstrip("#!")) + float(right_src.rstrip("#!")))
                            * (float(left_src.rstrip("#!")) - 1)
                        )
                    )
                    if (
                        (float(left_src.rstrip("#!")) + float(right_src.rstrip("#!")))
                        * (float(left_src.rstrip("#!")) - 1)
                    ).is_integer()
                    else str(
                        (float(left_src.rstrip("#!")) + float(right_src.rstrip("#!")))
                        * (float(left_src.rstrip("#!")) - 1)
                    ),
                ),
                "OP_MUL",
                0,
            )
        )
        generated.append(
            ExpressionCase(
                f"array_operand_{name}_promotion",
                f"DIM AExpr{idx}(2) AS {as_type}\nAExpr{idx}(0) = {left_src}\nAExpr{idx}(1) = {right_src}\nPRINT AExpr{idx}(0) + AExpr{idx}(1)",
                (
                    str(int(float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))))
                    if (float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))).is_integer()
                    else str(float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))),
                ),
                "OP_ADD",
                0,
            )
        )
        generated.append(
            ExpressionCase(
                f"udt_member_operand_{name}_promotion",
                f"TYPE TExpr{idx}\n  A AS {as_type}\n  B AS {as_type}\nEND TYPE\nDIM RExpr{idx} AS TExpr{idx}\nRExpr{idx}.A = {left_src}\nRExpr{idx}.B = {right_src}\nPRINT RExpr{idx}.A + RExpr{idx}.B",
                (
                    str(int(float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))))
                    if (float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))).is_integer()
                    else str(float(left_src.rstrip("#!")) + float(right_src.rstrip("#!"))),
                ),
                "OP_ADD",
                0,
            )
        )

    comparisons = (
        ("eq", "=", "OP_EQ", "1", "1", "-1"),
        ("gt", ">", "OP_GT", "2", "1", "-1"),
        ("ge", ">=", "OP_GE", "2", "2", "-1"),
        ("le", "<=", "OP_LE", "1", "2", "-1"),
    )
    for name, symbol, opcode, left, right, expected in comparisons:
        generated.append(
            ExpressionCase(
                f"comparison_{name}_numeric_truth",
                f"PRINT {left} {symbol} {right}",
                (expected,),
                opcode,
                0,
            )
        )

    bool_cases = (
        ("and", "AND", "OP_AND", "-1", "0", "0"),
        ("or", "OR", "OP_OR", "-1", "0", "-1"),
        ("eqv", "EQV", "OP_EQV", "-1", "-1", "-1"),
        ("imp", "IMP", "OP_IMP", "-1", "0", "0"),
    )
    for name, symbol, opcode, left, right, expected in bool_cases:
        generated.append(
            ExpressionCase(
                f"boolean_{name}_integer_operands",
                f"PRINT {left} {symbol} {right}",
                (expected,),
                opcode,
                0,
            )
        )

    generated.append(
        ExpressionCase(
            "string_concat_nested_chain",
            'A$ = "a"\nB$ = "b"\nC$ = "c"\nPRINT (A$ + B$) + C$',
            ("abc",),
            "OP_ADD",
            5,
        )
    )
    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_expression_case_literal(case: ExpressionCase) -> str:
    return (
        "ExpressionCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_array(case.expected)},\n"
        f'    opcode: "{case.opcode}",\n'
        f"    et: {case.et},\n"
        "}"
    )


def main() -> None:
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct ExpressionCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "    pub opcode: &'static str,",
            "    pub et: u8,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_expression_promotion_cases",
            "ExpressionCase",
            [(case.name, render_expression_case_literal(case)) for case in cases()],
            "super::assert_expression_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_expression_promotion_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
