"""The six steps, and what each is supposed to guarantee.

    parse -> raise -> passes -> lower -> allocate -> write

Not the shipped path yet: rewrite.py goes through wholeseg, and what this
produces is measured against that rather than trusted over it. What is
asserted here is that the seams hold -- every step takes one form and
returns the next -- and that the allocator prices what it is meant to.
"""

from pathlib import Path

import pytest

from qbopt import lir
from qbopt import mir
from qbopt import omf
from qbopt import flow
from qbopt import lower
from qbopt import module
from qbopt import phielim
from qbopt import runtime
from qbopt import allocate
from qbopt import intervals
from qbopt import blocks as split
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
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
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
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
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
        for one in lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found)).insns:
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
    ((name, body),) = bodies
    low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
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
        got = allocate.allocate(
            lower.lowered(who, body, _found.calls, set(_found.absorbed), runtime.for_module(_found)), flow._pinned(body)
        )
        assert got.optimal or got.why, "an unproven assignment has to say why"
        if got.optimal:
            assert got.why == ""
        assert got.cost >= 0.0


def test_lowering_gives_back_lir_and_allocation_gives_back_lir() -> None:
    """Each step's output is the next step's input, and nothing else."""
    _found, _blocks, bodies = _raised("hotlop-p-g2")
    for name, body in bodies:
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
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
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
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
    from qbopt import select
    from qbopt import target
    from qbopt import regalloc

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
            low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
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
    ((name, body),) = bodies
    low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
    assert not verify.verify(low, in_ssa=True), "a freshly lowered body is not well formed"

    first = low.blocks[0]
    twice = replace(first, insns=(first.insns[0], replace(first.insns[1], covers=first.insns[0].covers)))
    assert any("claimed by" in one for one in verify.verify(replace(low, blocks=(twice, *low.blocks[1:]))))


def test_a_spilled_value_gets_a_slot_and_the_prologue_reserves_it() -> None:
    """Choosing to spill is half of it. Until the spiller existed the
    choice was made and 237 of 487 objects were refused because an operand
    still named a value with no register."""
    from qbopt import verify
    from qbopt import spiller
    from qbopt import prologue
    from qbopt import frame as frames

    # divmod, because nested stopped spilling: the coalescer now asks
    # Briggs before a join and leaves the allocator a body it can place.
    # The claim here is the spiller's, so it needs a body that spills.
    _found, _blocks, bodies = _raised("divmod-p-g2")
    ((name, body),) = bodies
    low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
    # Everything up to the allocator, which now owns the spill loop -- so
    # asking it after that phase would see the spilling already done.
    for phase in flow.machine(flow._pinned(body)):
        if phase.name == "regalloc":
            break
        low = phase.transform(low)

    got = allocate.allocate(low, {})
    assert got.spilled, "divmod spills; the allocator says otherwise"
    frame = frames.of(low)
    after, reloads = spiller.spilled(low, got.spilled, frame)
    assert frame.size >= 2 * len(got.spilled), "the frame did not grow by a slot per spilled value"
    assert reloads, "spilling made no reload values"
    assert not allocate.allocate(after, {}, reloads).spilled, "spilling freed no register"

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
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for phase in flow.machine(flow._pinned(body), None, _found.calls):
            low = phase.transform(low)
        as_mir = objwrite._as_mir(low)
        for block in as_mir.blocks:
            for op in block.ops:
                if op.covers is not None and op.covers[0] == op.covers[1]:
                    assert op.node is None, f"{op.at:#06x} was inserted and still has a node"
                    assert op.id is None, f"{op.at:#06x} was inserted and still has an id"


def test_a_reload_cannot_be_spilled_again() -> None:
    """Otherwise nothing settles.

    A reload's value is live across one instruction, so its weight is
    tiny -- references over live range, and the range is one slot. Under a
    cost model it therefore never wins a register and is spilled again,
    which puts a load in front of a load: three values spilled every round
    and three instructions added every round, for ever. LLVM says it as
    `LiveInterval::markNotSpillable`; here the reloads are handed back to
    `allocate` and weigh infinity.
    """
    from qbopt import spiller
    from qbopt import frame as frames

    # divmod, because fpcsex stopped spilling once the coalescer began
    # refusing a join that would leave a class uncolourable.
    _found, _blocks, bodies = _raised("divmod-p-g2")
    ran = False
    for name, body in bodies:
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for phase in flow.machine(flow._pinned(body)):
            if phase.name == "regalloc":
                break
            low = phase.transform(low)
        got = allocate.allocate(low, {})
        if not got.spilled:
            continue
        ran = True
        after, reloads = spiller.spilled(low, got.spilled, frames.of(low))
        assert reloads, "spilling made no reload values"
        again = allocate.allocate(after, {}, reloads)
        assert not (again.spilled & reloads), f"a reload was spilled: {sorted(again.spilled & reloads)}"
    assert ran, "divmod spills; the allocator says otherwise"


def test_the_allocator_settles_on_every_program() -> None:
    """The phase's own loop: assign, evict, split, spill, and again.

    One object does not settle -- bools-q-evt spills the same twelve values
    every round and adds thirty-six instructions each time, because their
    ranges cross calls that clobber every register. Bounded rather than
    allowed: the number may not grow.
    """
    stuck = []
    for path in CORPUS:
        found = module.of(omf.parse(path.read_bytes()))
        blocks = split.partition(found, code_map(found))
        for name, body in mir.bodies(found, blocks):
            low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
            try:
                for phase in flow.machine(flow._pinned(body), None, found.calls):
                    low = phase.transform(low)
            except allocate.Spilled as short:
                stuck.append(f"{path.stem}: {short}")
    assert len(stuck) <= 1, f"{len(stuck)}: " + "; ".join(one[:70] for one in stuck[:3])


def test_the_register_file_is_written_down_once() -> None:
    """It was in six modules and one table existed twice, at two widths.

    Reading the two as duplicates and keeping the narrower one broke every
    object in the corpus: an `ir.Held` of width 1 has no entry in a table
    built from `ir.ROOT`, so it resolved to its root and `mov [k],al`
    became `mov [k],eax`.
    """
    from qbopt import select
    from qbopt import target
    from qbopt import regalloc

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
            low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
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
    ((name, body),) = bodies
    low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
    assert not verify.verify(low, in_ssa=True), "a freshly lowered body is not well formed"

    first = low.blocks[0]
    twice = replace(first, insns=(first.insns[0], replace(first.insns[1], covers=first.insns[0].covers)))
    assert any("claimed by" in one for one in verify.verify(replace(low, blocks=(twice, *low.blocks[1:]))))


def test_a_spilled_value_gets_a_slot_and_the_prologue_reserves_it() -> None:
    """Choosing to spill is half of it. Until the spiller existed the
    choice was made and 237 of 487 objects were refused because an operand
    still named a value with no register."""
    from qbopt import verify
    from qbopt import spiller
    from qbopt import prologue
    from qbopt import frame as frames

    # divmod, because nested stopped spilling: the coalescer now asks
    # Briggs before a join and leaves the allocator a body it can place.
    # The claim here is the spiller's, so it needs a body that spills.
    _found, _blocks, bodies = _raised("divmod-p-g2")
    ((name, body),) = bodies
    low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
    # Everything up to the allocator, which now owns the spill loop -- so
    # asking it after that phase would see the spilling already done.
    for phase in flow.machine(flow._pinned(body)):
        if phase.name == "regalloc":
            break
        low = phase.transform(low)

    got = allocate.allocate(low, {})
    assert got.spilled, "divmod spills; the allocator says otherwise"
    frame = frames.of(low)
    after, reloads = spiller.spilled(low, got.spilled, frame)
    assert frame.size >= 2 * len(got.spilled), "the frame did not grow by a slot per spilled value"
    assert reloads, "spilling made no reload values"
    assert not allocate.allocate(after, {}, reloads).spilled, "spilling freed no register"

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
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for phase in flow.machine(flow._pinned(body), None, _found.calls):
            low = phase.transform(low)
        as_mir = objwrite._as_mir(low)
        for block in as_mir.blocks:
            for op in block.ops:
                if op.covers is not None and op.covers[0] == op.covers[1]:
                    assert op.node is None, f"{op.at:#06x} was inserted and still has a node"
                    assert op.id is None, f"{op.at:#06x} was inserted and still has an id"


def test_a_reload_cannot_be_spilled_again() -> None:
    """Otherwise nothing settles.

    A reload's value is live across one instruction, so its weight is
    tiny -- references over live range, and the range is one slot. Under a
    cost model it therefore never wins a register and is spilled again,
    which puts a load in front of a load: three values spilled every round
    and three instructions added every round, for ever. LLVM says it as
    `LiveInterval::markNotSpillable`; here the reloads are handed back to
    `allocate` and weigh infinity.
    """
    from qbopt import spiller
    from qbopt import frame as frames

    # divmod, because fpcsex stopped spilling once the coalescer began
    # refusing a join that would leave a class uncolourable.
    _found, _blocks, bodies = _raised("divmod-p-g2")
    ran = False
    for name, body in bodies:
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for phase in flow.machine(flow._pinned(body)):
            if phase.name == "regalloc":
                break
            low = phase.transform(low)
        got = allocate.allocate(low, {})
        if not got.spilled:
            continue
        ran = True
        after, reloads = spiller.spilled(low, got.spilled, frames.of(low))
        assert reloads, "spilling made no reload values"
        again = allocate.allocate(after, {}, reloads)
        assert not (again.spilled & reloads), f"a reload was spilled: {sorted(again.spilled & reloads)}"
    assert ran, "divmod spills; the allocator says otherwise"


def test_the_allocator_settles_on_every_program() -> None:
    """The phase's own loop: assign, evict, split, spill, and again.

    Spilling frees registers for the values that failed and takes them from
    nobody, so each round can only reduce the pressure -- but only if the
    reloads keep theirs, which is the test above.
    """
    for path in CORPUS:
        found = module.of(omf.parse(path.read_bytes()))
        blocks = split.partition(found, code_map(found))
        # One map, built once and handed to both: built twice they can
        # differ, and the raise then establishes a contract the lowering
        # does not -- which left a pin naming a value the body no longer
        # held, reported as `value#19 at width 4 has no register`.
        contracts = runtime.for_module(found)
        for name, body in mir.bodies(found, blocks, contracts):
            low = lower.lowered(name, body, found.calls, set(found.absorbed), contracts)
            for phase in flow.machine(flow._pinned(body), None, found.calls):
                low = phase.transform(low)


def test_the_allocator_evicts_rather_than_spilling_a_costlier_range() -> None:
    """RegAllocGreedy's whole decision: a cheap range moves aside.

    The exhaustive search this replaced found the same answers by trying
    everything, which only proved the cost model right at a price that
    grows with the body.
    """
    from qbopt import target

    for path in CORPUS[:16]:
        found = module.of(omf.parse(path.read_bytes()))
        blocks = split.partition(found, code_map(found))
        for name, body in mir.bodies(found, blocks):
            low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
            got = allocate.allocate(low, {})
            # Nothing is in two places, and nothing is somewhere it may not be.
            confined = allocate.classes(low)
            for value, register in got.where.items():
                assert register in target.AVAILABLE, f"value#{value} is in {register}"
                if value in confined:
                    assert register in target.order(confined[value]), (
                        f"value#{value} addresses memory and is in {register}"
                    )


def test_a_call_carries_a_mask_rather_than_defining_a_value_per_register() -> None:
    """LLVM's register mask, and what it saves.

    A call used to define a value for every register it clobbered. 114 of
    nbody's 162 call defines were read by nothing, and each one still got
    an interval, competed for a register and was spilled -- a store for a
    value nobody wanted. The mask says the same thing and costs nothing.

    The mask has to be honoured or the drop is a miscompile: nothing live
    across the call may sit in a register the call destroys.
    """
    from qbopt import target
    from qbopt import intervals as ranges

    _found, _blocks, bodies = _raised("lngmix-p-g2")
    masked = 0
    for name, body in bodies:
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for block in low.blocks:
            for one in block.insns:
                if not one.clobbers:
                    continue
                masked += 1
                assert one.clobbers <= set(target.AVAILABLE), "a mask names a register nothing allocates"
                assert not (set(one.defines) & set(one.uses)), "a call both defines and uses one value"
    assert masked, "lngmix calls the runtime; no instruction carries a mask"

    # And nothing the allocator seats sits in a register a call it crosses
    # destroys.
    for name, body in bodies:
        low = lower.lowered(name, body, _found.calls, set(_found.absorbed), runtime.for_module(_found))
        for phase in flow.machine(flow._pinned(body), None, _found.calls):
            if phase.name == "regalloc":
                break
            low = phase.transform(low)
        index = ranges.indexed(low)
        live = ranges.intervals(low, index)
        masks = allocate._masks(low, index)
        for value, register in allocate.allocate(low, {}).where.items():
            if value in live:
                assert not allocate._clobbered(live[value], register, masks), (
                    f"value#{value} sits in {target.name_of(register)} across a call that destroys it"
                )


def test_strength_reduction_replaces_a_loop_multiply_with_an_add() -> None:
    """The transform works; it is off because it costs.

    matrix recomputes a row address from the counter every iteration with
    `imul word [w]`. Reduced, the multiply moves to the preheader and an
    add of the same width advances it -- one operation more in the body,
    one multiply fewer in the loop.

    No phi is written: a fresh variable assigned in the preheader and again
    at the latch *is* one, and `mir.resolved()` puts it at the header.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import strength
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/matrix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    bounds = module.landmarks(found)
    fired = False
    for _name, body in mir.bodies(found, blocks):
        out = strength.reduced(body, found.dgroup, bounds)
        if out is body:
            continue
        fired = True
        was = [op for block in body.blocks for op in block.ops if op.kind is mir.Kind.MUL]
        now = [op for block in out.blocks for op in block.ops if op.kind is mir.Kind.MUL]
        assert len(now) == len(was), "a multiply should move, not multiply"
        assert any(op.node is None for op in now), "the preheader multiply was not inserted"
        adds = [op for block in out.blocks for op in block.ops if op.kind is mir.Kind.ADD and op.node is None]
        assert adds, "no add advances the new counter"
        # And every inserted operation claims none of BC's own bytes.
        for block in out.blocks:
            for op in block.ops:
                if op.node is None and op.covers is not None:
                    assert op.covers[0] == op.covers[1], f"{op.at:#06x} claims bytes it did not stand for"
    assert fired, "matrix multiplies its counter by a width it never changes"


def test_strength_reduction_is_off_because_it_measured_worse() -> None:
    """Not a guess: through the flow path, where two-address is handled,

        harr    8.6x -> 9.8x      segld  6.4x -> 8.0x
        split   4.3x -> 6.2x      matrix 3.4x -> 3.5x

    The multiply it removes reads memory and the add it inserts reads the
    same memory, so the body is no cheaper -- and the new counter holds a
    register for the whole loop. LLVM's LoopStrengthReduce is mostly a cost
    model for this; ours prices nothing.
    """
    from qbopt import transform

    assert "strength" not in transform.PASSES_ON, "strength is on and it measured worse"


def test_promotion_takes_a_variable_out_of_memory() -> None:
    """LLVM's mem2reg. A cell nothing else can name becomes a value.

    It is off, and the reason is not the pass: `layout.allocated` hands
    back the *raised* body for one the allocator refuses, throwing away
    every pass's work. Promotion makes press's body unallocatable, so all
    of it is discarded and press costs 468 -> 1788 -- a four-fold
    regression from a pass that removed 207 memory operations.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import promote
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    was = now = cells = 0
    for name in ("arith-p-g2", "bools-p-g2", "flags-p-g2"):
        found = module.of(omf.parse(Path(f"fixtures/omf/{name}.obj").read_bytes()))
        blocks = split.partition(found, code_map(found))
        bounds = module.landmarks(found)
        for _who, body in mir.bodies(found, blocks):
            cells += len(promote.promotable(body, found.dgroup, bounds))
            out = promote.promoted(body, found.dgroup, bounds)
            was += sum(len(op.loads) + len(op.stores) for b in body.blocks for op in b.ops)
            now += sum(len(op.loads) + len(op.stores) for b in out.blocks for op in b.ops)
            # And it stays SSA: no phi is written here, `resolved` places them.
            assert not isinstance(mir.resolved(out, found.calls), str), f"{name} will not resolve"
    assert cells, "no cell is promotable in three programs that keep variables in memory"
    assert now < was, f"promotion removed no memory traffic ({was} -> {now})"


def test_promotion_is_only_sound_because_the_runtime_was_measured() -> None:
    """Every candidate is a cell in the program's own data, and a runtime
    call could write one -- until `runtime.toml` said which cells it can
    reach. With every call conceding `ANY`, nothing is promotable."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import promote
    from qbopt import runtime
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/arith-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    calls = {at: name for at, name in found.calls.items()}
    assert calls, "arith calls the runtime"
    assert any(runtime.contract(one).writes is runtime.Memory.OWN for one in calls.values()), (
        "no call in arith carries the measurement, so this proves nothing"
    )
    for _who, body in mir.bodies(found, blocks):
        assert promote.promotable(body, found.dgroup, module.landmarks(found))


def test_the_coalescer_joins_the_intervals_it_merges() -> None:
    """A value can be the destination of several copies.

    lngmix joins v3 with v9 and then, through the rename, v9 with v20.
    Checking each against the interval it started with says both are safe
    while their union is live across everything in between. LLVM's
    `RegisterCoalescer` joins the live intervals as it goes so the next
    join sees what the last one made.
    """
    from qbopt import coalesce
    from qbopt import intervals as ranges

    one = ranges.Interval(1, (ranges.Segment(15, 16), ranges.Segment(59, 60)))
    other = ranges.Interval(2, (ranges.Segment(0, 15),))
    assert not one.overlaps(other), "these abut and must not read as overlapping"

    both = coalesce._merged(one, other)
    assert both.segments == (ranges.Segment(0, 16), ranges.Segment(59, 60))
    # And a third value inside the union is now correctly refused.
    third = ranges.Interval(3, (ranges.Segment(4, 9),))
    assert not one.overlaps(third), "the original said nothing about this range"
    assert both.overlaps(third), "the merged interval must cover what it swallowed"


def test_an_absorbed_site_keeps_its_own_emission() -> None:
    """It is not the call it replaced, and lowering it to one loses its fixup.

    An absorbed site is seventeen bytes of `mov` and `idiv` that
    `select.absorbed` produces from `found.absorbed`. Lowering it gave it
    the semantics of the `call` it stands for, so `made` was set -- and
    layout then asked whether *that* still had an operand a fixup could sit
    in. A `call` with no operands does not, so the site's own relocation
    was dropped, the displacement stayed zero, and lngmix read the wrong
    address for `v`: `S= 50` where the answer is 142900.

    The whole LIR path miscompiled on this and nothing had ever run one of
    its objects.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import lower
    from qbopt import module
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    # After mir.bodies, not before: the raise is what folds a site, and
    # `found.absorbed` is empty until it has run.
    raised = list(mir.bodies(found, blocks))
    absorbed = set(found.absorbed)
    assert absorbed, "lngmix absorbs its divides; nothing was folded"
    seen = False
    for name, body in raised:
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        low = lower.lowered(name, body, found.calls, absorbed, runtime.for_module(found))
        for block in low.blocks:
            for one in block.insns:
                if one.op is not None and getattr(one.op, "id", None) in absorbed:
                    seen = True
                    assert one.what is None, f"{one.at:#06x} is an absorbed site and was lowered to something"
    assert seen, "no instruction in lngmix stands for an absorbed site"


def test_no_phi_survives_elimination_on_a_critical_edge() -> None:
    """bools-q-O has three, and every one was silently discarded."""
    from pathlib import Path

    from qbopt import omf
    from qbopt import lower
    from qbopt import module
    from qbopt import phielim
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/bools-q-O.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    raised = list(mir.bodies(found, blocks))
    absorbed = set(found.absorbed)
    critical = 0
    for name, body in raised:
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        low = lower.lowered(name, body, found.calls, absorbed, runtime.for_module(found))
        at_of = {block.at: block for block in low.blocks}
        for block in low.blocks:
            for phi in block.phis:
                if any(len(at_of[w].succ) > 1 for w, _v in phi.incoming if w in at_of):
                    critical += 1
        out = phielim.eliminated(low)
        left = [f"{block.at:#06x}" for block in out.blocks if block.phis]
        assert not left, f"{name}: a phi survives at {', '.join(left)}"
    assert critical >= 3, f"bools-q-O has three phis on critical edges; found {critical}"


def test_emission_refuses_a_body_that_still_has_a_phi() -> None:
    """A phi is not an instruction, so emitting one emits nothing."""
    from qbopt import lir
    from qbopt import objwrite

    stuck = lir.LirBody(
        name="one",
        entry=0,
        blocks=(lir.LirBlock(at=0, insns=(), phis=(lir.Phi(result=1, incoming=((0, 2),)),)),),
        origin={},
        pins={},
    )
    with pytest.raises(objwrite.Survived):
        objwrite._as_mir(stuck)


def test_verification_catches_a_cell_left_on_a_value_the_rename_ended() -> None:
    """The checker looked for a value in `Mem.through`, which holds a
    register, so a cell left on the old value was invisible to it: the
    three renames that got this wrong could not have been caught here."""
    from iced_x86 import Register

    from qbopt import lir
    from qbopt import verify
    from qbopt.module import Addr
    from qbopt.module import Space
    from qbopt import ir as machine

    where = Addr(Space.SEGMENT, 0x10, base=Register.SI)
    stale = machine.Mem(where, 2, Register.NONE, 0, 2, base=machine.Held(3, 2))
    reload_ = lir.Insn(
        at=0x20,
        covers=(0x20, 0x20),
        what=machine.Semantics(machine.Operation.MOVE, "mov", (machine.Held(6, 2),), (machine.Imm(0x40, 2),)),
        defines=(6,),
        uses=(),
        op=None,
    )
    load = lir.Insn(
        at=0x20,
        covers=(0x20, 0x22),
        what=machine.Semantics(machine.Operation.MOVE, "mov", (machine.Held(5, 2),), (stale,)),
        defines=(5,),
        uses=(6,),
        op=None,
    )
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(lir.LirBlock(at=0, insns=(reload_, load), succ=()),),
        origin={},
        pins={},
    )
    said = verify.verify(body)
    assert any("value#3" in one for one in said), f"the stale cell passed: {said}"
