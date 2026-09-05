"""The six steps, and what each is supposed to guarantee.

    parse -> raise -> passes -> lower -> allocate -> write

Not the shipped path yet: rewrite.py goes through wholeseg, and what this
produces is measured against that rather than trusted over it. What is
asserted here is that the seams hold -- every step takes one form and
returns the next -- and that the allocator prices what it is meant to.
"""

from pathlib import Path

import pytest

from qbopt import allocate
from qbopt import blocks as split
from qbopt import flow
from qbopt import intervals
from qbopt import ir
from qbopt import lir
from qbopt import lower
from qbopt import mir
from qbopt import module
from qbopt import omf
from qbopt import phielim
from qbopt.blocks import code_map
from qbopt.passes import LIRTransform

# One configuration per program rather than all twelve. The full sweep is
# `tools/flow.py`, which is where the byte total comes from; running 487
# objects here put three minutes on a gate that is meant to take fifty
# seconds, and the twelve configurations of one program fail together.
CORPUS = sorted(Path("fixtures/omf").glob("*-p-g2.obj"))


def _raised(name: str):
    found = module.of(omf.parse(Path(f"fixtures/omf/{name}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    return found, blocks, list(mir.bodies(found, blocks))


@pytest.mark.corpus
def test_the_whole_flow_writes_what_it_can_and_names_what_it_cannot() -> None:
    """Every refusal is one of the phases LLVM has and this does not.

    Today that is the spiller: the allocator prices spilling and chooses
    it, and nothing turns the choice into a store and a load. A refusal
    that is not `Spilled` is a bug rather than a missing phase.
    """
    missing, wrong = [], []
    for path in CORPUS:
        try:
            _out, why = flow.run(path.read_bytes())
        except allocate.Spilled as short:
            missing.append(f"{path.stem}: {short}")
            continue
        except Exception as error:  # noqa: BLE001 -- nothing else may escape
            wrong.append(f"{path.stem}: {type(error).__name__}: {error}")
            continue
        if why != "written":
            wrong.append(f"{path.stem}: {why}")
    # An emit refusal is a defect rather than a missing phase, and there is
    # one: a call whose fixup has no field to sit in after the spiller
    # renamed what it read, so select picked a form without a
    # displacement. Bounded rather than allowed -- the number may not grow.
    assert not wrong, f"{len(wrong)} of {len(CORPUS)}: " + "; ".join(wrong[:4])
    assert not missing, "the spiller exists; nothing should be refused for wanting one: " + "; ".join(missing[:2])


def test_every_machine_phase_takes_lir_and_gives_lir_back() -> None:
    """The contract, and the reason there are two pass bases.

    A phase that took MIR would be reaching back up a form, which is what
    the allocator did until `lir.Insn` learned to carry its own defs and
    uses.
    """
    _found, _blocks, bodies = _raised("nested-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body)
        for phase in flow.machine(flow._pinned(body)):
            assert isinstance(phase, LIRTransform), f"{phase} is not a LIR phase"
            try:
                low = phase.transform(low)
            except allocate.Spilled:
                return
            assert isinstance(low, lir.LirBody), f"{phase} gave back {type(low).__name__}"


def test_phi_elimination_takes_the_body_out_of_ssa() -> None:
    """Nothing can be assigned a register while a phi still stands: two
    values meet there and the interference is invisible, which is the bug
    docs/hoist-blocker.md spent a week on."""
    _found, _blocks, bodies = _raised("nested-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body)
        assert sum(len(block.phis) for block in low.blocks), "nested has phis; the lowering lost them"
        out = phielim.eliminated(low)
        assert not sum(len(block.phis) for block in out.blocks), "a phi survived elimination"
        assert len(out.insns) > len(low.insns), "a phi became no copy at all"


def test_lowering_leaves_no_mir_operand_behind() -> None:
    """Below LIR every operand is a location. A MemRef reaching select is
    the seam leaking: `mov [seg:5+0xe],ax` refused to encode for exactly
    that, and the message said only that the mov was not one it could emit.
    """
    _found, _blocks, bodies = _raised("flags-p-g2-zd")
    for name, body in bodies:
        for one in lower.lowered(name, body).insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                assert not isinstance(where, (mir.MemRef, mir.Held, mir.Const, mir.Cell)), (
                    f"{one.at:#06x} still holds {where!r}"
                )


def test_a_value_in_a_loop_costs_more_than_one_outside() -> None:
    """Spilling is priced by where the references are, not counted.

    LLVM's formula: references weighted by loop depth, divided by how long
    the value is live. The loop is where every program in this suite spends
    its time, so a value referenced inside one outweighs a value referenced
    as often outside it.
    """
    _found, _blocks, bodies = _raised("lngmix-p-g2")
    (name, body), = bodies
    low = lower.lowered(name, body)
    deep = intervals.depths(low)
    assert set(deep.values()) >= {0, 1}, "lngmix has a loop; the depths say otherwise"

    price = intervals.weights(low)
    inside = {v for block in low.blocks if deep[block.at] for one in block.insns for v in one.defines}
    outside = {v for block in low.blocks if not deep[block.at] for one in block.insns for v in one.defines}
    inside -= outside
    assert inside and outside
    assert min(price[one] for one in inside) > max(price[one] for one in outside), (
        "a value defined only inside a loop is priced no higher than one outside it"
    )


@pytest.mark.parametrize("name", ["lngmix-p-g2", "hotlop-p-g2", "nested-p-g2", "nots-p-g2"])
def test_the_allocation_is_searched_and_says_whether_it_is_optimal(name: str) -> None:
    """Branch and bound, with a node budget. A result that ran out of
    budget says so rather than claiming an optimum it did not prove."""
    _found, _blocks, bodies = _raised(name)
    for who, body in bodies:
        got = allocate.allocate(lower.lowered(who, body), flow._pinned(body))
        assert got.optimal or got.why, "an unproven assignment has to say why"
        if got.optimal:
            assert got.why == ""
        assert got.cost >= 0.0


def test_lowering_gives_back_lir_and_allocation_gives_back_lir() -> None:
    """Each step's output is the next step's input, and nothing else."""
    _found, _blocks, bodies = _raised("hotlop-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body)
        assert isinstance(low, lir.LirBody)
        after = allocate.applied(low, allocate.allocate(low, flow._pinned(body)))
        assert isinstance(after, lir.LirBody)
        assert [one.at for one in after.insns] == [one.at for one in low.insns]


def test_the_allocator_reads_lir_and_nothing_above_it() -> None:
    """It used to take a MirBody, for one reason: liveness and interference
    read `op.defines` and `op.uses`, and a lowered instruction carried
    neither -- only `ir.Held(value_id)` inside an encoded operand. So the
    last pass in the machine half reached back up a form to ask a question
    about its own input. `lir.Insn` carries the two lists now."""
    import inspect

    source = inspect.getsource(allocate)
    for name in ("mir.", "liveness.", "transform.", "select."):
        assert name not in source.replace("liveness.py answers", ""), f"allocate.py still asks {name}"
    assert allocate.allocate.__annotations__["body"] is lir.LirBody


def test_lir_says_what_each_instruction_defines_and_uses() -> None:
    """Without them the allocator cannot build its own graph, which is the
    whole reason it used to be handed MIR. Flags are excluded: they are one
    register nothing is placed in."""
    _found, _blocks, bodies = _raised("lngmix-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body)
        assert any(one.defines for one in low.insns), "no instruction defines anything"
        assert any(one.uses for one in low.insns), "no instruction uses anything"
        assert any(block.arrives for block in low.blocks), "no phi result arrives anywhere"
        graph = allocate.interference(low)
        assert graph, "no interference at all, from a body with a loop in it"


def test_the_register_file_is_written_down_once() -> None:
    """It was in six modules and one table existed twice, at two widths.

    Reading the two as duplicates and keeping the narrower one broke every
    object in the corpus: an `ir.Held` of width 1 has no entry in a table
    built from `ir.ROOT`, so it resolved to its root and `mov [k],al`
    became `mov [k],eax`.
    """
    from qbopt import regalloc
    from qbopt import select
    from qbopt import target

    assert select.AT_WIDTH is target.AT_WIDTH
    assert select.WIDTHS is target.WIDTHS
    assert not hasattr(regalloc, "AVAILABLE"), "regalloc has its own register file again"
    assert 1 in target.AT_WIDTH[next(iter(target.WIDE))], "the byte halves are missing from the table"


def test_a_value_that_addresses_memory_is_confined_to_a_base_register() -> None:
    """`[dx+0Ah]` has no encoding. An allocator that does not know the class
    hands out dx eventually, and the instruction cannot be emitted."""
    from qbopt import target

    for path in CORPUS[:12]:
        found = module.of(omf.parse(path.read_bytes()))
        blocks = split.partition(found, code_map(found))
        for name, body in mir.bodies(found, blocks):
            low = lower.lowered(name, body)
            for value, where in allocate.classes(low).items():
                assert where is target.ADDRESSING, f"value#{value} confined to something unexpected"
                assert set(target.order(where)) <= set(target.AVAILABLE)


def test_the_verifier_objects_to_a_body_that_claims_a_byte_twice() -> None:
    """The check that would have caught it at the phase, not at emit.

    An inserted instruction carried the operation it stood beside, and the
    span came off that -- so a phi's copy and its neighbour both claimed
    the same bytes. Layout reported it twelve objects later as "1 bytes are
    claimed by more than one op", which names neither the phase nor the
    instruction.
    """
    from dataclasses import replace

    from qbopt import verify

    _found, _blocks, bodies = _raised("hotlop-p-g2")
    (name, body), = bodies
    low = lower.lowered(name, body)
    assert not verify.verify(low, in_ssa=True), "a freshly lowered body is not well formed"

    first = low.blocks[0]
    twice = replace(first, insns=(first.insns[0], replace(first.insns[1], covers=first.insns[0].covers)))
    assert any("claimed by" in one for one in verify.verify(replace(low, blocks=(twice, *low.blocks[1:]))))


def test_a_spilled_value_gets_a_slot_and_the_prologue_reserves_it() -> None:
    """Choosing to spill is half of it. Until the spiller existed the
    choice was made and 237 of 487 objects were refused because an operand
    still named a value with no register."""
    from qbopt import frame as frames
    from qbopt import prologue
    from qbopt import spiller
    from qbopt import verify

    _found, _blocks, bodies = _raised("nested-p-g2")
    (name, body), = bodies
    low = lower.lowered(name, body)
    # Everything up to the allocator, which now owns the spill loop -- so
    # asking it after that phase would see the spilling already done.
    for phase in flow.machine(flow._pinned(body)):
        if phase.name == "regalloc":
            break
        low = phase.transform(low)

    got = allocate.allocate(low, {})
    assert got.spilled, "nested spills; the allocator says otherwise"
    frame = frames.of(low)
    after = spiller.spilled(low, got.spilled, frame)
    assert frame.size >= 2 * len(got.spilled), "the frame did not grow by a slot per spilled value"
    assert not allocate.allocate(after, {}).spilled, "spilling freed no register"

    with_frame = prologue.reserved(after, frame, _found.calls)
    assert not verify.verify(with_frame), verify.verify(with_frame)[:2]


def test_a_register_names_which_bytes_of_its_root_it_is() -> None:
    """al and ah share eax and share no byte. `ir.ROOT` folds both to eax
    and cannot tell them apart, which is how two operations on opposite
    halves of one long compared equal and nots printed the right low word
    of NOTOR and the wrong high one."""
    from iced_x86 import Register

    from qbopt import target

    assert not target.overlaps(Register.AL, Register.AH)
    assert target.overlaps(Register.AL, Register.AX)
    assert target.overlaps(Register.AH, Register.EAX)
    assert not target.overlaps(Register.AL, Register.BL)


def test_an_inserted_instruction_carries_no_fixup() -> None:
    """It stands beside another and carries that one's address.

    A far call's four relocated bytes are found by reading `found.code` at
    `op.at`, so an inserted instruction sitting at a far call's address
    read the `9a` and claimed the call's own fixup. Nineteen objects said
    `call has 1 fixups and 0 fields to put them in`, naming the call --
    which was not the operation asking.

    The fix is that an inserted instruction has no node: `node` is the
    instruction an operation was raised from, and every question answered
    by reading the original bytes goes through it.
    """
    from qbopt import objwrite

    _found, _blocks, bodies = _raised("divmod-p-g2-zd")
    for name, body in bodies:
        low = lower.lowered(name, body)
        for phase in flow.machine(flow._pinned(body), None, _found.calls):
            low = phase.transform(low)
        as_mir = objwrite._as_mir(low)
        for block in as_mir.blocks:
            for op in block.ops:
                if op.covers is not None and op.covers[0] == op.covers[1]:
                    assert op.node is None, f"{op.at:#06x} was inserted and still has a node"
                    assert op.id is None, f"{op.at:#06x} was inserted and still has an id"
