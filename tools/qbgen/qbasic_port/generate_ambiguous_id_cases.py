#!/usr/bin/env python3
"""Generate ambiguous identifier/member parsing cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "ambiguous_id_cases.rs"


@dataclass(frozen=True, slots=True)
class AmbiguousIdCase:
    name: str
    source: str
    expected: tuple[str, ...]
    opcode: str
    et: int


@dataclass(frozen=True, slots=True)
class KnownFailureCase:
    name: str
    source: str
    reason: str


TYPES: tuple[tuple[str, str, str, int, str, str], ...] = (
    ("integer", "INTEGER", "%", 1, "1.6#", "2"),
    ("long", "LONG", "&", 2, "2.6#", "3"),
    ("single", "SINGLE", "!", 3, "3.25!", "3.25"),
    ("double", "DOUBLE", "#", 4, "4.5#", "4.5"),
    ("string", "STRING", "$", 5, '"s"', "s"),
)


def cases() -> list[AmbiguousIdCase]:
    generated: list[AmbiguousIdCase] = []

    for idx, (name, as_type, suffix, et, expr, expected) in enumerate(TYPES):
        field = f"F{idx}"
        generated.append(
            AmbiguousIdCase(
                f"udt_member_{name}_same_token_shape",
                f"TYPE TRec{idx}\n  {field} AS {as_type}\nEND TYPE\nDIM R{idx} AS TRec{idx}\nR{idx}.{field} = {expr}\nPRINT R{idx}.{field}",
                (expected,),
                "OP_OFF_LD",
                et,
            )
        )
        generated.append(
            AmbiguousIdCase(
                f"array_element_{name}_not_function_call",
                f"DIM Name{idx}(2) AS {as_type}\nName{idx}(1) = {expr}\nPRINT Name{idx}(1)",
                (expected,),
                "OP_AID_LD",
                et,
            )
        )
        generated.append(
            AmbiguousIdCase(
                f"suffix_function_{name}_not_array_element",
                f"FUNCTION Fn{idx}{suffix} (X AS {as_type})\n  Fn{idx}{suffix} = X\nEND FUNCTION\nPRINT Fn{idx}{suffix}({expr})",
                (expected,),
                "OP_ST_CALL_LESS",
                0,
            )
        )

    for idx in range(25):
        value = idx + 1
        generated.append(
            AmbiguousIdCase(
                f"array_name_call_shape_{idx}",
                f"DIM Amb{idx}(2) AS INTEGER\nAmb{idx}(1) = {value}\nPRINT Amb{idx}(1)",
                (str(value),),
                "OP_AID_LD",
                1,
            )
        )
        generated.append(
            AmbiguousIdCase(
                f"function_name_call_shape_{idx}",
                f"FUNCTION AmbFn{idx}% (X AS INTEGER)\n  AmbFn{idx}% = X + {value}\nEND FUNCTION\nPRINT AmbFn{idx}%(1)",
                (str(value + 1),),
                "OP_ST_CALL_LESS",
                0,
            )
        )

    for idx in range(20):
        value = idx + 10
        generated.append(
            AmbiguousIdCase(
                f"whole_array_byref_argument_{idx}",
                f"DIM Scores{idx}(1 TO 2)\nScores{idx}(1) = {value}\nSUB ShowArr{idx} (Arr())\n  PRINT Arr(1)\nEND SUB\nCALL ShowArr{idx}(Scores{idx}())",
                (str(value),),
                "OP_AVT_RF",
                0,
            )
        )

    for idx in range(20):
        value = idx + 3
        generated.append(
            AmbiguousIdCase(
                f"nested_arg_member_array_function_{idx}",
                f"TYPE TArg{idx}\n  V AS INTEGER\nEND TYPE\nDIM Rec{idx} AS TArg{idx}\nDIM Arr{idx}(2) AS INTEGER\nRec{idx}.V = {value}\nArr{idx}(1) = {value + 1}\nFUNCTION Pick{idx}% (X AS INTEGER)\n  Pick{idx}% = X + 1\nEND FUNCTION\nSUB Show{idx} (A AS INTEGER, B AS INTEGER, C AS INTEGER)\n  PRINT A; B; C\nEND SUB\nCALL Show{idx}(Rec{idx}.V, Arr{idx}(1), Pick{idx}%(Arr{idx}(1)))",
                (str(value), str(value + 1), str(value + 2)),
                "OP_OFF_LD",
                1,
            )
        )

    for idx in range(30):
        first = idx * 10 + 1
        second = idx * 10 + 2
        third = idx * 10 + 3
        total = first + second + third
        checksum = first + second * 3 + third * 7
        generated.append(
            AmbiguousIdCase(
                f"array_element_member_combination_{idx}",
                f"TYPE TArr{idx}\n  X AS INTEGER\nEND TYPE\nDIM P{idx}(2) AS TArr{idx}\nP{idx}(0).X = {first}\nP{idx}(1).X = {second}\nP{idx}(2).X = {third}\nS{idx}% = P{idx}(0).X + P{idx}(1).X + P{idx}(2).X\nC{idx}% = P{idx}(0).X + P{idx}(1).X * 3 + P{idx}(2).X * 7\nPRINT P{idx}(0).X\nPRINT P{idx}(1).X\nPRINT P{idx}(2).X\nPRINT S{idx}%\nPRINT C{idx}%",
                (str(first), str(second), str(third), str(total), str(checksum)),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(50):
        value = idx + 40
        generated.append(
            AmbiguousIdCase(
                f"max_ambiguity_dotted_array_index_member_chain_{idx}",
                f"TYPE Inner{idx}\n  X AS INTEGER\nEND TYPE\nTYPE Outer{idx}\n  UdtArr AS Inner{idx}\nEND TYPE\nDIM Arr.With.Name{idx}(2) AS Outer{idx}\nI.J{idx}% = 1\nArr.With.Name{idx}(I.J{idx}%).UdtArr.X = {value}\nPRINT Arr.With.Name{idx}(I.J{idx}%).UdtArr.X",
                (str(value),),
                "OP_AID_LD",
                0,
            )
        )
    for idx in range(30):
        first = idx * 10 + 101
        second = idx * 10 + 102
        third = idx * 10 + 103
        total = first + second + third
        checksum = first + second * 3 + third * 7
        generated.append(
            AmbiguousIdCase(
                f"scalar_udt_field_after_dotted_array_index_{idx}",
                f"TYPE InnerBad{idx}\n  X AS INTEGER\nEND TYPE\nTYPE OuterBad{idx}\n  UdtArr AS InnerBad{idx}\nEND TYPE\nDIM Arr.With.Bad{idx}(2) AS OuterBad{idx}\nI.JBad{idx}A% = 0\nI.JBad{idx}B% = 1\nI.JBad{idx}C% = 2\nArr.With.Bad{idx}(I.JBad{idx}A%).UdtArr.X = {first}\nArr.With.Bad{idx}(I.JBad{idx}B%).UdtArr.X = {second}\nArr.With.Bad{idx}(I.JBad{idx}C%).UdtArr.X = {third}\nSBad{idx}% = Arr.With.Bad{idx}(I.JBad{idx}A%).UdtArr.X + Arr.With.Bad{idx}(I.JBad{idx}B%).UdtArr.X + Arr.With.Bad{idx}(I.JBad{idx}C%).UdtArr.X\nCBad{idx}% = Arr.With.Bad{idx}(I.JBad{idx}A%).UdtArr.X + Arr.With.Bad{idx}(I.JBad{idx}B%).UdtArr.X * 3 + Arr.With.Bad{idx}(I.JBad{idx}C%).UdtArr.X * 7\nPRINT Arr.With.Bad{idx}(I.JBad{idx}A%).UdtArr.X\nPRINT Arr.With.Bad{idx}(I.JBad{idx}B%).UdtArr.X\nPRINT Arr.With.Bad{idx}(I.JBad{idx}C%).UdtArr.X\nPRINT SBad{idx}%\nPRINT CBad{idx}%",
                (str(first), str(second), str(third), str(total), str(checksum)),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(30):
        a0 = idx * 20 + 1
        a1 = idx * 20 + 2
        a2 = idx * 20 + 3
        b0 = idx * 20 + 11
        b1 = idx * 20 + 12
        b2 = idx * 20 + 13
        total = a0 + a1 + a2 + b0 + b1 + b2
        checksum = a0 + a1 * 3 + a2 * 7 + b0 * 11 + b1 * 13 + b2 * 17
        generated.append(
            AmbiguousIdCase(
                f"multi_field_primitive_udt_array_series_{idx}",
                f'TYPE TPrimMulti{idx}\n  A AS INTEGER\n  B AS LONG\n  Label AS STRING\nEND TYPE\nDIM P.Multi.Prim{idx}(2) AS TPrimMulti{idx}\nP.Multi.Prim{idx}(0).A = {a0}\nP.Multi.Prim{idx}(0).B = {b0}\nP.Multi.Prim{idx}(0).Label = "P{idx}A"\nP.Multi.Prim{idx}(1).A = {a1}\nP.Multi.Prim{idx}(1).B = {b1}\nP.Multi.Prim{idx}(1).Label = "P{idx}B"\nP.Multi.Prim{idx}(2).A = {a2}\nP.Multi.Prim{idx}(2).B = {b2}\nP.Multi.Prim{idx}(2).Label = "P{idx}C"\nSPrim{idx}& = P.Multi.Prim{idx}(0).A + P.Multi.Prim{idx}(1).A + P.Multi.Prim{idx}(2).A + P.Multi.Prim{idx}(0).B + P.Multi.Prim{idx}(1).B + P.Multi.Prim{idx}(2).B\nCPrim{idx}& = P.Multi.Prim{idx}(0).A + P.Multi.Prim{idx}(1).A * 3 + P.Multi.Prim{idx}(2).A * 7 + P.Multi.Prim{idx}(0).B * 11 + P.Multi.Prim{idx}(1).B * 13 + P.Multi.Prim{idx}(2).B * 17\nPRINT P.Multi.Prim{idx}(0).A\nPRINT P.Multi.Prim{idx}(1).A\nPRINT P.Multi.Prim{idx}(2).A\nPRINT P.Multi.Prim{idx}(0).B\nPRINT P.Multi.Prim{idx}(1).B\nPRINT P.Multi.Prim{idx}(2).B\nPRINT P.Multi.Prim{idx}(0).Label\nPRINT P.Multi.Prim{idx}(1).Label\nPRINT P.Multi.Prim{idx}(2).Label\nPRINT SPrim{idx}&\nPRINT CPrim{idx}&',
                (
                    str(a0),
                    str(a1),
                    str(a2),
                    str(b0),
                    str(b1),
                    str(b2),
                    f"P{idx}A",
                    f"P{idx}B",
                    f"P{idx}C",
                    str(total),
                    str(checksum),
                ),
                "OP_AID_LD",
                0,
            )
        )

    for idx in range(30):
        root0 = idx * 30 + 1
        root1 = idx * 30 + 2
        root2 = idx * 30 + 3
        left0 = idx * 30 + 11
        left1 = idx * 30 + 12
        left2 = idx * 30 + 13
        right0 = idx * 30 + 21
        right1 = idx * 30 + 22
        right2 = idx * 30 + 23
        total = root0 + root1 + root2 + left0 + left1 + left2 + right0 + right1 + right2
        checksum = (
            root0
            + root1 * 3
            + root2 * 7
            + left0 * 11
            + left1 * 13
            + left2 * 17
            + right0 * 19
            + right1 * 23
            + right2 * 29
        )
        generated.append(
            AmbiguousIdCase(
                f"multi_field_nested_udt_array_series_{idx}",
                f"TYPE TLeafMulti{idx}\n  X AS INTEGER\n  Y AS LONG\nEND TYPE\nTYPE TOuterMulti{idx}\n  Root AS INTEGER\n  Left AS TLeafMulti{idx}\n  Right AS TLeafMulti{idx}\nEND TYPE\nDIM Arr.Multi.Nested{idx}(2) AS TOuterMulti{idx}\nI.Multi{idx}A% = 0\nI.Multi{idx}B% = 1\nI.Multi{idx}C% = 2\nArr.Multi.Nested{idx}(I.Multi{idx}A%).Root = {root0}\nArr.Multi.Nested{idx}(I.Multi{idx}A%).Left.X = {left0}\nArr.Multi.Nested{idx}(I.Multi{idx}A%).Right.Y = {right0}\nArr.Multi.Nested{idx}(I.Multi{idx}B%).Root = {root1}\nArr.Multi.Nested{idx}(I.Multi{idx}B%).Left.X = {left1}\nArr.Multi.Nested{idx}(I.Multi{idx}B%).Right.Y = {right1}\nArr.Multi.Nested{idx}(I.Multi{idx}C%).Root = {root2}\nArr.Multi.Nested{idx}(I.Multi{idx}C%).Left.X = {left2}\nArr.Multi.Nested{idx}(I.Multi{idx}C%).Right.Y = {right2}\nSNested{idx}& = Arr.Multi.Nested{idx}(I.Multi{idx}A%).Root + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Root + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Root + Arr.Multi.Nested{idx}(I.Multi{idx}A%).Left.X + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Left.X + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Left.X + Arr.Multi.Nested{idx}(I.Multi{idx}A%).Right.Y + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Right.Y + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Right.Y\nCNested{idx}& = Arr.Multi.Nested{idx}(I.Multi{idx}A%).Root + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Root * 3 + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Root * 7 + Arr.Multi.Nested{idx}(I.Multi{idx}A%).Left.X * 11 + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Left.X * 13 + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Left.X * 17 + Arr.Multi.Nested{idx}(I.Multi{idx}A%).Right.Y * 19 + Arr.Multi.Nested{idx}(I.Multi{idx}B%).Right.Y * 23 + Arr.Multi.Nested{idx}(I.Multi{idx}C%).Right.Y * 29\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}A%).Root\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}B%).Root\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}C%).Root\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}A%).Left.X\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}B%).Left.X\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}C%).Left.X\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}A%).Right.Y\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}B%).Right.Y\nPRINT Arr.Multi.Nested{idx}(I.Multi{idx}C%).Right.Y\nPRINT SNested{idx}&\nPRINT CNested{idx}&",
                (
                    str(root0),
                    str(root1),
                    str(root2),
                    str(left0),
                    str(left1),
                    str(left2),
                    str(right0),
                    str(right1),
                    str(right2),
                    str(total),
                    str(checksum),
                ),
                "OP_AID_LD",
                0,
            )
        )

    suffix_cases = (
        ("", "7", "7", 0),
        ("%", "8", "8", 1),
        ("&", "9", "9", 2),
        ("!", "10.25!", "10.25", 3),
        ("#", "11.5#", "11.5", 4),
        ("$", '"dot"', "dot", 5),
    )
    for idx, (suffix, expr, expected, et) in enumerate(suffix_cases):
        generated.append(
            AmbiguousIdCase(
                f"literal_dotted_identifier_suffix_{idx}",
                f"A{idx}.B{suffix} = {expr}\nPRINT A{idx}.B{suffix}",
                (expected,),
                "OP_ID_LD",
                et,
            )
        )

    dotted_scalar_cases = (
        (
            "multi_dot_literal_identifier_three_parts",
            "A.B.C = 13\nPRINT A.B.C",
            ("13",),
        ),
        (
            "multi_dot_literal_identifier_four_parts",
            "A.B.C.D = 14\nPRINT A.B.C.D",
            ("14",),
        ),
        (
            "multi_dot_literal_identifier_default_type",
            "DEFDBL A-Z\nA.B.C = 15.5#\nPRINT A.B.C",
            ("15.5",),
        ),
        (
            "multi_dot_literal_identifier_coexists_with_prefix",
            "A.B = 16\nA.B.C = 17\nPRINT A.B\nPRINT A.B.C",
            ("16", "17"),
        ),
        (
            "multi_dot_member_chain_with_dotted_udt_base",
            "TYPE TDotInner\n  D AS INTEGER\nEND TYPE\nTYPE TDot\n  C AS TDotInner\nEND TYPE\nDIM A.B AS TDot\nA.B.C.D = 19\nPRINT A.B.C.D",
            ("19",),
        ),
    )
    for name, source, expected in dotted_scalar_cases:
        generated.append(
            AmbiguousIdCase(
                name,
                source,
                expected,
                "OP_ID_LD",
                0,
            )
        )

    dot_prefixes = ["A"]
    for _ in range(19):
        dot_prefixes.append(f"{dot_prefixes[-1]}.B")
    dot_values = [201 + idx for idx in range(len(dot_prefixes))]
    dot_sum = sum(dot_values)
    dot_checksum = sum(value * (idx * 2 + 1) for idx, value in enumerate(dot_values))
    dot_lines: list[str] = []
    for name, value in zip(dot_prefixes, dot_values, strict=True):
        dot_lines.append(f"{name} = {value}")
    dot_lines.extend(("SDot& = 0", "CDot& = 0"))
    for idx, name in enumerate(dot_prefixes):
        dot_lines.append(f"SDot& = SDot& + {name}")
        dot_lines.append(f"CDot& = CDot& + {name} * {idx * 2 + 1}")
    for name in dot_prefixes:
        dot_lines.append(f"PRINT {name}")
    dot_lines.extend(("PRINT SDot&", "PRINT CDot&"))
    generated.append(
        AmbiguousIdCase(
            "dotted_scalar_prefix_ladder_to_qb45_limit",
            "\n".join(dot_lines),
            tuple(str(value) for value in dot_values) + (str(dot_sum), str(dot_checksum)),
            "OP_ID_LD",
            0,
        )
    )

    return generated


def known_failures() -> list[KnownFailureCase]:
    return []


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_ambiguous_id_case_literal(case: AmbiguousIdCase) -> str:
    return (
        "AmbiguousIdCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_array(case.expected)},\n"
        f'    opcode: "{case.opcode}",\n'
        f"    et: {case.et},\n"
        "}"
    )


def render_known_failure_case_literal(case: KnownFailureCase) -> str:
    return (
        "KnownFailureCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f'    reason: "{case.reason}",\n'
        "}"
    )


def main() -> None:
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct AmbiguousIdCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "    pub opcode: &'static str,",
            "    pub et: u8,",
            "}",
            "",
            "#[allow(dead_code)]",
            "#[derive(Debug, Clone, Copy)]",
            "pub struct KnownFailureCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub reason: &'static str,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_ambiguous_identifier_cases",
            "AmbiguousIdCase",
            [(case.name, render_ambiguous_id_case_literal(case)) for case in cases()],
            "super::assert_ambiguous_id_case(case);",
        ),
        render_rstest_function(
            "generated_ambiguous_identifier_known_failures",
            "KnownFailureCase",
            [(case.name, render_known_failure_case_literal(case)) for case in known_failures()],
            "super::assert_ambiguous_id_known_failure(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_ambiguous_id_cases.py",
            struct_block,
            rstest_blocks,
        )
    )


if __name__ == "__main__":
    main()
