#!/usr/bin/env python3
"""Generate deterministic assignment/store coercion integration cases."""

from __future__ import annotations

import sys
import math
import struct
from pathlib import Path
from dataclasses import dataclass

TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

from rstest_codegen import render_generated_file
from rstest_codegen import render_rstest_function

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests" / "generated" / "assignment_coercion_cases.rs"


@dataclass(frozen=True, slots=True)
class StoreType:
    name: str
    as_type: str
    suffix: str
    et: int


@dataclass(frozen=True, slots=True)
class Expr:
    name: str
    source: str
    value: float | str


@dataclass(frozen=True, slots=True)
class AssignmentCase:
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


@dataclass(frozen=True, slots=True)
class AssignmentErrorCase:
    name: str
    source: str
    expected_error: int


TYPES: tuple[StoreType, ...] = (
    StoreType("integer", "INTEGER", "%", 1),
    StoreType("long", "LONG", "&", 2),
    StoreType("single", "SINGLE", "!", 3),
    StoreType("double", "DOUBLE", "#", 4),
    StoreType("string", "STRING", "$", 5),
)

NUMERIC_EXPRS: tuple[Expr, ...] = (
    Expr("positive_fraction", "1.6#", 1.6),
    Expr("negative_fraction", "-1.6#", -1.6),
    Expr("single_literal", "2.25!", 2.25),
    Expr("abs_intrinsic", "ABS(-5.5#)", 5.5),
)

STRING_EXPRS: tuple[Expr, ...] = (
    Expr("literal", '"abc"', "abc"),
    Expr("left_intrinsic", 'LEFT$("abcdef", 2)', "ab"),
    Expr("chr_intrinsic", "CHR$(65)", "A"),
)


def round_away_from_zero(value: float) -> int:
    if value >= 0:
        return math.floor(value + 0.5)
    return math.ceil(value - 0.5)


def coerce(value: float | str, target: StoreType) -> str:
    if target.name == "string":
        return str(value)
    number = float(value)
    if target.name in {"integer", "long"}:
        return str(round_away_from_zero(number))
    if target.name == "single":
        number = struct.unpack("f", struct.pack("f", number))[0]
    if number.is_integer():
        return str(int(number))
    return str(number)


def expr_for(target: StoreType, idx: int) -> Expr:
    if target.name == "string":
        return STRING_EXPRS[idx % len(STRING_EXPRS)]
    return NUMERIC_EXPRS[idx % len(NUMERIC_EXPRS)]


def scalar_cases() -> list[AssignmentCase]:
    cases: list[AssignmentCase] = []
    for target in TYPES:
        exprs = STRING_EXPRS if target.name == "string" else NUMERIC_EXPRS
        for idx, expr in enumerate(exprs):
            var = f"X{target.suffix}"
            source = f"""
DIM X AS {target.as_type}
{var} = {expr.source}
PRINT {var}
""".strip()
            cases.append(
                AssignmentCase(
                    f"scalar_{target.name}_from_{expr.name}_{idx}",
                    source,
                    (coerce(expr.value, target),),
                    "OP_ID_ST",
                    target.et,
                )
            )
    return cases


def array_cases() -> list[AssignmentCase]:
    cases: list[AssignmentCase] = []
    for target in TYPES:
        exprs = STRING_EXPRS if target.name == "string" else NUMERIC_EXPRS
        for idx, expr in enumerate(exprs):
            first = expr
            second = exprs[(idx + 1) % len(exprs)]
            source = f"""
DIM A{idx}(1) AS {target.as_type}
A{idx}(0) = {first.source}
A{idx}(1) = {second.source}
PRINT A{idx}(0)
PRINT A{idx}(1)
""".strip()
            cases.append(
                AssignmentCase(
                    f"array_{target.name}_series_from_{first.name}_{idx}",
                    source,
                    (coerce(first.value, target), coerce(second.value, target)),
                    "OP_AID_ST",
                    target.et,
                )
            )
    return cases


def udt_cases() -> list[AssignmentCase]:
    cases: list[AssignmentCase] = []
    for idx, target in enumerate(TYPES):
        expr = expr_for(target, idx)
        field = f"F{idx}"
        source = f"""
TYPE TRec{idx}
  {field} AS {target.as_type}
END TYPE
DIM R{idx} AS TRec{idx}
R{idx}.{field} = {expr.source}
PRINT R{idx}.{field}
""".strip()
        cases.append(
            AssignmentCase(
                f"udt_field_{target.name}_from_{expr.name}",
                source,
                (coerce(expr.value, target),),
                "OP_OFF_ST",
                target.et,
            )
        )
    for idx, target in enumerate(TYPES):
        expr = expr_for(target, idx + 1)
        field = f"F{idx}"
        source = f"""
TYPE TInnerStore{idx}
  {field} AS {target.as_type}
END TYPE
TYPE TOuterStore{idx}
  Inner AS TInnerStore{idx}
END TYPE
DIM ROuter{idx} AS TOuterStore{idx}
ROuter{idx}.Inner.{field} = {expr.source}
PRINT ROuter{idx}.Inner.{field}
""".strip()
        cases.append(
            AssignmentCase(
                f"nested_udt_field_{target.name}_from_{expr.name}",
                source,
                (coerce(expr.value, target),),
                "OP_OFF_ST",
                target.et,
            )
        )
    for idx, target in enumerate(TYPES):
        exprs = STRING_EXPRS if target.name == "string" else NUMERIC_EXPRS
        first = exprs[idx % len(exprs)]
        second = exprs[(idx + 1) % len(exprs)]
        field = f"F{idx}"
        source = f"""
TYPE TArrayStore{idx}
  {field} AS {target.as_type}
END TYPE
DIM RArray{idx}(1) AS TArrayStore{idx}
RArray{idx}(0).{field} = {first.source}
RArray{idx}(1).{field} = {second.source}
PRINT RArray{idx}(0).{field}
PRINT RArray{idx}(1).{field}
""".strip()
        cases.append(
            AssignmentCase(
                f"array_udt_field_{target.name}_series",
                source,
                (coerce(first.value, target), coerce(second.value, target)),
                "OP_OFF_ST",
                target.et,
            )
        )
    return cases


def function_return_cases() -> list[AssignmentCase]:
    cases: list[AssignmentCase] = []
    for target in TYPES:
        exprs = STRING_EXPRS if target.name == "string" else NUMERIC_EXPRS
        for idx, expr in enumerate(exprs):
            name = f"Ret{target.name}{idx}{target.suffix}"
            source = f"""
FUNCTION {name} ()
  {name} = {expr.source}
END FUNCTION
PRINT {name}()
""".strip()
            cases.append(
                AssignmentCase(
                    f"function_return_{target.name}_from_{expr.name}_{idx}",
                    source,
                    (coerce(expr.value, target),),
                    "OP_ID_ST",
                    target.et,
                )
            )
    return cases


def error_cases() -> list[AssignmentErrorCase]:
    er_tm = 13
    return [
        AssignmentErrorCase("string_scalar_from_number", "DIM X AS STRING\nX = 1.6#\nPRINT X", er_tm),
        AssignmentErrorCase("integer_scalar_from_string", 'DIM X AS INTEGER\nX = "abc"\nPRINT X', er_tm),
        AssignmentErrorCase("string_array_from_number", "DIM A(1) AS STRING\nA(0) = 1.6#\nPRINT A(0)", er_tm),
        AssignmentErrorCase("long_array_from_string", 'DIM A(1) AS LONG\nA(0) = "abc"\nPRINT A(0)', er_tm),
    ]


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_assignment_case_literal(case: AssignmentCase) -> str:
    return (
        "AssignmentCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_array(case.expected)},\n"
        f'    opcode: "{case.opcode}",\n'
        f"    et: {case.et},\n"
        "}"
    )


def render_assignment_error_case_literal(case: AssignmentErrorCase) -> str:
    return (
        "AssignmentErrorCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected_error: {case.expected_error},\n"
        "}"
    )


def main() -> None:
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct AssignmentCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "    pub opcode: &'static str,",
            "    pub et: u8,",
            "}",
            "",
            "#[derive(Debug, Clone, Copy)]",
            "pub struct AssignmentErrorCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected_error: u16,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_scalar_assignment_coercion_cases",
            "AssignmentCase",
            [(case.name, render_assignment_case_literal(case)) for case in scalar_cases()],
            "super::assert_assignment_case(case);",
        ),
        render_rstest_function(
            "generated_array_assignment_coercion_cases",
            "AssignmentCase",
            [(case.name, render_assignment_case_literal(case)) for case in array_cases()],
            "super::assert_assignment_case(case);",
        ),
        render_rstest_function(
            "generated_udt_assignment_coercion_cases",
            "AssignmentCase",
            [(case.name, render_assignment_case_literal(case)) for case in udt_cases()],
            "super::assert_assignment_case(case);",
        ),
        render_rstest_function(
            "generated_function_return_assignment_coercion_cases",
            "AssignmentCase",
            [(case.name, render_assignment_case_literal(case)) for case in function_return_cases()],
            "super::assert_assignment_case(case);",
        ),
        render_rstest_function(
            "generated_assignment_type_mismatch_cases",
            "AssignmentErrorCase",
            [(case.name, render_assignment_error_case_literal(case)) for case in error_cases()],
            "super::assert_assignment_error_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_assignment_coercion_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
