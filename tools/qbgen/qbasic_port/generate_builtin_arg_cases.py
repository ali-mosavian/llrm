#!/usr/bin/env python3
"""Generate built-in argument coercion contract cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "builtin_arg_cases.rs"


@dataclass(frozen=True, slots=True)
class BuiltinCase:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str


def cases() -> list[BuiltinCase]:
    generated = [
        BuiltinCase(
            "string_count", "right_count_truncates_double", 'PRINT RIGHT$("abcdef", 2.9#)', ("ef",), "OP_FN_RIGHT_"
        ),
        BuiltinCase("string_count", "space_count_truncates_double", "PRINT LEN(SPACE$(3.9#))", ("3",), "OP_FN_SPACE_"),
        BuiltinCase(
            "string_count",
            "instr_three_arg_current_contract",
            'PRINT INSTR(2.9#, "abcdef", "d")',
            ("0",),
            "OP_FN_INSTR3",
        ),
        BuiltinCase("string_count", "oct_arg_truncates_double", "PRINT OCT$(15.9#)", ("17",), "OP_FN_OCT_"),
        BuiltinCase(
            "graphics",
            "pset_point_round_fractional_coordinates",
            "SCREEN 9\nPSET (10.6#, 20.6#), 3\nPRINT POINT(11, 21)",
            ("3",),
            "OP_ST_PSET_COLOR",
        ),
        BuiltinCase(
            "graphics",
            "point_rounds_fractional_coordinates",
            "SCREEN 9\nPSET (11, 21), 3\nPRINT POINT(10.6#, 20.6#)",
            ("3",),
            "OP_FN_POINT2",
        ),
        BuiltinCase(
            "read", "read_integer_destination_rounds_data", "DATA 1.6\nREAD X%\nPRINT X%", ("2",), "OP_ST_READ"
        ),
        BuiltinCase(
            "read", "read_string_destination_preserves_data", 'DATA "abc"\nREAD X$\nPRINT X$', ("abc",), "OP_ST_READ"
        ),
    ]
    for idx in range(20):
        generated.append(
            BuiltinCase(
                "string_count_matrix",
                f"left_right_mid_nested_counts_{idx}",
                f'A$ = "abcdefghi"\nN{idx}# = {idx % 3 + 2}.9#\nPRINT LEFT$(A$, N{idx}#)\nPRINT RIGHT$(A$, N{idx}#)\nPRINT MID$(A$, 2.9#, N{idx}#)',
                ("abcdefghi"[: idx % 3 + 2], "abcdefghi"[-(idx % 3 + 2) :], "abcdefghi"[1 : 1 + idx % 3 + 2]),
                "OP_FN_LEFT_",
            )
        )
    for idx in range(20):
        value = idx + 65
        generated.append(
            BuiltinCase(
                "numeric_string_intrinsics",
                f"chr_asc_len_roundtrip_{idx}",
                f'X{idx}% = {value}\nA{idx}$ = CHR$(X{idx}%)\nPRINT A{idx}$\nPRINT ASC(A{idx}$)\nPRINT LEN(A{idx}$ + "zz")',
                (chr(value), str(value), "3"),
                "OP_FN_CHR_",
            )
        )
    for idx in range(20):
        generated.append(
            BuiltinCase(
                "read",
                f"read_mixed_destinations_{idx}",
                f'DATA {idx + 1}.6, "S{idx}", {idx + 2}.4\nREAD A{idx}%, B{idx}$, C{idx}&\nPRINT A{idx}%\nPRINT B{idx}$\nPRINT C{idx}&',
                (str(idx + 2), f"S{idx}", str(idx + 2)),
                "OP_ST_READ",
            )
        )
    for idx in range(10):
        generated.append(
            BuiltinCase(
                "graphics",
                f"pset_point_series_{idx}",
                f"SCREEN 9\nPSET ({10 + idx}.6#, {20 + idx}.6#), {idx % 4 + 1}\nPSET ({12 + idx}, {22 + idx}), {idx % 4 + 2}\nPRINT POINT({11 + idx}, {21 + idx})\nPRINT POINT({12 + idx}.1#, {22 + idx}.1#)",
                (str(idx % 4 + 1), str(idx % 4 + 2)),
                "OP_ST_PSET_COLOR",
            )
        )
    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_builtin_case_literal(case: BuiltinCase) -> str:
    return (
        "BuiltinCase {\n"
        f'    family: "{case.family}",\n'
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_array(case.expected)},\n"
        f'    opcode: "{case.opcode}",\n'
        "}"
    )


def main() -> None:
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct BuiltinCase {",
            "    pub family: &'static str,",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "    pub opcode: &'static str,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_builtin_argument_contract_cases",
            "BuiltinCase",
            [(case.name, render_builtin_case_literal(case)) for case in cases()],
            "super::assert_builtin_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_builtin_arg_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
