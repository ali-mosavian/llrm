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


def test_reference_assembly_is_measured_per_function_without_directives_or_comments() -> None:
    """Saving GCC's assembly path alone left every structural comparison manual."""
    assembly = """
        .type bench_one, @function
    bench_one:
        mov eax, DWORD PTR [esp+4] # load the argument
        add DWORD PTR [esp+8], eax
        jne .Lagain
        call helper
        ret
        .size bench_one, .-bench_one
    """
    assert quality._reference_functions(assembly) == [
        {
            "name": "bench_one",
            "instructions": 5,
            "loads": 2,
            "stores": 1,
            "branches": 1,
            "calls": 1,
            "address_calculations": 0,
            "normalized_sha256": quality._normalized_hash(
                (
                    "mov eax, dword ptr [esp+4]",
                    "add dword ptr [esp+8], eax",
                    "jne .lagain",
                    "call helper",
                    "ret",
                )
            ),
        }
    ]


@pytest.mark.parametrize("mnemonic", ["fld", "fsubr", "cmp", "push"])
def test_reference_memory_sources_are_not_counted_as_stores(mnemonic: str) -> None:
    """nbody's x87 memory operands made the reference report more stores than instructions."""
    assert quality._reference_memory(mnemonic, "qword ptr [esp+4]") == (1, 0)


def test_structural_comparison_matches_c_and_medium_model_symbol_spellings() -> None:
    reports = [
        {
            "source": "bench/c/sieve.c",
            "cpu": "386",
            "functions": [
                {
                    "name": "_bench_sieve",
                    "instructions": 30,
                    "loads": 8,
                    "stores": 4,
                    "branches": 3,
                    "calls": 0,
                    "address_calculations": 2,
                }
            ],
        }
    ]
    references = [
        {
            "source": "bench/c/sieve.c",
            "compiler": "i686-elf-gcc",
            "assembly": "build/sieve-gcc.s",
            "functions": [
                {
                    "name": "bench_sieve",
                    "instructions": 20,
                    "loads": 5,
                    "stores": 4,
                    "branches": 2,
                    "calls": 0,
                    "address_calculations": 1,
                }
            ],
        }
    ]
    comparison = quality._comparisons(reports, references)[0]
    assert comparison["function"] == "bench_sieve"
    assert comparison["ratios"] == {
        "instructions": 1.5,
        "loads": 1.6,
        "stores": 1.0,
        "branches": 1.5,
        "calls": None,
        "address_calculations": 2.0,
    }


def test_gap_attribution_names_the_first_stage_after_which_excess_stays() -> None:
    """A temporary excess is not the stage responsible for the emitted gap."""
    stages = [
        {"stage": "lir-lower", "loads": 8},
        {"stage": "lir-coalesce", "loads": 4},
        {"stage": "lir-regalloc", "loads": 7},
        {"stage": "lir-peephole", "loads": 6},
    ]
    assert quality._first_excess_stage(stages, "loads", 5) == "lir-regalloc"
    assert quality._first_excess_stage(stages, "loads", 7) is None


def test_stage_metrics_do_not_count_non_emitting_lir_markers() -> None:
    """nbody's final LIR reported 286 instructions for the 204 actually emitted."""
    from qbopt.model import ir
    from qbopt.model import lir

    live = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.MOVE, "mov"), (), ())
    marker = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.NOTHING, ""), (), ())
    body = lir.LirBody("markers", 0, (lir.LirBlock(0, (live, marker)),), {}, {})
    assert quality._stage_metrics(body)["instructions"] == 1


def test_gap_attribution_refuses_a_stage_measure_that_disagrees_with_emitted_bytes() -> None:
    """nbody's LIR counted 99 loads where decoding the emitted bytes counted 62."""
    stages = [
        {"stage": "lir-lower", "form": "lir", "loads": 70},
        {"stage": "lir-layout", "form": "lir", "loads": 99},
    ]
    assert quality._gap_attribution(stages, "loads", reference=44, emitted=62) == {
        "status": "unmapped",
        "stage": None,
        "last_stage": 99,
        "emitted": 62,
    }


def test_emitted_memory_metrics_cover_forms_missing_from_the_cycle_table() -> None:
    """MOVZX memory loads vanished from nbody's decoded load total."""
    rows = [
        ("0fb604", "movzx", "eax,byte ptr [si]"),
        ("833e000000", "cmp", "word ptr [0],0"),
        ("dd1e0000", "fstp", "qword ptr [0]"),
        ("01060000", "add", "word ptr [0],ax"),
    ]
    assert quality._memory(rows) == (3, 2)


def test_stage_metrics_count_a_two_address_memory_operand_once() -> None:
    """The same RMW cell is both a semantic source and destination, but one load."""
    from qbopt.model import ir
    from qbopt.model import lir

    cell = ir.Mem(None, 2)
    what = ir.Semantics(ir.Operation.BINARY, "add", (cell,), (cell, ir.Held(1, 2)))
    insn = lir.Insn(0, (0, 2), what, (), (1,))
    body = lir.LirBody("rmw", 0, (lir.LirBlock(0, (insn,)),), {}, {})
    metrics = quality._stage_metrics(body)
    assert (metrics["loads"], metrics["stores"]) == (1, 1)
