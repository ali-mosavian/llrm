#!/usr/bin/env python3
"""Generate explicit/implicit procedure-call ambiguity cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "procedure_call_cases.rs"


@dataclass(frozen=True, slots=True)
class ProcedureCallCase:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str


def cases() -> list[ProcedureCallCase]:
    generated: list[ProcedureCallCase] = []

    for idx in range(30):
        value = idx + 1
        generated.append(
            ProcedureCallCase(
                "explicit_call",
                f"explicit_call_one_arg_{idx}",
                f"SUB Show{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nCALL Show{idx}({value})",
                (str(value),),
                "OP_ST_CALL",
            )
        )
        generated.append(
            ProcedureCallCase(
                "implicit_call",
                f"implicit_call_one_arg_{idx}",
                f"SUB ShowImplicit{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nShowImplicit{idx} {value}",
                (str(value),),
                "OP_ST_CALL_LESS",
            )
        )

    for idx in range(25):
        value = idx + 10
        generated.append(
            ProcedureCallCase(
                "bare_no_arg",
                f"bare_no_arg_sub_{idx}",
                f"SUB Play{idx}\n  PRINT {value}\nEND SUB\nPlay{idx}",
                (str(value),),
                "OP_ST_CALL_LESS",
            )
        )
        generated.append(
            ProcedureCallCase(
                "function_expression",
                f"function_in_expression_{idx}",
                f"FUNCTION Init{idx}% (X AS INTEGER)\n  Init{idx}% = X + 1\nEND FUNCTION\nPRINT Init{idx}%({value}) * 2",
                (str((value + 1) * 2),),
                "OP_ST_CALL_LESS",
            )
        )

    for idx in range(20):
        value = idx + 2
        generated.append(
            ProcedureCallCase(
                "implicit_parenthesized_expr",
                f"implicit_parenthesized_expression_arg_{idx}",
                f"SUB Scale{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nScale{idx} ({value} * 2)",
                (str(value * 2),),
                "OP_ST_CALL_LESS",
            )
        )
        generated.append(
            ProcedureCallCase(
                "nested_function_arg",
                f"nested_function_argument_{idx}",
                f"FUNCTION F{idx}% (X AS INTEGER)\n  F{idx}% = X + 1\nEND FUNCTION\nSUB Draw{idx} (X AS INTEGER, Y AS INTEGER)\n  PRINT X; Y\nEND SUB\nCALL Draw{idx}(F{idx}%({value}), F{idx}%({value + 1}))",
                (str(value + 1), str(value + 2)),
                "OP_ST_CALL",
            )
        )

    for idx in range(20):
        value = idx + 5
        generated.append(
            ProcedureCallCase(
                "statement_separator",
                f"statement_separator_adjacent_call_{idx}",
                f"SUB Bump{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nA{idx} = {value}: CALL Bump{idx}(A{idx}): PRINT A{idx}",
                (str(value), str(value)),
                "OP_ST_CALL",
            )
        )

    for idx in range(20):
        value = idx + 30
        generated.append(
            ProcedureCallCase(
                "byref_mutation",
                f"implicit_byref_mutation_{idx}",
                f"SUB Mutate{idx} (X AS INTEGER)\n  X = X + 2\nEND SUB\nA{idx}% = {value}\nMutate{idx} A{idx}%\nPRINT A{idx}%",
                (str(value + 2),),
                "OP_ST_CALL_LESS",
            )
        )
        generated.append(
            ProcedureCallCase(
                "array_argument",
                f"whole_array_mutation_call_{idx}",
                f"SUB MutArr{idx} (A() AS INTEGER)\n  A(1) = A(1) + 3\nEND SUB\nDIM V{idx}(1 TO 2) AS INTEGER\nV{idx}(1) = {value}\nCALL MutArr{idx}(V{idx}())\nPRINT V{idx}(1)",
                (str(value + 3),),
                "OP_ST_CALL",
            )
        )

    for idx in range(20):
        a = idx + 1
        b = idx + 2
        c = idx + 3
        generated.append(
            ProcedureCallCase(
                "many_args",
                f"explicit_many_args_{idx}",
                f"SUB Many{idx} (A AS INTEGER, B AS INTEGER, C AS INTEGER)\n  PRINT A; B; C\nEND SUB\nCALL Many{idx}({a}, {b}, {c})",
                (str(a), str(b), str(c)),
                "OP_ST_CALL",
            )
        )
        generated.append(
            ProcedureCallCase(
                "udt_field_arg",
                f"udt_field_argument_{idx}",
                f"TYPE TCall{idx}\n  X AS INTEGER\nEND TYPE\nDIM RCall{idx} AS TCall{idx}\nRCall{idx}.X = {a}\nSUB ShowField{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nShowField{idx} RCall{idx}.X",
                (str(a),),
                "OP_ST_CALL_LESS",
            )
        )

    for idx in range(15):
        value = idx + 50
        generated.append(
            ProcedureCallCase(
                "control_context",
                f"call_inside_if_and_for_{idx}",
                f"SUB Emit{idx} (X AS INTEGER)\n  PRINT X\nEND SUB\nIF -1 THEN CALL Emit{idx}({value})\nFOR I{idx}% = 1 TO 2\n  Emit{idx} I{idx}%\nNEXT I{idx}%",
                (str(value), "1", "2"),
                "OP_ST_CALL",
            )
        )

    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_procedure_call_case_literal(case: ProcedureCallCase) -> str:
    return (
        "ProcedureCallCase {\n"
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
            "pub struct ProcedureCallCase {",
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
            "generated_procedure_call_cases",
            "ProcedureCallCase",
            [(case.name, render_procedure_call_case_literal(case)) for case in cases()],
            "super::assert_procedure_call_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_procedure_call_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
