"""The code-quality report must measure emitted code and distrust its targets."""

import json
import hashlib
from pathlib import Path

import pytest

from tools import quality
from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"
CORPUS = Path(__file__).resolve().parents[1] / "bench" / "c"


def _report(cpu: str = "386") -> dict:
    source = FIXTURES / "halve.cgs"
    built = cfront.assembled(source.read_text(), source.stem, optimise=True, cpu=cpu)
    return quality.module_report(built, source, cpu)


def test_report_measures_each_emitted_function_and_names_its_profile() -> None:
    report = _report("P5")
    assert report["cpu"] == "P5"
    assert report["source_sha256"] == hashlib.sha256((FIXTURES / "halve.cgs").read_bytes()).hexdigest()
    assert {one["name"] for one in report["functions"]} == {"_half", "_eighth", "_gaps"}
    for function in report["functions"]:
        assert function["bytes"] > 0
        assert function["instructions"] > 0
        assert function["peak_live_values"] >= 0
        assert function["target_status"] == "missing"
        assert function["ratio"] is None


def test_report_refuses_a_target_derived_from_its_own_output() -> None:
    report = _report()
    function = report["functions"][0]
    target = {
        "audited": True,
        "evidence": "copied from the candidate",
        "assembly_sha256": function["assembly_sha256"],
        "metrics": {"bytes": function["bytes"]},
    }
    with pytest.raises(quality.UntrustedTarget, match="candidate's own assembly"):
        quality.apply_target(function, target)


@pytest.mark.parametrize(("code", "message"), [(b"", "no bytes"), (b"\x0f", "invalid instruction")])
def test_report_refuses_incomplete_function_extent(code: bytes, message: str) -> None:
    with pytest.raises(quality.InvalidMeasurement, match=message):
        quality._rows(code)


@pytest.mark.parametrize("target", [{}, {"audited": False}, {"audited": True, "evidence": ""}])
def test_report_refuses_incomplete_target_evidence(target: dict) -> None:
    function = _report()["functions"][0]
    with pytest.raises(quality.UntrustedTarget):
        quality.apply_target(function, target)


def test_corpus_has_independent_inputs_and_canonical_crc() -> None:
    expected = json.loads((CORPUS / "expected.json").read_text())
    assert set(expected) == {"sieve", "crc", "matmul", "mandel", "shellsort", "floats", "nbody"}
    assert expected["crc"] == {"arguments": [0], "result": 0xCBF43926}
    for name, oracle in expected.items():
        assert (CORPUS / f"{name}.c").is_file()
        assert oracle["arguments"]
        assert isinstance(oracle["result"], int)


def test_reference_compilers_name_the_i686_gcc_not_the_host_gcc() -> None:
    assert quality.REFERENCE_COMPILERS == ("clang", "i686-elf-gcc")
