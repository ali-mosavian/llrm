#!/usr/bin/env python3
"""Generate array access and whole-array reference cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "array_access_cases.rs"


@dataclass(frozen=True, slots=True)
class ArrayCase:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


TYPES: tuple[tuple[str, str, int, str, str], ...] = (
    ("integer", "INTEGER", 1, "1.6#", "2"),
    ("long", "LONG", 2, "2.6#", "3"),
    ("single", "SINGLE", 3, "3.25!", "3.25"),
    ("double", "DOUBLE", 4, "4.5#", "4.5"),
    ("string", "STRING", 5, '"arr"', "arr"),
)


def cases() -> list[ArrayCase]:
    generated: list[ArrayCase] = []
    for idx in range(30):
        name, as_type, et, expr, expected = TYPES[idx % len(TYPES)]
        generated.append(
            ArrayCase(
                "scalar_bounds",
                f"simple_bounds_{name}_{idx}",
                f"DIM A{idx}(3) AS {as_type}\nA{idx}(2) = {expr}\nPRINT A{idx}(2)",
                (expected,),
                "OP_AID_LD",
                et,
            )
        )
    for idx in range(25):
        name, as_type, et, expr, expected = TYPES[idx % len(TYPES)]
        generated.append(
            ArrayCase(
                "option_base",
                f"option_base_one_{name}_{idx}",
                f"OPTION BASE 1\nDIM B{idx}(3) AS {as_type}\nB{idx}(1) = {expr}\nPRINT B{idx}(1)",
                (expected,),
                "OP_AID_LD",
                et,
            )
        )
    for idx in range(25):
        name, as_type, et, expr, expected = TYPES[idx % len(TYPES)]
        generated.append(
            ArrayCase(
                "two_dimensional",
                f"two_dimensional_{name}_{idx}",
                f"DIM M{idx}(1 TO 2, 1 TO 2) AS {as_type}\nM{idx}(2, 1) = {expr}\nPRINT M{idx}(2, 1)",
                (expected,),
                "OP_AID_LD",
                et,
            )
        )
    for idx in range(25):
        value = idx + 10
        generated.append(
            ArrayCase(
                "whole_array_byref",
                f"whole_array_byref_{idx}",
                f"DIM Scores{idx}(1 TO 2)\nScores{idx}(1) = {value}\nSUB Show{idx} (Arr())\n  PRINT Arr(1)\nEND SUB\nCALL Show{idx}(Scores{idx}())",
                (str(value),),
                "OP_AVT_RF",
                0,
            )
        )
    for idx in range(20):
        value = idx + 5
        generated.append(
            ArrayCase(
                "fractional_index",
                f"fractional_index_truncates_{idx}",
                f"DIM F{idx}(2) AS INTEGER\nF{idx}(1) = {value}\nPRINT F{idx}(1.6#)",
                (str(value),),
                "OP_AID_LD",
                1,
            )
        )
    for idx in range(20):
        a = idx * 10 + 1
        b = idx * 10 + 2
        c = idx * 10 + 3
        total = a + b + c
        checksum = a + b * 3 + c * 7
        generated.append(
            ArrayCase(
                "three_dimensional",
                f"three_dimensional_mixed_bounds_{idx}",
                f"DIM T{idx}(-1 TO 1, 2 TO 4, 0 TO 1) AS INTEGER\nT{idx}(-1, 2, 0) = {a}\nT{idx}(0, 3, 1) = {b}\nT{idx}(1, 4, 0) = {c}\nS{idx}% = T{idx}(-1, 2, 0) + T{idx}(0, 3, 1) + T{idx}(1, 4, 0)\nC{idx}% = T{idx}(-1, 2, 0) + T{idx}(0, 3, 1) * 3 + T{idx}(1, 4, 0) * 7\nPRINT T{idx}(-1, 2, 0)\nPRINT T{idx}(0, 3, 1)\nPRINT T{idx}(1, 4, 0)\nPRINT S{idx}%\nPRINT C{idx}%",
                (str(a), str(b), str(c), str(total), str(checksum)),
                "OP_AID_LD",
                1,
            )
        )
    for idx in range(20):
        value = idx + 70
        generated.append(
            ArrayCase(
                "computed_indices",
                f"computed_index_expression_{idx}",
                f"DIM Cmp{idx}(0 TO 4, 0 TO 4) AS INTEGER\nI{idx}% = 1\nJ{idx}% = 2\nCmp{idx}(I{idx}% + 1, J{idx}% * 2) = {value}\nPRINT Cmp{idx}(2, 4)",
                (str(value),),
                "OP_AID_LD",
                1,
            )
        )
    for idx in range(15):
        value = idx + 90
        generated.append(
            ArrayCase(
                "whole_array_byref_mutation",
                f"whole_array_byref_mutates_{idx}",
                f"DIM Mut{idx}(1 TO 3) AS INTEGER\nMut{idx}(1) = {value}\nSUB BumpArr{idx} (Arr() AS INTEGER)\n  Arr(1) = Arr(1) + 1\n  Arr(3) = Arr(1) + 2\nEND SUB\nCALL BumpArr{idx}(Mut{idx}())\nPRINT Mut{idx}(1)\nPRINT Mut{idx}(3)",
                (str(value + 1), str(value + 3)),
                "OP_AVT_RF",
                0,
            )
        )
    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_array_case_literal(case: ArrayCase) -> str:
    return (
        "ArrayCase {\n"
        f'    family: "{case.family}",\n'
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
            "pub struct ArrayCase {",
            "    pub family: &'static str,",
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
            "generated_array_access_cases",
            "ArrayCase",
            [(case.name, render_array_case_literal(case)) for case in cases()],
            "super::assert_array_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_array_access_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
