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


def test_cost_does_not_count_synthetic_raising_operations():
    """NOTS acquired extra cost from MIR extracts even though its emitted bytes were unchanged."""
    from collections import Counter
    from dataclasses import replace
    from qbopt.model import mir
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks
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
    assert opportunity.against_targets([Path("fixtures/omf/nots-q-O.obj")]) == 0
    assert "1.43x" in capsys.readouterr().out


def test_fpdeep_has_a_source_derived_reference(capsys):
    """FPDEEP's missing denominator hid its constant floating-expression gap."""
    assert opportunity.TARGETS["FPDEEP"] == 9 * (4 * (6 + 20)) + 2 * 52 + 46
    assert opportunity.against_targets([Path("fixtures/omf/fpdeep-p-g2.obj")]) == 1
    report = capsys.readouterr().out
    assert "NO TARGET" not in report and "PROVISIONAL" not in report


@pytest.mark.parametrize("name,body_cost", [("B$FIST", 86), ("B$FIS2", 80)])
def test_float_conversion_helpers_are_not_priced_as_empty_calls(name, body_cost):
    """Inlining FPDEEP's conversion appeared costlier because its callee body was free."""
    assert opportunity.CALLED[name] == opportunity.CALL + body_cost


@pytest.mark.parametrize("tag", ["p-evt", "q-evt", "v-evt"])
def test_event_build_does_not_use_a_plain_program_target(tag, tmp_path, capsys):
    """BOOLS /V/W was scored against a reference with no event checks (QB read as 4.57x)."""
    path = tmp_path / "bools-renamed.obj"
    path.write_bytes(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes())
    assert opportunity.against_targets([path], raw=True) != 0
    report = capsys.readouterr().out
    assert "PROVISIONAL" in report and "event" in report
    assert "x " not in report


def test_event_configuration_cannot_pass_even_below_plain_target(monkeypatch, capsys):
    from collections import Counter
    monkeypatch.setattr(opportunity, "counted", lambda *args: Counter({"cost": 1, "event-enabled configuration": 1}))
    assert opportunity.against_targets([Path("bools-q-O.obj")]) != 0
    assert "PROVISIONAL" in capsys.readouterr().out


def test_a_fallback_is_not_scored_as_success(monkeypatch) -> None:
    """A backend refusal could score BC or fallback bytes as optimized output."""
    monkeypatch.setattr(opportunity.rewrite, "rewrite", lambda data, **kwargs: (data, []))
    with pytest.raises(opportunity.Unmeasured, match="LIR"):
        opportunity._measured(Path("fixtures/omf/hotlop-p-g2.obj"), raw=False)


def test_unmapped_output_is_not_a_zero_cost_success(monkeypatch, capsys) -> None:
    """bools-q-O was reported as 0.0x when the output could not be mapped."""
    monkeypatch.setattr(opportunity, "code_map", lambda _: "unmapped code")
    status = opportunity.against_targets([Path("fixtures/omf/bools-q-O.obj")])
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


@pytest.mark.parametrize("program", ["fpcsex"])
def test_provisional_target_cannot_verify_completion(program, monkeypatch, capsys):
    """Runtime-input twins inherited constant-source targets; FP references changed rounding and sums."""
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
    assert opportunity.TARGETS["FPCSE"] == 3 * (6 + 20) + 20 == 98
    assert "FPCSE" not in opportunity.PROVISIONAL_TARGETS


def test_hotlpx_target_accounts_for_the_complete_runtime_input_reference():
    """HOTLPX inherited 312 from HOTLOP instead of pricing its own input and closed form."""
    # Hand listing in docs/targets.md, not inferred from optimized output.
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


@pytest.mark.parametrize("values", [(3,5,7,11,13,17,19,23), (-32768,)*8, (32767,)*8, (-123,71)*4])
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
