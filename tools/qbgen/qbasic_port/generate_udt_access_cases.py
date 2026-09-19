#!/usr/bin/env python3
"""Generate UDT access-chain cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "udt_access_cases.rs"


@dataclass(frozen=True, slots=True)
class UdtCase:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


TYPES: tuple[tuple[str, str, int, str, str], ...] = (
    ("integer", "INTEGER", 1, "1.6#", "2"),
    ("long", "LONG", 2, "100000", "100000"),
    ("single", "SINGLE", 3, "3.25!", "3.25"),
    ("double", "DOUBLE", 4, "4.5#", "4.5"),
    ("string", "STRING", 5, '"udt"', "udt"),
)


def cases() -> list[UdtCase]:
    generated: list[UdtCase] = []
    for idx in range(40):
        name, as_type, et, expr, expected = TYPES[idx % len(TYPES)]
        field = f"F{idx}"
        generated.append(
            UdtCase(
                "scalar_field",
                f"scalar_field_{name}_{idx}",
                f"TYPE TScalar{idx}\n  {field} AS {as_type}\nEND TYPE\nDIM R{idx} AS TScalar{idx}\nR{idx}.{field} = {expr}\nPRINT R{idx}.{field}",
                (expected,),
                "OP_OFF_LD",
                et,
            )
        )

    for idx in range(35):
        value = idx + 7
        generated.append(
            UdtCase(
                "nested_field",
                f"nested_field_chain_{idx}",
                f"TYPE TInner{idx}\n  X AS INTEGER\nEND TYPE\nTYPE TOuter{idx}\n  Inner AS TInner{idx}\nEND TYPE\nDIM R{idx} AS TOuter{idx}\nR{idx}.Inner.X = {value}\nPRINT R{idx}.Inner.X",
                (str(value),),
                "OP_OFF_LD",
                1,
            )
        )

    for idx in range(30):
        first = idx * 10 + 20
        second = idx * 10 + 21
        third = idx * 10 + 22
        total = first + second + third
        checksum = first + second * 3 + third * 7
        generated.append(
            UdtCase(
                "array_of_udt",
                f"array_of_udt_member_series_{idx}",
                f"TYPE TArrRec{idx}\n  X AS INTEGER\nEND TYPE\nDIM A{idx}(2) AS TArrRec{idx}\nA{idx}(0).X = {first}\nA{idx}(1).X = {second}\nA{idx}(2).X = {third}\nS{idx}% = A{idx}(0).X + A{idx}(1).X + A{idx}(2).X\nC{idx}% = A{idx}(0).X + A{idx}(1).X * 3 + A{idx}(2).X * 7\nPRINT A{idx}(0).X\nPRINT A{idx}(1).X\nPRINT A{idx}(2).X\nPRINT S{idx}%\nPRINT C{idx}%",
                (str(first), str(second), str(third), str(total), str(checksum)),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(20):
        generated.append(
            UdtCase(
                "fixed_string",
                f"fixed_string_field_{idx}",
                f'TYPE TFixed{idx}\n  S AS STRING * 10\nEND TYPE\nDIM R{idx} AS TFixed{idx}\nR{idx}.S = "Name{idx}"\nPRINT R{idx}.S',
                (f"Name{idx}",),
                "OP_OFF_LD",
                0,
            )
        )

    for idx in range(20):
        value = idx + 50
        generated.append(
            UdtCase(
                "shared_udt",
                f"shared_udt_field_{idx}",
                f"TYPE TShared{idx}\n  X AS INTEGER\nEND TYPE\nDIM SHARED R{idx} AS TShared{idx}\nR{idx}.X = {value}\nSUB Show{idx}\n  PRINT R{idx}.X\nEND SUB\nCALL Show{idx}",
                (str(value),),
                "OP_OFF_LD",
                1,
            )
        )

    for idx in range(30):
        value = idx + 80
        generated.append(
            UdtCase(
                "max_chain",
                f"dotted_array_member_chain_{idx}",
                f"TYPE TUdtInner{idx}\n  X AS INTEGER\nEND TYPE\nTYPE TUdtOuter{idx}\n  UdtArr AS TUdtInner{idx}\nEND TYPE\nDIM Arr.With.Name{idx}(2) AS TUdtOuter{idx}\nI.J{idx}% = 1\nArr.With.Name{idx}(I.J{idx}%).UdtArr.X = {value}\nPRINT Arr.With.Name{idx}(I.J{idx}%).UdtArr.X",
                (str(value),),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(20):
        a0 = idx * 20 + 1
        a1 = idx * 20 + 2
        b0 = idx * 20 + 11
        b1 = idx * 20 + 12
        total = a0 + a1 + b0 + b1
        checksum = a0 + a1 * 3 + b0 * 7 + b1 * 11
        generated.append(
            UdtCase(
                "multi_field_array",
                f"multi_field_array_of_udt_{idx}",
                f"TYPE TMultiArr{idx}\n  A AS INTEGER\n  B AS LONG\nEND TYPE\nDIM MArr{idx}(1) AS TMultiArr{idx}\nMArr{idx}(0).A = {a0}\nMArr{idx}(0).B = {b0}\nMArr{idx}(1).A = {a1}\nMArr{idx}(1).B = {b1}\nSMulti{idx}& = MArr{idx}(0).A + MArr{idx}(1).A + MArr{idx}(0).B + MArr{idx}(1).B\nCMulti{idx}& = MArr{idx}(0).A + MArr{idx}(1).A * 3 + MArr{idx}(0).B * 7 + MArr{idx}(1).B * 11\nPRINT MArr{idx}(0).A\nPRINT MArr{idx}(1).A\nPRINT MArr{idx}(0).B\nPRINT MArr{idx}(1).B\nPRINT SMulti{idx}&\nPRINT CMulti{idx}&",
                (str(a0), str(a1), str(b0), str(b1), str(total), str(checksum)),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(20):
        root0 = idx * 30 + 1
        root1 = idx * 30 + 2
        left0 = idx * 30 + 11
        left1 = idx * 30 + 12
        right0 = idx * 30 + 21
        right1 = idx * 30 + 22
        total = root0 + root1 + left0 + left1 + right0 + right1
        checksum = root0 + root1 * 3 + left0 * 7 + left1 * 11 + right0 * 13 + right1 * 17
        generated.append(
            UdtCase(
                "nested_multi_field_array",
                f"nested_multi_field_array_of_udt_{idx}",
                f"TYPE TLeafArr{idx}\n  X AS INTEGER\n  Y AS LONG\nEND TYPE\nTYPE TOuterArr{idx}\n  Root AS INTEGER\n  Left AS TLeafArr{idx}\n  Right AS TLeafArr{idx}\nEND TYPE\nDIM NArr{idx}(1) AS TOuterArr{idx}\nNArr{idx}(0).Root = {root0}\nNArr{idx}(0).Left.X = {left0}\nNArr{idx}(0).Right.Y = {right0}\nNArr{idx}(1).Root = {root1}\nNArr{idx}(1).Left.X = {left1}\nNArr{idx}(1).Right.Y = {right1}\nSNArr{idx}& = NArr{idx}(0).Root + NArr{idx}(1).Root + NArr{idx}(0).Left.X + NArr{idx}(1).Left.X + NArr{idx}(0).Right.Y + NArr{idx}(1).Right.Y\nCNArr{idx}& = NArr{idx}(0).Root + NArr{idx}(1).Root * 3 + NArr{idx}(0).Left.X * 7 + NArr{idx}(1).Left.X * 11 + NArr{idx}(0).Right.Y * 13 + NArr{idx}(1).Right.Y * 17\nPRINT NArr{idx}(0).Root\nPRINT NArr{idx}(1).Root\nPRINT NArr{idx}(0).Left.X\nPRINT NArr{idx}(1).Left.X\nPRINT NArr{idx}(0).Right.Y\nPRINT NArr{idx}(1).Right.Y\nPRINT SNArr{idx}&\nPRINT CNArr{idx}&",
                (
                    str(root0),
                    str(root1),
                    str(left0),
                    str(left1),
                    str(right0),
                    str(right1),
                    str(total),
                    str(checksum),
                ),
                "OP_AID_LD",
                0,
            )
        )

    return generated


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_udt_case_literal(case: UdtCase) -> str:
    return (
        "UdtCase {\n"
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
            "pub struct UdtCase {",
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
            "generated_udt_access_cases",
            "UdtCase",
            [(case.name, render_udt_case_literal(case)) for case in cases()],
            "super::assert_udt_case(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_udt_access_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
