#!/usr/bin/env python3
"""Generate name/type binding and default-type cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "name_binding_cases.rs"


@dataclass(frozen=True, slots=True)
class NameBindingCase:
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


def cases() -> list[NameBindingCase]:
    generated = [
        NameBindingCase("defint_unsuffixed_store", "DEFINT A-Z\nX = 1.6#\nPRINT X", ("2",), "OP_ID_ST", 1),
        NameBindingCase("defdbl_unsuffixed_store", "DEFDBL A-Z\nX = 1.5#\nPRINT X", ("1.5",), "OP_ID_ST", 4),
        NameBindingCase("suffix_overrides_defstr", "DEFSTR A-Z\nX% = 1.6#\nPRINT X%", ("2",), "OP_ID_ST", 1),
        NameBindingCase(
            "dim_as_single_sets_variable_type",
            "DIM X AS SINGLE\nX = 1.6#\nPRINT X",
            ("1.600000023841858",),
            "OP_ID_ST",
            3,
        ),
        NameBindingCase(
            "proc_local_shadows_module_variable",
            "X = 1\nSUB Show\n  X = 2\n  PRINT X\nEND SUB\nCALL Show\nPRINT X",
            ("2", "1"),
            "OP_ID_LD",
            0,
        ),
        NameBindingCase(
            "formal_parameter_long_binding",
            "SUB Show (X AS LONG)\n  PRINT X\nEND SUB\nCALL Show(1.6#)",
            ("2",),
            "OP_ID_LD",
            2,
        ),
        NameBindingCase(
            "suffixed_function_return_binding",
            'FUNCTION MakeText$ (X AS STRING)\n  MakeText$ = X\nEND FUNCTION\nPRINT MakeText$("abc")',
            ("abc",),
            "OP_ID_ST",
            5,
        ),
        NameBindingCase(
            "const_binding_reuse",
            "CONST C = 2\nPRINT C + 3",
            ("5",),
            "OP_ST_CONST",
            0,
        ),
        NameBindingCase(
            "udt_field_operand_stays_member_name",
            "TYPE TRec\n  I AS INTEGER\nEND TYPE\nDIM R AS TRec\nR.I = 1\nPRINT R.I",
            ("1",),
            "OP_OFF_LD",
            1,
        ),
    ]
    def_cases = (
        ("deflng_a_m", "DEFLNG A-M\nAvar = 1.6#\nNvar = 1.6#\nPRINT Avar\nPRINT Nvar", ("2", "1.600000023841858"), 2),
        (
            "defsng_n_z",
            "DEFSNG N-Z\nMvar = 1.6#\nNvar = 1.6#\nPRINT Mvar\nPRINT Nvar",
            ("1.600000023841858", "1.600000023841858"),
            3,
        ),
        ("defstr_s_only", 'DEFSTR S-S\nSname = "ok"\nTname = 3\nPRINT Sname\nPRINT Tname', ("ok", "3"), 5),
    )
    for name, source, expected, et in def_cases:
        generated.append(NameBindingCase(name, source, expected, "OP_ID_ST", et))

    for idx in range(10):
        generated.append(
            NameBindingCase(
                f"dotted_scalar_and_udt_base_collision_{idx}",
                f"TYPE TBind{idx}\n  C AS INTEGER\nEND TYPE\nDIM A{idx}.B{idx} AS TBind{idx}\nA{idx}.B{idx}.C = {idx + 1}\nPRINT A{idx}.B{idx}.C",
                (str(idx + 1),),
                "OP_OFF_LD",
                0,
            )
        )

    for idx in range(10):
        generated.append(
            NameBindingCase(
                f"shared_module_visible_in_sub_{idx}",
                f"DIM SHARED SBind{idx} AS INTEGER\nSBind{idx} = {idx + 3}\nSUB ShowBind{idx}\n  PRINT SBind{idx}\nEND SUB\nCALL ShowBind{idx}",
                (str(idx + 3),),
                "OP_ID_LD",
                1,
            )
        )
        generated.append(
            NameBindingCase(
                f"local_array_shadows_module_scalar_{idx}",
                f"XBind{idx} = {idx + 4}\nSUB ShadowBind{idx}\n  DIM XBind{idx}(1) AS INTEGER\n  XBind{idx}(1) = {idx + 5}\n  PRINT XBind{idx}(1)\nEND SUB\nCALL ShadowBind{idx}\nPRINT XBind{idx}",
                (str(idx + 5), str(idx + 4)),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(10):
        generated.append(
            NameBindingCase(
                f"const_and_function_return_reuse_{idx}",
                f"CONST CBind{idx} = {idx + 6}\nFUNCTION FBind{idx}% ()\n  FBind{idx}% = CBind{idx} + 1\nEND FUNCTION\nPRINT CBind{idx}\nPRINT FBind{idx}%()",
                (str(idx + 6), str(idx + 7)),
                "OP_ST_CONST",
                0,
            )
        )
    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_name_binding_case_literal(case: NameBindingCase) -> str:
    return (
        "NameBindingCase {\n"
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
            "pub struct NameBindingCase {",
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
            "generated_name_binding_cases",
            "NameBindingCase",
            [(case.name, render_name_binding_case_literal(case)) for case in cases()],
            "super::assert_name_binding_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_name_binding_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
