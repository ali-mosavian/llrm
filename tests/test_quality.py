"""The code-quality report must measure emitted code and distrust its targets."""

import json
import hashlib
from pathlib import Path
from types import SimpleNamespace

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
        assert function["comparison"]["instructions"] < function["instructions"]
        assert function["peak_live_values"] >= 0
        assert function["rematerializations"] >= 0
        assert function["dynamic_operations"] is not None
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


def test_machine_cse_reuses_c_nbody_frame_addresses() -> None:
    """C nbody recomputed two unchanged frame bases in its hot pair loop.

    Allocation leaves ``lea bx,[bp-100]`` live across the floating update,
    then lowering emitted the identical LEA again before the store; the
    ``bp-132`` velocity array did the same.  This is a post-allocation
    redundancy, not a MIR expression.  Keep the real benchmark as the
    symptom while allowing a future selector to fold either address away
    completely.
    """
    source = CORPUS / "nbody.c"
    stream = cfront.recorded(source, [])
    assembly = cfront.compiled(stream, "nbody_machine_cse", optimise=True)
    blocks: list[list[str]] = []
    for line in assembly.splitlines():
        if line.startswith("L") and line.endswith(":"):
            blocks.append([])
        if blocks:
            blocks[-1].append(line)
    rendered = ["\n".join(lines) for lines in blocks]
    candidates = [
        block
        for block in rendered
        if all(displacement in block for displacement in ("[bp-100]", "[bp-132]"))
        and "fld qword ptr" in block
        and "fstp qword ptr" in block
    ]
    # Exact CFG peeling exposes one block per fixed interaction rather than
    # one shared pair loop.  The regression is address reuse within each hot
    # region; the number of regions is deliberately not part of it.
    assert candidates
    for hot in candidates:
        assert "fld qword ptr" in hot and "fstp qword ptr" in hot, hot
        for displacement in ("[bp-100]", "[bp-132]"):
            addresses = [line for line in hot.splitlines() if "lea " in line and displacement in line]
            assert len(addresses) <= 1, hot


def test_nbody_strength_reduction_respects_the_address_register_budget() -> None:
    """C nbody gave four sibling array addresses independent recurrences.

    The 16-bit address register file could not hold them with the surrounding
    loop state, so allocation inserted six reload/store pairs. Formula
    selection must carry their shared byte offset as one recurrence, while
    constant addresses exposed by unrolling fold into frame displacements.
    """
    source = CORPUS / "nbody.c"
    built = cfront.assembled(cfront.recorded(source, []), "nbody_strength_budget", optimise=True)
    body = built.procedures[0].body

    assert sum(one.spill_reload for one in body.insns) < 6
    assert sum(one.spill_store for one in body.insns) < 6


def test_shellsort_folds_indexed_frame_array_addresses() -> None:
    """C shellsort rematerialised its fixed local-array base in three hot blocks."""
    source = CORPUS / "shellsort.c"
    assembly = cfront.compiled(cfront.recorded(source, []), "shellsort_frame_index", optimise=True)
    frame_bases = [line for line in assembly.splitlines() if "lea " in line and "[bp-132]" in line]

    # Pointer recurrences initialize the array, walk values[i], and read the
    # final checksum. The three indexed insertion-sort accesses encode BP
    # directly and must not materialize additional copies of this base.
    assert len(frame_bases) <= 3, assembly
    assert sum("ss:[bp+" in line and "-132]" in line for line in assembly.splitlines()) >= 3, assembly


def test_reference_compilers_name_the_i686_gcc_not_the_host_gcc() -> None:
    assert quality.REFERENCE_COMPILERS == ("clang", "i686-elf-gcc")


def test_reference_contract_keeps_flat_i386_listings_advisory() -> None:
    """A flat GCC listing was once easy to read as an achievable medium-model
    result, silently ignoring far pointers and segment loads.

    The report must carry the ABI difference with the generated listing, not
    merely mention it in prose beside the command that produced it.
    """
    contract = quality.REFERENCE_CONTRACT

    assert contract["kind"] == "best-case-flat-i386-structural-reference"
    assert "16-bit medium-model" in contract["candidate_abi"]
    assert "Not an ABI-equivalent performance target" in contract["caveat"]


def test_report_revision_marks_a_dirty_worktree(monkeypatch: pytest.MonkeyPatch) -> None:
    """A performance run made with uncommitted optimizer fixes claimed its old HEAD.

    That made the report look reproducible from a revision which did not contain
    the code it measured.  Dirty tracked input must be explicit in the recorded
    revision instead of silently inheriting the last commit's identity.
    """
    results = iter(
        [
            SimpleNamespace(returncode=0, stdout="0123456789abcdef\n"),
            SimpleNamespace(returncode=0, stdout=" M qbopt/optimize/fold.py\n"),
        ]
    )
    monkeypatch.setattr(quality.subprocess, "run", lambda *args, **kwargs: next(results))

    assert quality._revision() == "0123456789abcdef-dirty"


def test_reference_compilers_erase_watcom_memory_model_qualifiers(tmp_path: Path) -> None:
    """The installed i686 GCC rejected choose.c at its first ``near`` and
    quality.py silently recorded a failed reference instead of assembly.
    """
    compiler = "i686-elf-gcc"
    if quality.shutil.which(compiler) is None:
        pytest.skip(f"{compiler} is not installed")
    report = quality._reference(FIXTURES / "choose.c", compiler, tmp_path / "choose.s")
    assert report["status"] == "generated", report["diagnostic"]
    assert {"-Dnear=", "-Dfar=", "-Dhuge=", "-Dcdecl=", "-Dpascal="} <= set(report["flags"])
    assert {function["name"] for function in report["functions"]} == {"choose"}


@pytest.mark.parametrize("compiler", quality.REFERENCE_COMPILERS)
def test_reference_compilers_use_the_same_strict_floating_contract(tmp_path: Path, compiler: str) -> None:
    """GCC kept nbody's ``double`` temporaries in extended precision, so its
    apparent load/store advantage was a comparison against different numeric
    and exception semantics rather than a code-generation opportunity.
    """
    if quality.shutil.which(compiler) is None:
        pytest.skip(f"{compiler} is not installed")
    report = quality._reference(CORPUS / "nbody.c", compiler, tmp_path / f"nbody-{compiler}.s")
    assert report["status"] == "generated", report["diagnostic"]
    assert {
        "-frounding-math",
        "-ftrapping-math",
        "-fexcess-precision=standard",
        "-ffp-contract=off",
    } <= set(report["flags"])


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
            "comparison": {
                "instructions": 5,
                "loads": 2,
                "stores": 1,
                "branches": 1,
                "calls": 1,
                "address_calculations": 0,
            },
            "dynamic_operations": None,
            "dynamic_status": "unmeasured: call, interrupt, or repeated instruction hides executed work",
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


def test_reference_dynamic_operations_count_a_natural_loop() -> None:
    """CRC's unrolled GCC body looked 7.7x worse because only static work was compared."""
    assembly = """
        .type bench_loop, @function
    bench_loop:
        mov ecx, 10
    .Lagain:
        add eax, 1
        dec ecx
        jne .Lagain
        ret
        .size bench_loop, .-bench_loop
    """

    (function,) = quality._reference_functions(assembly)

    assert function["dynamic_operations"] == 32.0
    assert function["dynamic_status"] == "estimated: CFG branches and ten iterations per natural loop"


def test_dynamic_frequency_uses_a_proven_fixed_trip_count() -> None:
    """Nbody's fixed C loops were each charged ten trips after GCC unrolled them.

    A surviving canonical loop with an exact MIR trip proof has a stronger
    fact than the fallback profile.  Its header must execute precisely that
    many times in the dynamic structural estimate.
    """
    from qbopt.model import lir

    body = lir.LirBody(
        "fixed",
        0,
        (
            lir.LirBlock(0, (), (1,)),
            lir.LirBlock(1, (), (2,)),
            lir.LirBlock(2, (), (1, 3)),
            lir.LirBlock(3, (), ()),
        ),
        {},
        {},
        loop_trip_counts=((1, 4),),
    )

    assert quality._frequencies(body)[1] == 4


@pytest.mark.parametrize("mnemonic", ["fld", "fsubr", "cmp", "push"])
def test_reference_memory_sources_are_not_counted_as_stores(mnemonic: str) -> None:
    """nbody's x87 memory operands made the reference report more stores than instructions."""
    assert quality._reference_memory(mnemonic, "qword ptr [esp+4]") == (1, 0)


def test_reference_comparison_excludes_abi_frame_scaffolding() -> None:
    """Shellsort's LIR/body comparison was ``unmapped`` by prologue instructions."""
    assembly = """
        .type bench_one, @function
        bench_one:
        push ebp
        mov ebp, esp
        push esi
        sub esp, 16
        mov eax, DWORD PTR [ebp+8]
        add eax, 1
        add esp, 16
        pop esi
        pop ebp
        ret
        .Lcold:
        xor ecx, ecx
        jmp .Ljoin
        .size bench_one, .-bench_one
    """

    function = quality._reference_functions(assembly)[0]

    assert function["instructions"] == 12, "the raw report must retain the exact emitted total"
    assert function["comparison"] == {
        "instructions": 5,
        "loads": 1,
        "stores": 0,
        "branches": 1,
        "calls": 0,
        "address_calculations": 0,
    }


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
                    "dynamic_operations": 120.0,
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
                    "dynamic_operations": 100.0,
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
        "dynamic_operations": 1.2,
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


def test_gap_attribution_ignores_rejected_structural_candidate_stages() -> None:
    """Matmul's rejected peel looked like rotate restored 41 loads and 64 stores.

    A candidate is measured so its decision remains auditable, but it never
    became production code.  Its locally optimized counts must not move the
    first production stage blamed for an emitted excess.
    """
    stages = [
        {"stage": "mir-r01-promote", "loads": 8},
        {"stage": "mir-candidate-peel-r01-promote", "loads": 2, "tentative": True},
        {"stage": "mir-rotate", "loads": 8},
    ]
    assert quality._first_excess_stage(stages, "loads", 5) == "mir-r01-promote"


@pytest.mark.parametrize(
    ("stage", "tentative"),
    [
        ("mir-candidate-peel-r01-promote", True),
        ("mir-candidate-peel-unroll-accepted", True),
        ("mir-peel-candidate", True),
        ("mir-unroll-candidate", True),
        ("mir-peel-rejected-residual-loops", True),
        ("mir-unroll-rejected-growth", True),
        ("mir-peel-accepted", False),
        ("mir-r01-promote", False),
    ],
)
def test_structural_candidate_stage_names_record_transaction_state(stage: str, tentative: bool) -> None:
    assert quality._tentative_stage(stage) is tentative


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


def test_386_cost_covers_integer_and_x87_forms_in_emitted_c() -> None:
    """One ordinary load made every 386 corpus cost null; x87 was priced as generic work elsewhere."""
    from qbopt.backend import cpu

    rows = [
        ("668b4606", "mov", "eax,[bp+6]"),
        ("668346f418", "add", "dword ptr [bp-0ch],18h"),
        ("dd46f4", "fld", "qword ptr [bp-0ch]"),
        ("d80e0000", "fmul", "dword ptr ds:[0]"),
        ("dd5ef4", "fstp", "qword ptr [bp-0ch]"),
        ("c9", "leave", ""),
    ]
    assert quality._cost(rows, cpu.profile("386")) == 69


@pytest.mark.parametrize("name", ["486", "P5", "P6", "K5", "K6", "K7", "Core"])
def test_every_cpu_profile_prices_emitted_integer_and_x87_forms(name: str) -> None:
    """Every floating C benchmark reported a null cost outside the 386 profile."""
    from qbopt.backend import cpu

    rows = [
        ("dd46f4", "fld", "qword ptr [bp-0ch]"),
        ("dec1", "faddp", "st(1),st"),
        ("dc0e0000", "fmul", "qword ptr ds:[0]"),
        ("def9", "fdivp", "st(1),st"),
        ("df7ef4", "fistp", "qword ptr [bp-0ch]"),
        ("d97ef2", "fnstcw", "word ptr [bp-0eh]"),
        ("d96ef2", "fldcw", "word ptr [bp-0eh]"),
        ("c9", "leave", ""),
    ]

    assert quality._cost(rows, cpu.profile(name)) is not None


def test_x87_stack_dependencies_are_not_scored_as_parallel_work() -> None:
    """A dependent P6 expression was priced as only its slowest x87 instruction."""
    from qbopt.backend import cpu

    rows = [
        ("dd46f4", "fld", "qword ptr [bp-0ch]"),
        ("dec1", "faddp", "st(1),st"),
        ("dc0e0000", "fmul", "qword ptr ds:[0]"),
        ("def9", "fdivp", "st(1),st"),
        ("df7ef4", "fistp", "qword ptr [bp-0ch]"),
    ]

    assert quality._cost(rows, cpu.profile("P6")) == 75


def test_unknown_instruction_has_no_invented_default_cost() -> None:
    """An unclassified instruction used to receive two cycles on every non-386 target."""
    from qbopt.backend import cpu

    assert quality._cost([("0f0b", "ud2", "")], cpu.profile("P5")) is None


def test_unpriced_repeated_instruction_names_the_missing_cost_form() -> None:
    """Sieve's null weighted cost did not say that runtime-counted REP was unpriced."""
    from qbopt.backend import cpu

    rows = [("f3aa", "rep stosb", "")]
    cost, status, forms = quality._cost_report(rows, cpu.profile("386"))
    assert cost is None
    assert status == "unpriced: rep_string"
    assert forms == ("rep_string",)


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


def test_stage_metrics_do_not_count_control_word_store_as_a_load() -> None:
    """C floats reported two unmapped loads because FNSTCW was treated as RMW."""
    from qbopt.model import ir
    from qbopt.model import lir

    cell = ir.Mem(None, 2)
    what = ir.Semantics(ir.Operation.BARRIER, "fnstcw", (cell,), ())
    insn = lir.Insn(0, (0, 3), what, (), ())
    body = lir.LirBody("control-store", 0, (lir.LirBlock(0, (insn,)),), {}, {})

    assert quality._stage_metrics(body)["loads"] == 0


def test_stage_metrics_count_rematerialized_instructions() -> None:
    """A reconstructed value was reported as null even after the allocator made it."""
    from qbopt.model import ir
    from qbopt.model import lir

    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Imm(20, 2),))
    insn = lir.Insn(0, (0, 0), what, (1,), (), rematerialized=True)
    body = lir.LirBody("remat", 0, (lir.LirBlock(0, (insn,)),), {}, {})
    assert quality._stage_metrics(body)["rematerializations"] == 1


def test_dynamic_frequencies_account_for_branches_and_loop_iterations() -> None:
    """A static instruction count cannot stand in for hot-path execution."""
    from qbopt.model import lir

    blocks = (
        lir.LirBlock(0, (), succ=(1,)),
        lir.LirBlock(1, (), succ=(2, 3)),
        lir.LirBlock(2, (), succ=(1,)),
        lir.LirBlock(3, (), succ=()),
    )
    body = lir.LirBody("loop", 0, blocks, {}, {})
    assert quality._frequencies(body) == pytest.approx({0: 1.0, 1: 10.0, 2: 9.0, 3: 1.0})


def test_dynamic_frequencies_solve_nested_loops_without_iteration_cutoff() -> None:
    """C nbody's four nested loops converged too slowly and were reported unmeasured."""
    from qbopt.model import lir

    edges = {
        1: (35,),
        35: (40, 180),
        40: (42,),
        42: (46, 140),
        46: (50,),
        50: (54, 135),
        54: (50,),
        135: (42,),
        140: (142,),
        142: (146, 175),
        146: (142,),
        175: (35,),
        180: (),
    }
    body = lir.LirBody("nested", 1, tuple(lir.LirBlock(at, (), succ) for at, succ in edges.items()), {}, {})
    frequencies = quality._frequencies(body)
    assert frequencies is not None
    assert frequencies == pytest.approx(
        {1: 1, 35: 10, 40: 9, 42: 90, 46: 81, 50: 810, 54: 729, 135: 81, 140: 9, 142: 90, 146: 81, 175: 9, 180: 1}
    )


def test_dynamic_estimate_refuses_hidden_callee_cost() -> None:
    """Counting CALL as one instruction made an arbitrarily expensive helper look free."""
    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import masm

    call = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.CALL, "call"), (), ())
    ret = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.RETURN, "ret"), (), ())
    body = lir.LirBody("caller", 0, (lir.LirBlock(0, (call, ret)),), {}, {})
    procedure = masm.Procedure("caller", False, False, body, 0, {0: masm.Callee("helper", False)})
    module = masm.Module("CODE", {}, (("helper", "near"),), (), (), (procedure,))
    estimate, status = quality._dynamic_operations(module, procedure, 0)
    assert estimate is None
    assert status == "unmeasured: call or interrupt hides executed work"


def test_dynamic_estimate_refuses_an_unmapped_block(monkeypatch: pytest.MonkeyPatch) -> None:
    """A CFG block absent from the byte layout must not silently cost zero."""
    from qbopt.model import lir
    from qbopt.backend import masm

    body = lir.LirBody("lost", 7, (lir.LirBlock(7, ()),), {}, {})
    procedure = masm.Procedure("lost", False, False, body, 0, {})
    module = masm.Module("CODE", {}, (), (), (), (procedure,))
    monkeypatch.setattr(quality, "_image", lambda *_args: (b"\x90", {}))
    with pytest.raises(quality.InvalidMeasurement, match="no label for block 0x7"):
        quality._dynamic_operations(module, procedure, 0)
