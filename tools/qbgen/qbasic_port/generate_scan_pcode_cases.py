#!/usr/bin/env python3
"""Generate control-flow scan pcode-shape cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "scan_pcode_cases.rs"


@dataclass(frozen=True, slots=True)
class ScanPcodeCase:
    name: str
    source: str
    opcode: str


def cases() -> list[ScanPcodeCase]:
    generated: list[ScanPcodeCase] = []
    for idx in range(25):
        generated.append(
            ScanPcodeCase(
                f"for_next_shape_{idx}",
                f"total = 0\nFOR I{idx} = 1 TO 3\n  total = total + I{idx}\nNEXT I{idx}\nPRINT total",
                "OP_ST_FOR",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"if_elseif_else_shape_{idx}",
                f"X{idx} = {idx % 3}\nIF X{idx} = 0 THEN\n  PRINT 0\nELSEIF X{idx} = 1 THEN\n  PRINT 1\nELSE\n  PRINT 2\nEND IF",
                "OP_ST_IF_BLOCK",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"do_loop_exit_shape_{idx}",
                f"X{idx} = 0\nDO\n  X{idx} = X{idx} + 1\n  IF X{idx} = 2 THEN EXIT DO\nLOOP WHILE X{idx} < 5\nPRINT X{idx}",
                "OP_ST_DO",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"while_wend_shape_{idx}",
                f"X{idx} = 0\nWHILE X{idx} < 2\n  X{idx} = X{idx} + 1\nWEND\nPRINT X{idx}",
                "OP_ST_WHILE",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"select_case_shape_{idx}",
                f"X{idx} = {idx % 4}\nSELECT CASE X{idx}\nCASE 0, 1\n  PRINT 1\nCASE 2 TO 3\n  PRINT 2\nCASE ELSE\n  PRINT 3\nEND SELECT",
                "OP_ST_SELECT_CASE",
            )
        )
    for idx in range(15):
        generated.append(
            ScanPcodeCase(
                f"select_contains_for_and_exit_{idx}",
                f"X{idx} = 2\nTotal{idx} = 0\nSELECT CASE X{idx}\nCASE 2\n  FOR I{idx} = 1 TO 4\n    IF I{idx} = 3 THEN EXIT FOR\n    Total{idx} = Total{idx} + I{idx}\n  NEXT I{idx}\nCASE ELSE\n  Total{idx} = 99\nEND SELECT\nPRINT Total{idx}",
                "OP_ST_SELECT_CASE",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"do_contains_while_and_exit_{idx}",
                f"X{idx} = 0\nDO\n  WHILE X{idx} < 2\n    X{idx} = X{idx} + 1\n  WEND\n  EXIT DO\nLOOP\nPRINT X{idx}",
                "OP_ST_DO",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"goto_around_nested_blocks_{idx}",
                f"GOTO After{idx}\nFOR I{idx} = 1 TO 2\n  PRINT 99\nNEXT I{idx}\nAfter{idx}:\nIF -1 THEN\n  PRINT {idx}\nEND IF",
                "OP_ST_IF_BLOCK",
            )
        )
        generated.append(
            ScanPcodeCase(
                f"nested_do_for_select_{idx}",
                f"Total{idx} = 0\nDO WHILE Total{idx} < 1\n  FOR I{idx} = 1 TO 2\n    SELECT CASE I{idx}\n    CASE 1\n      Total{idx} = Total{idx} + 1\n    CASE ELSE\n      Total{idx} = Total{idx} + 0\n    END SELECT\n  NEXT I{idx}\nLOOP\nPRINT Total{idx}",
                "OP_ST_DO_WHILE",
            )
        )
    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def render_scan_pcode_case_literal(case: ScanPcodeCase) -> str:
    return (
        "ScanPcodeCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f'    opcode: "{case.opcode}",\n'
        "}"
    )


def main() -> None:
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct ScanPcodeCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub opcode: &'static str,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_control_flow_scan_pcode_cases",
            "ScanPcodeCase",
            [(case.name, render_scan_pcode_case_literal(case)) for case in cases()],
            "super::assert_scan_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_scan_pcode_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
