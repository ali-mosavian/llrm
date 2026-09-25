"""
tools/opportunity.py's own gate: the board has to answer for our output.

It costed the parsed object for a while, which is BC's code and not ours.
Every ratio on the board then sat perfectly still no matter what a pass
did -- a reading that agreed with "there is nothing here" and was believed
for exactly as long as it took to notice hotlop had not moved after the
hoist started firing on it.

That is rule 2 in the project's own words, and the instrument that broke is
the one every other number is judged by, so it gets a test of its own.
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import opportunity


def test_jumps_reference_uses_three_iterations_and_all_six_output_rows():
    """JUMPS was priced as ten iterations although FOR k=1 TO 3 prints six rows."""
    left, right = 305419896, 252645135
    on = [left & right, left | right, left ^ right]
    cases = [left + right, left - right, -left]
    expected = [
        line
        for index, (one, case) in enumerate(zip(on, cases), 1)
        for line in (f"ON {index} ={one: d}", f"CASE {index} ={case: d}")
    ] + ["DONE"]
    assert Path("suite/golden/jumps.txt").read_text().splitlines() == expected
    assert opportunity.TRIPS.get("JUMPS") == 3
    stores = (2 + 4 + 6) * (2 + opportunity.TOUCH)
    output = (6 * 4 + 1) * (6 + opportunity.CALL) + opportunity.CALL
    assert opportunity.TARGETS.get("JUMPS") == stores + output == 742


def test_chain_reference_preserves_signed_remainders_and_output_states():
    """CHAIN's seven nested divide/remainder rows had no complete reference target."""

    def quotient(left, right):
        magnitude = abs(left) // abs(right)
        return -magnitude if (left < 0) != (right < 0) else magnitude

    def remainder(left, right):
        return left - quotient(left, right) * right

    numerator, divisor, outer = 1073741831, 39678839, 100003
    positive = remainder(remainder(numerator, divisor), outer)
    negative = remainder(remainder(-numerator, divisor), outer)
    results = [
        0,
        0,
        positive,
        positive,
        quotient(quotient(numerator, divisor), 3),
        negative,
        quotient(quotient(-numerator, divisor), 3),
    ]
    assert results == [0, 0, 13106, 13106, 9, -13106, -9]
    labels = ["ONE", "CONST", "CONST2", "MODMOD", "DIVDIV", "NEGMOD", "NEGDIV"]
    expected = [f"{label}={value: d}" for label, value in zip(labels, results)] + ["DONE"]
    assert Path("suite/golden/chain.txt").read_text().splitlines() == expected
    stores = (4 + 7 + 1) * (2 + opportunity.TOUCH)
    output = (7 * 2 + 1) * (6 + opportunity.CALL) + opportunity.CALL
    assert opportunity.TARGETS.get("CHAIN") == stores + output == 482


def test_divmod_reference_keeps_error_registration_resume_and_mutable_caught_state(capsys):
    """DIVMOD's old objects omitted nine rows; a plain print-only target also lost ON ERROR."""
    assert len(Path("suite/golden/divmod.txt").read_text().splitlines()) == 21
    error_registration = 2 * (2 + opportunity.TOUCH) + opportunity.CALL
    output = 20 * 2 * (2 + opportunity.TOUCH + opportunity.CALL)
    # caught=0 remains observable if printing MULOVF's label raises and
    # RESUME NEXT reaches the numeric print.  Store plus memory-push premium.
    caught_state = (2 + opportunity.TOUCH) + opportunity.TOUCH
    done = (2 + opportunity.TOUCH + opportunity.CALL) + opportunity.CALL
    handler = opportunity.CALL + (2 + opportunity.TOUCH) + opportunity.CALL
    assert opportunity.TARGETS["DIVMOD"] == error_registration + output + caught_state + done + handler == 1174
    assert opportunity.against_targets([Path("fixtures/omf/divmod-p-g2.obj")]) == 0
    report = capsys.readouterr().out
    assert "divmod-p-g2" in report and "PROVISIONAL" not in report


def test_fpemu_reference_folds_only_exact_exception_free_results(capsys):
    """FPEMU's positive square root and binary-power arithmetic have exact constant answers."""
    rows = Path("suite/golden/fpemu.txt").read_text().splitlines()
    assert rows[-2] == "FCMP=-1  0" and len(rows) == 13
    ordinary_rows = 11 * 2 * (2 + opportunity.TOUCH + opportunity.CALL)
    comparison_row = 3 * (2 + opportunity.TOUCH + opportunity.CALL)
    done = (2 + opportunity.TOUCH + opportunity.CALL) + opportunity.CALL
    assert opportunity.TARGETS["FPEMU"] == ordinary_rows + comparison_row + done == 696
    assert opportunity.against_targets([Path("fixtures/omf/fpemu-p-g2.obj")]) == 0
    report = capsys.readouterr().out
    assert "fpemu-p-g2" in report and "PROVISIONAL" not in report


def test_procs_reference_preserves_public_bodies_and_string_temporary_failures():
    """Whole-module folding may specialize calls, but public procedures and SASS/STDL remain."""
    # Main: three SASS/call/STDL sequences, each retaining a numeric byref
    # temporary, followed by DONE.  Generic TWICE and REPORT bodies retain
    # their BASIC ABI prologue/epilogue and are costed independently.
    main = 3 * 96 + 46
    twice = 63
    report = 136
    assert opportunity.TARGETS["PROCS"] == main + twice + report == 533


def test_non_benchmark_fixtures_are_explicitly_scoped_out(monkeypatch, capsys):
    """Format/debug fixtures must be neither missing targets nor silent benchmark members."""
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1))
    assert opportunity.against_targets([Path("byref2-q-O.obj")]) == 0
    report = capsys.readouterr().out
    assert "OUT OF SCOPE" in report and "CodeView" in report


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_chain_legacy_objects_cannot_pass_a_seven_row_reference(tag, capsys, tmp_path):
    """Five-row CHAIN fixtures must not look cheaper by omitting CONST and CONST2 output."""
    path = tmp_path / f"chain-{tag}.obj"
    path.write_bytes(Path(f"fixtures/regressions/chain5-{tag}.obj".lower()).read_bytes())
    assert opportunity.against_targets([path]) == 1
    report = capsys.readouterr().out
    assert "PROVISIONAL" in report and "seven-row" in report


@pytest.mark.parametrize("path", sorted(Path("fixtures/omf").glob("chain-*.obj")))
def test_chain_fixtures_cover_current_seven_result_source(path):
    """Legacy CHAIN objects omitted CONST and CONST2, hiding two output/arithmetic paths."""
    from qbopt.objectfile import module

    assert sum(name == "B$PEI4" for name in module.load(path).calls.values()) == 7


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_cmpord_has_a_complete_source_derived_reference(tag, capsys):
    """CMPORD's 24 signed-comparison rows had no reference despite constant answers."""
    assert opportunity.against_targets([Path(f"fixtures/omf/cmpord-{tag}.obj".lower())]) == 0
    report = capsys.readouterr().out
    assert "1918" in report and "1.00x" in report
    assert opportunity.TARGETS["CMPORD"] == 24 * 3 * (6 + 20) + (6 + 20) + 20


def test_subexp_reference_includes_all_stores_and_output_calls():
    """SUBEXP's old listing showed only arithmetic, leaving its 162-unit full target unauditable."""
    first = 11 | (5 << 16)
    second = ((11 + 5) * 2) | (((11 + 5) * 3) << 16)
    assert first.to_bytes(4, "little") == bytes.fromhex("0b000500")
    assert second.to_bytes(4, "little") == bytes.fromhex("20003000")
    stores = 2 * (2 + opportunity.TOUCH)
    output = 5 * (2 + opportunity.TOUCH + opportunity.CALL)
    assert opportunity.TARGETS["SUBEXP"] == stores + output + opportunity.CALL == 162


def test_flags_reference_preserves_states_at_each_output_call():
    """FLAGS lacked a target after its branches folded; observable numeric stores still cost work."""
    states = [(65535, 61680, 61680), (-65536, -65536, -65536), (0, 0, 0), (65536, 1, 65535)]
    assert [a & b for a, b, _ in states[:3]] == [r for _, _, r in states[:3]]
    assert states[-1][0] - states[-1][1] == states[-1][2]
    stores = len(states) * 3 * (2 + opportunity.TOUCH)
    outputs = 6 * (2 + opportunity.TOUCH + opportunity.CALL)
    assert opportunity.TARGETS.get("FLAGS") == stores + outputs + opportunity.CALL == 248
    assert "FLAGS" not in opportunity.PROVISIONAL_TARGETS


@pytest.mark.parametrize("tag,stores", [("p-g2", 9), ("q-O", 7), ("v-g3", 9)])
def test_fpcse_complete_reference_uses_object_compiler_identity(tag, stores, tmp_path, monkeypatch, capsys):
    """The old 98-unit FPCSE target omitted entry synchronization and observable numeric stores."""
    from collections import Counter

    path = tmp_path / "renamed.obj"
    path.write_bytes(Path(f"fixtures/omf/fpcse-{tag}.obj".lower()).read_bytes())
    target = (
        stores * (2 + opportunity.TOUCH) + opportunity.FLOAT["wait"] + 3 * (6 + opportunity.CALL) + opportunity.CALL
    )
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=target))
    assert opportunity.against_targets([path]) == 0
    report = capsys.readouterr().out
    assert "PROVISIONAL" not in report
    assert "1.00x" in report


def test_nbody_cost_includes_main_when_optimized_code_cannot_be_raised(tmp_path):
    """NBODY scored only PITSNAP's 6386 units, omitting its entire optimized simulation."""
    from collections import Counter
    from types import SimpleNamespace

    from qbopt import wholeseg
    from qbopt.model import ir
    from qbopt.frontend import blocks

    source = Path("fixtures/bench/nbody-v-g3.obj")
    emitted = wholeseg.emitted(source.read_bytes())
    assert emitted.outcome is wholeseg.Emission.LIR
    path = tmp_path / source.name
    path.write_bytes(emitted.data)
    module = opportunity._measured(path, True)
    partition = blocks.partition(module, blocks.code_map(module))
    expected = Counter()
    for decoded in ir.decode_module(module):
        body = SimpleNamespace(
            entry=decoded.body.seed,
            blocks=tuple(block for block in partition if any(lo <= block.at < hi for lo, hi in decoded.body.ranges)),
        )
        opportunity._cost(body, module, expected)
    assert expected["cost"] > 6386
    assert opportunity.counted([path], raw=True)["cost"] == expected["cost"]


def test_instruction_cost_does_not_require_mir_recognition(monkeypatch):
    """Unrecognized optimized code must retain its cost, not disappear with its MIR body."""
    path = Path("fixtures/omf/procs-p-g2.obj")
    expected = opportunity.counted([path], raw=True)["cost"]
    monkeypatch.setattr(opportunity.mir, "bodies", lambda *args, **kwargs: [])
    measured = opportunity.counted([path], raw=True)
    assert measured["cost"] == expected
    assert measured["code blocks unavailable to MIR opportunity analysis"] > 0


@pytest.mark.parametrize("proven", [True, False])
def test_nbody_output_loop_is_outside_the_completed_simulation(tmp_path, proven):
    """NBODY's output was priced at 100 trips because CEND fell into appended phi edges."""
    from dataclasses import replace

    from qbopt import wholeseg
    from qbopt.abi import runtime
    from qbopt.analysis import loops
    from qbopt.frontend import blocks

    source = Path("fixtures/bench/nbody-v-g3.obj")
    path = tmp_path / source.name
    path.write_bytes(wholeseg.emitted(source.read_bytes()).data)
    module = opportunity._measured(path, True)
    decoded = opportunity.ir.decode_module(module)[0].body
    physical = blocks.partition(module, blocks.code_map(module))
    mine = [block for block in physical if any(lo <= block.at < hi for lo, hi in decoded.ranges)]
    contracts = runtime.for_module(module)
    # Recreate the former appended-edge hazard independently of today's allocator layout.
    mine = [
        replace(block, succ=(decoded.seed,))
        if any(
            contracts.get(insn.at) is not None and contracts[insn.at].control is runtime.Control.NEVER
            for insn in block.insns
        )
        else block
        for block in mine
    ]
    if not proven:
        contracts = {at: replace(contract, control=runtime.Control.UNKNOWN) for at, contract in contracts.items()}
    execution = opportunity._execution_blocks(mine, decoded.seed, contracts)
    output = next(block for block in execution if any(module.calls.get(insn.at) == "B$STI2" for insn in block.insns))
    assert loops.depth(execution, decoded.seed)[output.at] == (1 if proven else 2)


@pytest.mark.parametrize("duplicate", [False, True])
def test_cost_refuses_incomplete_or_overlapping_body_partitions(monkeypatch, duplicate):
    """NBODY's omitted main exposed that partial body coverage was accepted as a full score."""
    decode = opportunity.ir.decode_module

    def broken(module):
        bodies = decode(module)
        assert len(bodies) > 1
        return (*bodies, bodies[0]) if duplicate else bodies[1:]

    monkeypatch.setattr(opportunity.ir, "decode_module", broken)
    with pytest.raises(opportunity.Unmeasured, match="overlap|cover every"):
        opportunity.counted([Path("fixtures/omf/procs-p-g2.obj")], raw=True)


@pytest.mark.parametrize("filename", ["ARRIDXQ.OBJ", "hotlop-p-g2.obj", "renamed.obj"])
def test_object_identity_not_temporary_filename_selects_weight_and_target(filename, tmp_path, capsys):
    """ARRIDX falsely fell from 504 to 312 when its temporary name selected ten rather than twenty trips."""
    original = Path("fixtures/omf/arridx-p-g2.obj")
    renamed = tmp_path / filename
    renamed.write_bytes(original.read_bytes())
    before = opportunity.counted([original], raw=True)
    after = opportunity.counted([renamed], raw=True)
    assert after == before
    assert opportunity._program(renamed) == "ARRIDX"
    opportunity.against_targets([renamed], raw=True)
    report = capsys.readouterr().out
    assert "NO TARGET" not in report
    assert str(opportunity.TARGETS["ARRIDX"]) in report


def test_program_identity_accepts_dos_source_paths_and_retains_headerless_fallback():
    from qbopt.objectfile import omf

    source = b"C:\\BUILD\\ArrIdx.BAS"
    header = omf.Record(omf.THEADR, bytes([len(source)]) + source)
    assert opportunity._program(Path("renamed.obj"), [header]) == "ARRIDX"
    assert opportunity._program(Path("arridx-p-g2.obj"), []) == "ARRIDX"


def test_cost_does_not_count_synthetic_raising_operations():
    """NOTS acquired extra cost from MIR extracts even though its emitted bytes were unchanged."""
    from collections import Counter
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    found = module.of(omf.parse(Path("fixtures/omf/nots-p-g2.obj").read_bytes()))
    partition = blocks.partition(found, blocks.code_map(found))
    body = mir.bodies(found, partition)[0][1]
    extra = next(op for block in body.blocks for op in block.ops if op.name == "extract")
    duplicate = replace(body, blocks=tuple(replace(block, ops=(*block.ops, extra)) for block in body.blocks))
    before, after = Counter(), Counter()
    opportunity._cost(body, found, before)
    opportunity._cost(duplicate, found, after)
    assert before["cost"] == after["cost"]


@pytest.mark.parametrize("program,target,status", [("nots", 306, 0), ("negnot", 254, 0), ("arith", 592, 0)])
def test_constant_bitwise_programs_have_references_and_report_the_gap(program, target, status, capsys):
    """NOTS/NEGNOT lacked targets, hiding their remaining constant-result propagation gap."""
    assert opportunity.TARGETS[program.upper()] == target
    assert opportunity.against_targets([Path(f"fixtures/omf/{program}-p-g2.obj")]) == status
    report = capsys.readouterr().out
    assert "NO TARGET" not in report and "PROVISIONAL" not in report


def test_qb_nots_meets_the_corrected_reference(capsys):
    """Constant argument propagation closes QB NOTS's gap without increasing its target."""
    assert opportunity.against_targets([Path("fixtures/omf/nots-q-O.obj".lower())]) == 0
    assert "nots-q-O" in capsys.readouterr().out


def test_fpdeep_has_a_source_derived_reference(capsys: pytest.CaptureFixture[str]) -> None:
    # The old 1086 reference omitted 21 numeric stores and 21 checkpoints.
    assert opportunity.TARGETS["FPDEEP"] == 9 * (4 * (6 + 20)) + 2 * 52 + 46 + 21 * 6 + 21 * 5 == 1317
    assert opportunity.against_targets([Path("fixtures/omf/fpdeep-p-g2.obj")]) == 0
    report = capsys.readouterr().out
    assert "PROVISIONAL" not in report and "1.19x" in report


@pytest.mark.parametrize("tag,accepted", [("p-g2", True), ("q-O", False), ("v-g3", False), ("p-evt", False)])
def test_fpdeep_reference_scope_comes_from_object_metadata(
    tag: str, accepted: bool, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    from collections import Counter

    path = tmp_path / "renamed.obj"
    path.write_bytes(Path(f"fixtures/omf/fpdeep-{tag}.obj".lower()).read_bytes())
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1317))
    assert opportunity.against_targets([path]) == int(not accepted)
    report = capsys.readouterr().out
    assert ("PROVISIONAL" not in report) == accepted


@pytest.mark.parametrize("name,body_cost", [("B$FIST", 86), ("B$FIS2", 80)])
def test_float_conversion_helpers_are_not_priced_as_empty_calls(name, body_cost):
    """Inlining FPDEEP's conversion appeared costlier because its callee body was free."""
    assert opportunity.CALLED[name] == opportunity.CALL + body_cost


@pytest.mark.parametrize("tag", ["p-evt", "q-evt", "v-evt"])
def test_event_build_is_outside_the_plain_release_code_gate(tag, tmp_path, capsys):
    """BOOLS /V/W is a correctness configuration, not a failed release-code target."""
    path = tmp_path / "bools-renamed.obj"
    path.write_bytes(Path(f"fixtures/omf/bools-{tag}.obj".lower()).read_bytes())
    assert opportunity.against_targets([path], raw=True) == 0
    report = capsys.readouterr().out
    assert "OUT OF SCOPE" in report and "event" in report
    assert "x " not in report


def test_event_configuration_cannot_claim_a_plain_target_ratio(monkeypatch, capsys):
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter({"cost": 1, "event-enabled configuration": 1}))
    assert opportunity.against_targets([Path("bools-q-O.obj")]) == 0
    report = capsys.readouterr().out
    assert "OUT OF SCOPE" in report and "x " not in report


def test_a_fallback_is_not_scored_as_success(monkeypatch) -> None:
    """A backend refusal could score BC or fallback bytes as optimized output."""
    monkeypatch.setattr(opportunity.rewrite, "rewrite", lambda data, **kwargs: (data, []))
    with pytest.raises(opportunity.Unmeasured, match="LIR"):
        opportunity._measured(Path("fixtures/omf/hotlop-p-g2.obj"), raw=False)


def test_unmapped_output_is_not_a_zero_cost_success(monkeypatch, capsys) -> None:
    """bools-q-O was reported as 0.0x when the output could not be mapped."""
    monkeypatch.setattr(opportunity, "code_map", lambda _: "unmapped code")
    status = opportunity.against_targets([Path("fixtures/omf/bools-q-O.obj".lower())])
    report = capsys.readouterr().out
    assert status != 0
    assert "UNMEASURED" in report and "0.0x" not in report


def test_above_target_is_not_success(monkeypatch) -> None:
    """A target report returned success even when press exceeded 1.5x."""
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=463))
    assert opportunity.against_targets([Path("press-p-g2.obj")]) != 0


def test_rewrite_failure_cannot_be_scored_as_optimized_output(monkeypatch) -> None:
    """A failed rewrite silently scored BC's original hotlop as our output."""

    def broken(*args, **kwargs):
        raise RuntimeError("rewrite failed")

    monkeypatch.setattr(opportunity.rewrite, "rewrite", broken)
    path = Path("fixtures/omf/hotlop-p-g2.obj")
    with pytest.raises(RuntimeError, match="rewrite failed"):
        opportunity._measured(path, raw=False)
    assert opportunity._measured(path, raw=True) is not None


def test_fresh_emitter_refusal_is_reported_and_does_not_abort_the_board(monkeypatch, capsys) -> None:
    """ARITH's flagged long add made fresh lowering refuse, but `--targets`
    aborted before reporting every later fixture.  A refusal is unmeasured,
    never fallback BC output and never a reason to hide the rest of a run.
    """
    monkeypatch.setattr(
        opportunity.rewrite,
        "rewrite",
        lambda *args, **kwargs: (_ for _ in ()).throw(opportunity.rewrite.Unsupported("live condition")),
    )
    path = Path("fixtures/omf/hotlop-p-g2.obj")
    with pytest.raises(opportunity.Unmeasured, match="fresh emission refused: live condition"):
        opportunity._measured(path, raw=False)
    assert opportunity.against_targets([path]) == 1
    assert "UNMEASURED" in capsys.readouterr().out


def test_cost_does_not_make_unknown_memory_accesses_free() -> None:
    """harr's cost changed when allocation obscured an array's address.

    The instruction still accesses memory if its exact address is unknown.
    Changing only address knowledge must not change its execution cost.
    """
    from collections import Counter
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split

    found = module.of(omf.parse(Path("fixtures/omf/harr-p-g2.obj").read_bytes()))
    blocks = split.partition(found, split.code_map(found))
    before, after = Counter(), Counter()
    for _name, body in mir.bodies(found, blocks):
        obscured = replace(
            body,
            blocks=tuple(
                replace(
                    block,
                    ops=tuple(
                        replace(
                            op,
                            loads=tuple(replace(ref, addr=None) for ref in op.loads),
                            stores=tuple(replace(ref, addr=None) for ref in op.stores),
                        )
                        for op in block.ops
                    ),
                )
                for block in body.blocks
            ),
        )
        opportunity._cost(body, found, before)
        opportunity._cost(obscured, found, after)
    assert before["cost"] > 0
    assert after["cost"] == before["cost"], "unknown addresses made real memory accesses free"


# Programs the passes provably change. Any of them would do; more than one
# so that a single fixture going quiet cannot make this vacuous.
MOVED = ("hotlop-p-g2", "press-p-g2", "spill-p-g2")


@pytest.mark.parametrize("name", MOVED)
def test_the_board_costs_our_output_and_not_bc(name: str) -> None:
    """--raw is BC. The default is what we ship, and they must differ."""
    path = Path(f"fixtures/omf/{name}.obj")
    ours = opportunity.counted([path])["cost"]
    theirs = opportunity.counted([path], raw=True)["cost"]
    assert ours and theirs, f"{name}: no cost at all, so this proves nothing"
    assert ours < theirs, (
        f"{name}: costed {ours} against BC's {theirs} -- the board is not "
        "reading our output, or the passes stopped paying"
    )


# Targets for programs no fixture exists for. They score nothing and cannot,
# so they are written down here rather than found again: the point of the
# test below is that this list does not grow without someone saying so.
UNBUILT = {"HG", "FX"}


def test_a_target_is_a_number_the_board_can_reach() -> None:
    """Every target names a program that exists, and none is zero.

    A target for a program with no fixture never prints and never fails --
    it simply is not on the board -- so nothing else would ever say so.
    """
    named = {opportunity._program(path) for path in Path("fixtures/omf").glob("*.obj")}
    for program, want in opportunity.TARGETS.items():
        assert want > 0, f"{program}: a target of {want} makes every ratio infinite"
    dead = {program for program in opportunity.TARGETS if program not in named}
    assert dead == UNBUILT, f"targets naming no fixture: {sorted(dead)}, expected {sorted(UNBUILT)}"


def test_missing_target_cannot_verify_completion(monkeypatch, capsys):
    """Nbody had no hand-derived target, yet its target report returned success."""
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1))
    assert opportunity.against_targets([Path("nbody-p-g2.obj")]) != 0
    assert "NO TARGET" in capsys.readouterr().out


@pytest.mark.parametrize("program", ["fpcsex", "fpcse", "fpdeep"])
def test_provisional_target_cannot_verify_completion(program, monkeypatch, capsys):
    """Unverified floating references or unknown compiler identities cannot certify completion."""
    from collections import Counter

    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter(cost=1))
    assert opportunity.against_targets([Path(f"{program}-p-g2.obj")]) != 0
    assert "PROVISIONAL" in capsys.readouterr().out


def test_fpcse_target_preserves_each_single_rounding_without_reassociation():
    """FPCSE's provisional 1340 hid its exact 487.5 constant-output reference."""
    from fractions import Fraction

    total = Fraction(0)
    for _ in range(10):
        product = (Fraction(2) + 4) * 8
        quotient = (Fraction(2) + 4) / 8
        subtotal = total + product
        total = subtotal + quotient
        for value in (product, quotient, subtotal, total):
            assert value.denominator & (value.denominator - 1) == 0
            assert abs(value.numerator).bit_length() <= 24
    assert total == Fraction(975, 2)
    # PDS/VBDOS preserve five stores before the pending-exception checkpoint,
    # then four final stores. The output-only 98-unit subtotal is not a target.
    assert opportunity.TARGETS["FPCSE"] == 9 * (2 + opportunity.TOUCH) + 5 + 3 * (6 + 20) + 20 == 157
    assert "FPCSE" not in opportunity.PROVISIONAL_TARGETS


def test_hotlpx_target_accounts_for_the_complete_runtime_input_reference():
    """HOTLPX inherited 312 from HOTLOP instead of pricing its own input and closed form."""
    # Hand listing in docs/measurement/targets.md, not inferred from optimized output.
    input_cost = 2 * (6 + 6 + 20)
    arithmetic = 6 + 26 + 2 + 3 + 2 + 6 + 6
    output_cost = 6 + 20 + 10 + 20 + 6 + 20 + 20
    assert opportunity.TARGETS["HOTLPX"] == input_cost + arithmetic + output_cost == 217
    assert "HOTLPX" not in opportunity.PROVISIONAL_TARGETS


@pytest.mark.parametrize("program,target", [("PRESSX", 508), ("LNGMXX", 208)])
def test_integer_runtime_references_include_input_arithmetic_and_output(program, target):
    """PRESSX and LNGMXX inherited 308/210 from different, constant-input computations."""
    if program == "PRESSX":
        hand = 8 * (6 + 6 + 20) + 4 * (6 + 26) + 3 * 2 + 2 + 2 + 6 + 6 + 102
    else:
        hand = 32 + (6 + 2 + 22 + 2 + 2 + 3 + 3 + 2 + 2 + 2 + 2 + 2 + 2 + 6 + 6) + 112
    assert opportunity.TARGETS[program] == hand == target
    assert program not in opportunity.PROVISIONAL_TARGETS


def test_lngmxx_magic_reference_keeps_signed_quotient_and_wrapped_sum():
    """LNGMXX's reference must truncate negative division toward zero, not floor it."""
    values = set(range(-1000, 1001))
    values.update(sign * (2**bit + delta) for bit in range(32) for sign in (-1, 1) for delta in range(-7, 8))
    for value in values:
        if not -(1 << 31) <= value < (1 << 31):
            continue
        corrected = (value * -1840700269 >> 32) + value
        quotient = (corrected >> 2) + ((corrected & 0xFFFFFFFF) >> 31)
        expected = abs(value) // 7 * (-1 if value < 0 else 1)
        assert quotient == expected
        remainder = value - expected * 7
        assert (10 * (value - 6 * quotient)) & 0xFFFFFFFF == (10 * (expected + remainder)) & 0xFFFFFFFF


@pytest.mark.parametrize("values", [(3, 5, 7, 11, 13, 17, 19, 23), (-32768,) * 8, (32767,) * 8, (-123, 71) * 4])
def test_pressx_reference_retains_modular_sum(values):
    """The PRESSX reference combines ten iterations without assuming signed arithmetic cannot wrap."""
    products = [values[index] * values[index + 1] for index in range(0, 8, 2)]
    total = 0
    for _ in range(10):
        for product in products:
            total = (total + product) & 65535
    assert total == (sum(products) * 10) & 65535


@pytest.mark.parametrize("left,right", [(7, 3), (-32768, -1), (32767, 32767), (-123, 71), (0, 32767)])
def test_hotlpx_reference_preserves_word_wrap_and_loop_exit(left, right):
    """Closed-form HOTLPX must retain wraparound rather than assume mathematical signed overflow."""
    total = 0
    for counter in range(1, 21):
        total = (total + ((left * right) & 65535) + counter) & 65535
    product = (left * right) & 65535
    reference = ((((product + product * 4) & 65535) << 2) + 210) & 65535
    assert reference == total
    assert counter + 1 == 21
