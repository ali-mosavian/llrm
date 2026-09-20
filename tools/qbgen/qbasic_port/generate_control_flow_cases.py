#!/usr/bin/env python3
"""Generate deterministic QBasic control-flow integration cases."""

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
OUTPUT = ROOT / "tests" / "generated" / "control_flow_cases.rs"
MAX_DEPTH = 10


@dataclass(frozen=True, slots=True)
class Case:
    family: str
    name: str
    source: str
    expected: tuple[str, ...]
    max_steps: int | None = 20_000
    known_failure_reason: str | None = None


@dataclass(frozen=True, slots=True)
class Container:
    name: str
    multiplier: int
    open_lines: tuple[str, ...]
    close_lines: tuple[str, ...]


def indent(lines: list[str], width: int = 2) -> list[str]:
    prefix = " " * width
    return [f"{prefix}{line}" if line else line for line in lines]


def container_templates(depth: int) -> list[Container]:
    suffix = str(depth)
    return [
        Container(
            name="if_true",
            multiplier=1,
            open_lines=("IF -1 THEN",),
            close_lines=("END IF",),
        ),
        Container(
            name="if_else",
            multiplier=1,
            open_lines=(
                "IF 0 THEN",
                "  total = total + 10000",
                "ELSE",
            ),
            close_lines=("END IF",),
        ),
        Container(
            name="if_elseif",
            multiplier=1,
            open_lines=(
                "IF 0 THEN",
                "  total = total + 10000",
                "ELSEIF -1 THEN",
            ),
            close_lines=(
                "ELSE",
                "  total = total + 10000",
                "END IF",
            ),
        ),
        Container(
            name="if_string_compare",
            multiplier=1,
            open_lines=(
                f'i{suffix}$ = "yes"',
                f'IF i{suffix}$ = "yes" THEN',
            ),
            close_lines=("END IF",),
        ),
        Container(
            name="for_int_positive_twice",
            multiplier=2,
            open_lines=(f"FOR fp{suffix} = 1 TO 2",),
            close_lines=(f"NEXT fp{suffix}",),
        ),
        Container(
            name="for_int_negative_twice",
            multiplier=2,
            open_lines=(f"FOR fn{suffix} = 2 TO 1 STEP -1",),
            close_lines=(f"NEXT fn{suffix}",),
        ),
        Container(
            name="for_single_fractional_three",
            multiplier=3,
            open_lines=(f"FOR fs{suffix}# = 0 TO 1 STEP .5",),
            close_lines=(f"NEXT fs{suffix}#",),
        ),
        Container(
            name="do_until_twice",
            multiplier=2,
            open_lines=(
                f"d{suffix} = 0",
                "DO",
                f"  d{suffix} = d{suffix} + 1",
            ),
            close_lines=(f"LOOP UNTIL d{suffix} = 2",),
        ),
        Container(
            name="do_while_twice",
            multiplier=2,
            open_lines=(
                f"dw{suffix} = 0",
                "DO",
                f"  dw{suffix} = dw{suffix} + 1",
            ),
            close_lines=(f"LOOP WHILE dw{suffix} < 2",),
        ),
        Container(
            name="do_until_pretest_twice",
            multiplier=2,
            open_lines=(
                f"du{suffix} = 0",
                f"DO UNTIL du{suffix} = 2",
                f"  du{suffix} = du{suffix} + 1",
            ),
            close_lines=("LOOP",),
        ),
        Container(
            name="while_twice",
            multiplier=2,
            open_lines=(
                f"w{suffix} = 0",
                f"WHILE w{suffix} < 2",
                f"  w{suffix} = w{suffix} + 1",
            ),
            close_lines=("WEND",),
        ),
        Container(
            name="while_expression_twice",
            multiplier=2,
            open_lines=(
                f"we{suffix} = 0",
                f"WHILE we{suffix} + 1 <= 2",
                f"  we{suffix} = we{suffix} + 1",
            ),
            close_lines=("WEND",),
        ),
        Container(
            name="select_numeric_eq",
            multiplier=1,
            open_lines=(
                f"s{suffix} = 2",
                f"SELECT CASE s{suffix}",
                "CASE 1",
                "  total = total + 10000",
                "CASE 2",
            ),
            close_lines=(
                "CASE ELSE",
                "  total = total + 10000",
                "END SELECT",
            ),
        ),
        Container(
            name="select_numeric_range",
            multiplier=1,
            open_lines=(
                f"sr{suffix} = 3",
                f"SELECT CASE sr{suffix}",
                "CASE 1",
                "  total = total + 10000",
                "CASE 2 TO 4",
            ),
            close_lines=(
                "CASE ELSE",
                "  total = total + 10000",
                "END SELECT",
            ),
        ),
        Container(
            name="select_string_eq",
            multiplier=1,
            open_lines=(
                f'se{suffix}$ = "b"',
                f"SELECT CASE se{suffix}$",
                'CASE "a"',
                "  total = total + 10000",
                'CASE "b"',
            ),
            close_lines=(
                "CASE ELSE",
                "  total = total + 10000",
                "END SELECT",
            ),
        ),
        Container(
            name="select_string_range",
            multiplier=1,
            open_lines=(
                f'st{suffix}$ = "5"',
                f"SELECT CASE st{suffix}$",
                'CASE "a" TO "z"',
                "  total = total + 10000",
                'CASE "0" TO "9"',
            ),
            close_lines=(
                "CASE ELSE",
                "  total = total + 10000",
                "END SELECT",
            ),
        ),
        Container(
            name="gosub_wrapper",
            multiplier=1,
            open_lines=(
                f"GOSUB generated_sub_{suffix}",
                f"GOTO generated_after_sub_{suffix}",
                f"generated_sub_{suffix}:",
            ),
            close_lines=(
                "RETURN",
                f"generated_after_sub_{suffix}:",
            ),
        ),
    ]


def wrap(body: list[str], containers: list[Container]) -> list[str]:
    lines = body
    for container in reversed(containers):
        lines = [*container.open_lines, *indent(lines), *container.close_lines]
    return lines


def compose_case(name: str, containers: list[Container], family: str = "pairwise") -> Case:
    multiplier = 1
    for container in containers:
        multiplier *= container.multiplier
    body = wrap(["total = total + 1"], containers)
    source = "\n".join(["total = 0", *body, "PRINT total", 'PRINT "done"'])
    max_steps = max(20_000, multiplier * 200)
    return Case(
        family=family,
        name=name,
        source=source,
        expected=(str(multiplier), "done"),
        max_steps=max_steps,
    )


def template_by_name(depth: int) -> dict[str, Container]:
    return {template.name: template for template in container_templates(depth)}


def repeated_depth_cases() -> list[Case]:
    representative_names = [
        "if_true",
        "if_else",
        "if_elseif",
        "for_int_positive_twice",
        "for_int_negative_twice",
        "do_until_twice",
        "do_while_twice",
        "do_until_pretest_twice",
        "while_twice",
        "while_expression_twice",
        "select_numeric_eq",
        "select_numeric_range",
        "select_string_eq",
        "select_string_range",
        "gosub_wrapper",
    ]
    cases: list[Case] = []
    for template_name in representative_names:
        for depth in range(3, MAX_DEPTH + 1):
            layers = [template_by_name(level)[template_name] for level in range(1, depth + 1)]
            cases.append(compose_case(f"depth{depth}_repeated_{template_name}", layers))
    return cases


def mixed_replacement_depth_cases() -> list[Case]:
    variant_names = [template.name for template in container_templates(1)]
    cases: list[Case] = []
    for depth in range(3, MAX_DEPTH + 1):
        for offset in range(len(variant_names)):
            layers: list[Container] = []
            for level in range(1, depth + 1):
                template_name = variant_names[(offset + level - 1) % len(variant_names)]
                layers.append(template_by_name(level)[template_name])
            case_name = f"depth{depth}_mixed_offset_{offset}_{variant_names[offset]}"
            cases.append(compose_case(case_name, layers))
    return cases


def high_risk_depth10_cases() -> list[Case]:
    sequences = [
        (
            "depth10_for_select_if_repeated",
            [
                "for_int_positive_twice",
                "select_numeric_range",
                "if_elseif",
                "for_int_negative_twice",
                "select_string_range",
                "if_else",
                "do_until_twice",
                "while_twice",
                "select_numeric_eq",
                "if_true",
            ],
        ),
        (
            "depth10_select_for_do_while",
            [
                "select_string_eq",
                "for_int_positive_twice",
                "do_while_twice",
                "while_expression_twice",
                "select_numeric_range",
                "for_int_negative_twice",
                "do_until_pretest_twice",
                "if_string_compare",
                "select_string_range",
                "for_int_positive_twice",
            ],
        ),
        (
            "depth10_gosub_inside_structured_frames",
            [
                "for_int_positive_twice",
                "gosub_wrapper",
                "if_elseif",
                "select_numeric_eq",
                "gosub_wrapper",
                "do_until_twice",
                "while_twice",
                "gosub_wrapper",
                "if_else",
                "select_string_eq",
            ],
        ),
        (
            "depth10_fractional_for_mixed_once",
            [
                "for_single_fractional_three",
                "if_true",
                "select_numeric_eq",
                "do_until_twice",
                "while_twice",
                "for_int_positive_twice",
                "if_string_compare",
                "select_string_range",
                "do_until_pretest_twice",
                "for_int_negative_twice",
            ],
        ),
    ]

    cases: list[Case] = []
    for name, sequence in sequences:
        layers = [template_by_name(level)[template_name] for level, template_name in enumerate(sequence, start=1)]
        cases.append(compose_case(name, layers))
    return cases


def pairwise_cases() -> list[Case]:
    cases: list[Case] = []
    singles = container_templates(1)
    for outer in singles:
        cases.append(compose_case(f"depth1_{outer.name}", [outer]))

    for outer in container_templates(1):
        for inner in container_templates(2):
            cases.append(compose_case(f"depth2_{outer.name}_contains_{inner.name}", [outer, inner]))

    cases.extend(repeated_depth_cases())
    cases.extend(mixed_replacement_depth_cases())
    cases.extend(high_risk_depth10_cases())

    return cases


def exit_unwind_cases() -> list[Case]:
    return [
        Case(
            family="exit_unwind",
            name="exit_for_from_nested_if_leaves_outer_for_active",
            source="""
total = 0
FOR i = 1 TO 3
  FOR j = 1 TO 3
    IF j = 2 THEN
      EXIT FOR
    END IF
    total = total + i * 10 + j
  NEXT j
NEXT i
PRINT total
PRINT "done"
""".strip(),
            expected=("63", "done"),
        ),
        Case(
            family="exit_unwind",
            name="exit_do_from_nested_if_leaves_outer_for_active",
            source="""
total = 0
FOR i = 1 TO 3
  d = 0
  DO
    d = d + 1
    IF d = 2 THEN
      EXIT DO
    END IF
    total = total + i
  LOOP
NEXT i
PRINT total
PRINT "done"
""".strip(),
            expected=("6", "done"),
        ),
        Case(
            family="exit_unwind",
            name="exit_do_from_select_case_continues_outer_for",
            source="""
total = 0
FOR i = 1 TO 3
  d = 0
  DO
    d = d + 1
    SELECT CASE d
    CASE 1
      total = total + i
    CASE 2
      EXIT DO
    CASE ELSE
      total = total + 10000
    END SELECT
  LOOP
NEXT i
PRINT total
PRINT "done"
""".strip(),
            expected=("6", "done"),
        ),
        Case(
            family="exit_unwind",
            name="exit_for_from_select_case_unwinds_select_stack",
            source="""
total = 0
FOR i = 1 TO 5
  SELECT CASE i
  CASE 1, 2
    total = total + i
  CASE 3
    EXIT FOR
  CASE ELSE
    total = total + 100
  END SELECT
NEXT i
PRINT i
PRINT total
PRINT "done"
""".strip(),
            expected=("3", "3", "done"),
        ),
    ]


def goto_dispatch_cases() -> list[Case]:
    return [
        Case(
            family="goto_dispatch",
            name="on_goto_zero_falls_through_then_dispatches",
            source="""
FOR i = 0 TO 2
  ON i GOTO one, two
  PRINT "fall"; i
  GOTO continue
one:
  PRINT "one"
  GOTO continue
two:
  PRINT "two"
continue:
NEXT i
PRINT "done"
""".strip(),
            expected=("fall", "0", "one", "two", "done"),
        ),
        Case(
            family="goto_dispatch",
            name="gosub_inside_nested_if_for_returns_to_call_site",
            source="""
FOR i = 1 TO 2
  IF i = 1 THEN
    GOSUB helper
  ELSE
    PRINT "else"
  END IF
NEXT i
PRINT "done"
END
helper:
PRINT "helper"
RETURN
""".strip(),
            expected=("helper", "else", "done"),
        ),
        Case(
            family="goto_dispatch",
            name="goto_out_of_if_inside_for_continues_after_label",
            source="""
total = 0
FOR i = 1 TO 3
  IF i = 2 THEN
    GOTO skipped
  END IF
  total = total + i
skipped:
NEXT i
PRINT total
PRINT "done"
""".strip(),
            expected=("4", "done"),
        ),
    ]


def known_failure_cases() -> list[Case]:
    return []


def rust_string(value: str) -> str:
    return f'r###"{value}"###'


def rust_string_array(values: tuple[str, ...]) -> str:
    return "&[" + ", ".join(f'"{value}"' for value in values) + "]"


def render_control_flow_case_literal(case: Case) -> str:
    max_steps = "None" if case.max_steps is None else f"Some({case.max_steps})"
    reason = "None" if case.known_failure_reason is None else f'Some("{case.known_failure_reason}")'
    return (
        "ControlFlowCase {\n"
        f'    name: "{case.name}",\n'
        f"    source: {rust_string(case.source)},\n"
        f"    expected: {rust_string_array(case.expected)},\n"
        f"    max_steps: {max_steps},\n"
        f"    known_failure_reason: {reason},\n"
        "}"
    )


def main() -> None:
    groups = {
        "PAIRWISE_CASES": pairwise_cases(),
        "EXIT_UNWIND_CASES": exit_unwind_cases(),
        "GOTO_DISPATCH_CASES": goto_dispatch_cases(),
        "KNOWN_FAILURE_CASES": known_failure_cases(),
    }
    struct_block = "\n".join(
        [
            "#[derive(Debug, Clone, Copy)]",
            "pub struct ControlFlowCase {",
            "    pub name: &'static str,",
            "    pub source: &'static str,",
            "    pub expected: &'static [&'static str],",
            "    pub max_steps: Option<usize>,",
            "    pub known_failure_reason: Option<&'static str>,",
            "}",
        ]
    )
    rstest_blocks = [
        render_rstest_function(
            "generated_pairwise_control_flow_cases",
            "ControlFlowCase",
            [(case.name, render_control_flow_case_literal(case)) for case in groups["PAIRWISE_CASES"]],
            "super::assert_control_flow_passing_case(case);",
        ),
        render_rstest_function(
            "generated_exit_unwind_control_flow_cases",
            "ControlFlowCase",
            [(case.name, render_control_flow_case_literal(case)) for case in groups["EXIT_UNWIND_CASES"]],
            "super::assert_control_flow_passing_case(case);",
        ),
        render_rstest_function(
            "generated_goto_dispatch_control_flow_cases",
            "ControlFlowCase",
            [(case.name, render_control_flow_case_literal(case)) for case in groups["GOTO_DISPATCH_CASES"]],
            "super::assert_control_flow_passing_case(case);",
        ),
        render_rstest_function(
            "generated_control_flow_known_failures",
            "ControlFlowCase",
            [(case.name, render_control_flow_case_literal(case)) for case in groups["KNOWN_FAILURE_CASES"]],
            "super::assert_control_flow_known_failure(case);",
        ),
    ]
    OUTPUT.write_text(
        render_generated_file(
            "generate_control_flow_cases.py",
            struct_block,
            rstest_blocks,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
