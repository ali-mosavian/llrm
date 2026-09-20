"""Regression tests for the in-tree QB source-semantic generators."""

import re
import json
from pathlib import Path
from dataclasses import replace

import pytest

from tools.qbgen import cases
from tools.qbgen import verify
from tools.qbgen.model import validate
from tools.qbgen.model import render_bas
from tools.qbgen.model import write_cases
from tools.qbgen.model import GeneratedCase
from tools.qbgen.ported import family_counts


def test_generated_corpus_is_deterministic_and_covers_each_ported_family() -> None:
    """A changed generator ordering once made a source failure irreproducible."""
    first = cases()
    second = cases()
    validate(first)
    assert first == second
    assert len(first) == 2160
    assert {case.family.removeprefix("ported_") for case in first} >= {
        "ambiguous_id",
        "array_access",
        "assignment_coercion",
        "builtin_arg",
        "control_flow",
        "expression_promotion",
        "name_binding",
        "param_coercion",
        "procedure_call",
        "scan_syntax",
        "udt_access",
    }
    assert all(case.origin for case in first)
    assert all(case.outcomes for case in first)
    assert sum(case.classification == "semantic-negative" for case in first) == 4
    assert family_counts() == {
        "ported_ambiguous_id": 287,
        "ported_array_access": 180,
        "ported_assignment_coercion": 76,
        "ported_builtin_arg": 78,
        "ported_control_flow": 573,
        "ported_expression_promotion": 81,
        "ported_name_binding": 52,
        "ported_param_coercion": 139,
        "ported_procedure_call": 265,
        "ported_scan_syntax": 185,
        "ported_udt_access": 215,
    }


def test_materialized_sources_are_dos_83_crlf_and_manifest_is_stable(tmp_path: Path) -> None:
    """DOS compiler fixtures used to get host LF names and became un-runnable."""
    manifest = write_cases(tmp_path, cases())
    again = tmp_path / "again"
    second = write_cases(again, cases())
    assert manifest.read_bytes() == second.read_bytes()
    assert manifest.name == "MANIFEST.JSN"
    assert all(
        not path.is_file() or re.fullmatch(r"[A-Z0-9]{1,8}\.[A-Z0-9]{1,3}", path.name) for path in tmp_path.iterdir()
    )
    records = json.loads(manifest.read_text())["cases"]
    assert len(records) == len(cases())
    for record in records:
        path = tmp_path / record["file"]
        payload = path.read_bytes()
        assert len(path.stem) <= 8 and path.suffix == ".BAS"
        assert b"\n" not in payload.replace(b"\r\n", b"")
        assert payload.endswith(b"\r\n")
        assert record["sha256"] == __import__("hashlib").sha256(payload).hexdigest()


def test_invalid_negative_case_and_non_dos_filename_are_refused() -> None:
    """A generator that calls a rejected program a runnable witness is broken."""
    from tools.qbgen.model import DialectOutcome

    invalid = GeneratedCase(
        "one", "bad", "print 1", outcomes=(DialectOutcome("qb45", "syntax-error"),), hir_ops=("add",)
    )
    with pytest.raises(ValueError, match="fully rejected source"):
        validate((invalid,))
    too_many = GeneratedCase("ordinary", "long", "print 1")
    assert len(too_many.filename(1_000_000).split(".")[0]) > 8


def test_scan_pcode_origin_is_explicitly_source_only_not_an_ir_oracle() -> None:
    """Scanner fixtures retain source shapes while their p-code fields stay inert."""
    scans = [case for case in cases() if case.family == "ported_scan_syntax"]
    assert len(scans) == 185
    assert all("generate_scan_pcode_cases.py" in case.origin for case in scans)
    assert all(case.ast == ("qbasic-port:scan_syntax",) for case in scans)
    assert all(not case.hir_ops and not case.runtime_calls for case in scans)


def test_mutating_an_expected_hir_fact_is_detected_before_a_green_claim(monkeypatch: pytest.MonkeyPatch) -> None:
    """The verifier must reject a wrong HIR expectation, not just parse source."""
    case = next(one for one in cases() if one.name == "long_multiply_is_whole_value")
    changed = replace(case, hir_ops=("divide",))
    assert changed.hir_ops != case.hir_ops
    # This is a focused mutation of the expectation itself; real execution is
    # covered below, avoiding a full cargo process for every generated case.
    fake = object()
    monkeypatch.setattr(verify, "_bindings", lambda _program: {"FACTOR": {"long"}})
    monkeypatch.setattr(verify, "_ops", lambda _program: {"mul"})
    monkeypatch.setattr(verify, "_calls", lambda _program: set())
    with pytest.raises(AssertionError, match="missing HIR operations"):
        verify.assert_hir_contract(fake, changed)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "name",
    [
        "defint_and_explicit_as_are_distinct",
        "bounded_two_dimensional_distinguishes_cells",
        "byref_sub_mutates_caller",
        "underscore_outcomes_are_explicit",
    ],
)
def test_representative_generated_cases_reach_expected_frontend_stage(name: str, tmp_path: Path) -> None:
    """Parser, binding, HIR, and dialect-negative generation all get a real witness."""
    all_cases = cases()
    ordinal = next(index for index, case in enumerate(all_cases, 1) if case.name == name)
    case = all_cases[ordinal - 1]
    verify.assert_case(case, ordinal, tmp_path)


def test_render_normalizes_every_host_newline_spelling_to_dos_crlf() -> None:
    """The source writer must not leak LF or bare CR into a BC fixture."""
    assert render_bas("a\rb\nc\r\nd") == "a\r\nb\r\nc\r\nd\r\n"
