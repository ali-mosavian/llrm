"""Mechanical adapter for qbasic-port's 2,131 source generator cases.

The original scripts are vendored alongside this module, rather than imported
from ``~/work``.  They still construct precisely the same source matrices;
this adapter intentionally discards their p-code opcode oracle.  It retains
the source and expected DOS text so qbopt can assert its own source contracts.
"""

from __future__ import annotations

import runpy
from typing import Any
from pathlib import Path
from collections.abc import Iterable

from tools.qbgen.model import GeneratedCase
from tools.qbgen.model import DialectOutcome

ROOT = Path(__file__).with_name("qbasic_port")


def _loaded(name: str) -> dict[str, Any]:
    return runpy.run_path(ROOT / name)


def _items(namespace: dict[str, Any], filename: str) -> Iterable[Any]:
    if filename == "generate_assignment_coercion_cases.py":
        return tuple(
            item
            for function in ("scalar_cases", "array_cases", "udt_cases", "function_return_cases", "error_cases")
            for item in namespace[function]()
        )
    if filename == "generate_control_flow_cases.py":
        return tuple(
            item
            for function in ("pairwise_cases", "exit_unwind_cases", "goto_dispatch_cases", "known_failure_cases")
            for item in namespace[function]()
        )
    if filename == "generate_ambiguous_id_cases.py":
        return (*tuple(namespace["cases"]()), *tuple(namespace["known_failures"]()))
    if filename == "generate_param_coercion_cases.py":
        groups = namespace["all_cases"]()
        return tuple(item for group in groups.values() for item in group)
    return tuple(namespace["cases"]())


_FILES: tuple[tuple[str, str], ...] = (
    ("ambiguous_id", "generate_ambiguous_id_cases.py"),
    ("array_access", "generate_array_access_cases.py"),
    ("assignment_coercion", "generate_assignment_coercion_cases.py"),
    ("builtin_arg", "generate_builtin_arg_cases.py"),
    ("control_flow", "generate_control_flow_cases.py"),
    ("expression_promotion", "generate_expression_promotion_cases.py"),
    ("name_binding", "generate_name_binding_cases.py"),
    ("param_coercion", "generate_param_coercion_cases.py"),
    ("procedure_call", "generate_procedure_call_cases.py"),
    ("scan_syntax", "generate_scan_pcode_cases.py"),
    ("udt_access", "generate_udt_access_cases.py"),
)


def cases() -> tuple[GeneratedCase, ...]:
    """Return all 2,131 ported source cases in original family order."""
    generated: list[GeneratedCase] = []
    for family, filename in _FILES:
        for item in _items(_loaded(filename), filename):
            reason = getattr(item, "known_failure_reason", None) or getattr(item, "reason", None)
            error_case = type(item).__name__ == "AssignmentErrorCase"
            outcomes = (
                (
                    DialectOutcome("qb45", "semantic-error"),
                    DialectOutcome("pds71", "semantic-error"),
                    DialectOutcome("vbdos", "semantic-error"),
                )
                if error_case
                else (DialectOutcome("qb45"), DialectOutcome("pds71"), DialectOutcome("vbdos"))
            )
            generated.append(
                GeneratedCase(
                    f"ported_{family}",
                    f"ported_{item.name}",
                    item.source,
                    outcomes=outcomes,
                    ast=(f"qbasic-port:{family}",),
                    expected_output=() if error_case or reason else tuple(getattr(item, "expected", ())),
                    classification=(
                        f"upstream-known-failure:{reason}"
                        if reason
                        else ("semantic-negative" if error_case else "ported-source")
                    ),
                    origin=f"qbasic-port/tools/{filename}",
                )
            )
    if len(generated) != 2131:
        raise AssertionError(f"ported qbasic-port corpus changed: expected 2131, got {len(generated)}")
    return tuple(generated)


def family_counts() -> dict[str, int]:
    """A testable inventory which detects an accidental matrix contraction."""
    found: dict[str, int] = {}
    for case in cases():
        found[case.family] = found.get(case.family, 0) + 1
    return found
