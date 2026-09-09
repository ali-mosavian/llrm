"""What each operation requires of a register."""

from pathlib import Path

import pytest
from iced_x86 import Register

from qbopt import ir
from qbopt import lir
from qbopt import mir
from qbopt import omf
from qbopt import module
from qbopt import target
from qbopt import blocks as split
from qbopt.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def test_x87_memory_operands_still_need_address_registers() -> None:
    """nbody's FLD pointer was allocated to AX, which cannot address 16-bit memory."""
    what = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                        (ir.Mem(None, 4, through=Register.SI),))
    assert target.reads(what)[Register.ESI].where == frozenset(ir.ROOT[x] for x in target.ADDRESSING)


def test_string_copy_keeps_its_implicit_address_registers() -> None:
    """fpdeep printed DSQ=0 for 144: movsw lost the SI/DI addresses of its double copy."""
    from qbopt import lower
    from qbopt import runtime

    found = module.of(omf.parse(Path("fixtures/omf/fpdeep-p-g2.obj").read_bytes()))
    contracts = runtime.for_module(found)
    name, body = mir.bodies(found, split.partition(found, code_map(found)), contracts)[0]
    lowered = lower.lowered(name, body, found.calls, found.absorbed, contracts)
    copies = [
        one
        for block in lowered.blocks
        for one in block.insns
        if one.op is not None and one.op.kind is mir.Kind.OPAQUE and one.op.at in range(0x154, 0x158)
    ]
    assert len(copies) == 4
    for one in copies:
        assert {register for _, register in one.requires} == {Register.SI, Register.DI}
        assert {register for _, register in one.delivers} == {Register.SI, Register.DI}


def _semantics(op: mir.Op) -> ir.Semantics | None:
    return op.made if op.made is not None else getattr(op.node, "semantics", None)


@pytest.mark.parametrize("obj", FIXTURES, ids=lambda p: p.stem)
def test_bcs_own_assignment_satisfies_every_requirement(obj: Path) -> None:
    """The table is only worth having if the code it describes obeys it.

    BC put every value somewhere, and if this says an instruction needs one
    in a register BC did not use for it, the requirement is wrong. Two of
    them were: `bp` was left out of the addressing class, which made every
    `[bp-12h]` in the corpus a violation, and `fdivp` is modelled as a
    DIVIDE, so the widening rule claimed it reads dx:ax when there is
    nothing in ax to read.
    """
    found = module.of(omf.parse(obj.read_bytes()))
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return

    for name, body in mir.bodies(found, split.partition(found, mapped)):
        for block in body.blocks:
            for op in block.ops:
                what = _semantics(op)
                if what is None:
                    continue
                held = {ir.ROOT.get(body.origin.get(one, -1), -1) for one in op.uses}
                for want, need in target.reads(what).items():
                    if need.fixed is not None:
                        assert want in held, (
                            f"{obj.stem} {name}: {op.at:#x} {op.name} is said to need {want} and BC has no value there"
                        )
                    else:
                        assert want in {ir.ROOT.get(one, one) for one in need.where}, (
                            f"{obj.stem} {name}: {op.at:#x} {op.name} reaches memory by "
                            f"{want}, which the class does not permit"
                        )


def test_the_widening_forms_need_what_they_do_not_name() -> None:
    """`imul word [k]` multiplies by ax and says so nowhere.

    One destination written down is `imul r,r/m`, which names everything it
    touches. More than one is dx:ax, which names neither.
    """
    cell = ir.Mem(None, 2)
    ax = ir.Reg(register=Register.AX, width=2)
    dx = ir.Reg(register=Register.DX, width=2)

    named = ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax,), sources=(ax, cell))
    assert not any(need.fixed for need in target.reads(named).values()), "this one names its operands"

    wide = ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax, dx), sources=(cell,))
    assert target.reads(wide)[Register.EAX].fixed is Register.EAX
    assert target.writes(wide)[Register.EDX].fixed is Register.EDX

    # A divide reads both halves of the dividend.
    divide = ir.Semantics(ir.Operation.DIVIDE, "idiv", dests=(ax, dx), sources=(cell,))
    assert set(target.reads(divide)) == {Register.EAX, Register.EDX}

    # And x87 shares none of it, however `ir` happens to model the op.
    on_stack = ir.Semantics(ir.Operation.DIVIDE, "fdivp", dests=(ir.St(0),), sources=(ir.St(1),))
    assert target.reads(on_stack) == {} and target.writes(on_stack) == {}


def test_a_shift_by_a_register_takes_its_count_in_cl() -> None:
    """The one place a count may live, and the instruction does not say it."""
    ax = ir.Reg(register=Register.AX, width=2)
    cl = ir.Reg(register=Register.CL, width=1)

    by_one = ir.Semantics(ir.Operation.BINARY, "shl", dests=(ax,), sources=(ax, ir.Imm(value=1, width=1)))
    assert not any(need.fixed for need in target.reads(by_one).values())

    by_cl = ir.Semantics(ir.Operation.BINARY, "shl", dests=(ax,), sources=(ax, cl))
    assert target.reads(by_cl)[Register.ECX].fixed is Register.ECX


def test_the_addressing_class_is_what_the_encoding_permits() -> None:
    """Legal and assignable are different questions.

    16-bit addressing reaches memory through bx, bp, si or di; regalloc may
    not hand out bp because it is the frame pointer. Answering both with one
    set made every frame slot look like a violated requirement.
    """

    assert Register.BP in target.ADDRESSING, "a frame slot is reached through bp"
    assert Register.EBP not in target.BASES, "and the allocator may not hand it out"
    assert Register.DX not in target.ADDRESSING, "`[dx+0Ah]` has no encoding"


def test_a_byte_wide_held_is_the_low_byte() -> None:
    """al and ah are both one byte and both root to eax.

    AT_WIDTH was built by assignment, so whichever came last in the byte
    row won -- and that is ah. An ir.Held of width 1 resolved to `ah`,
    which is a different register holding a different byte, and nothing
    would have said so.
    """
    from iced_x86 import Register

    from qbopt import ir
    from qbopt import select

    for root, low in (
        (Register.EAX, Register.AL),
        (Register.EBX, Register.BL),
        (Register.ECX, Register.CL),
        (Register.EDX, Register.DL),
    ):
        assert select.AT_WIDTH[root][1] is low, f"{root} at one byte is not its low half"
    assert ir.ROOT[Register.AH] is Register.EAX, "the high byte still roots to eax"


def _one_body(stem: str):
    from qbopt import lower

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    return lower, name, body, found


def test_lowering_is_one_instruction_per_operation_unless_something_expands() -> None:
    """The default, and it has to stay exactly what it was: every operation
    is one instruction, carrying its own address, span and operation."""
    lower, name, body, found = _one_body("hotlop-p-g2")
    from qbopt import runtime

    low = lower.lowered(name, body, found.calls, None, runtime.for_module(found))
    ops = [op for block in body.blocks for op in block.ops]
    insns = [one for block in low.blocks for one in block.insns]
    assert len(insns) == len(ops)
    for one, op in zip(insns, ops, strict=True):
        assert (one.at, one.covers, one.op) == (op.at, op.covers, op)


def test_an_expansion_gives_its_leader_the_operation_and_its_followers_none(monkeypatch) -> None:
    """One operation, several instructions. The leader stands for the bytes
    and the rest stand for none: an inserted instruction claiming the same
    span made layout say a byte was held by more than one op.

    What each instruction reads and writes is its own, the leader included.
    A leader computing the expansion's first step while claiming the whole
    operation's operands tells the allocator the multiply's product is live
    from the load -- and the effect the operation had belongs to the run,
    not to any one instruction in it.
    """
    lower, name, body, found = _one_body("hotlop-p-g2")
    kind = next(op.kind for block in body.blocks for op in block.ops if op.kind is mir.Kind.ADD)

    def two(op, making):
        temp = ir.Held(making.fresh(), 4)
        made = next(one for one in op.defines if not one.flags)
        return (
            ir.Semantics(ir.Operation.MOVE, "mov", (temp,), (ir.Imm(1, 4),)),
            ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(made.id, 4),), (temp,)),
        )

    monkeypatch.setitem(lower._EXPANDS, kind, two)
    from qbopt import runtime

    low = lower.lowered(name, body, found.calls, None, runtime.for_module(found))
    runs = []
    for block in low.blocks:
        for index, one in enumerate(block.insns):
            if one.op is not None and one.op.kind is kind:
                runs.append((one, block.insns[index + 1]))
    assert runs

    for leader, follower in runs:
        original = {one.id for one in leader.op.defines if not one.flags}
        assert follower.covers == (follower.at, follower.at), "a follower stands for no bytes"
        assert follower.op is None and not follower.clobbers
        # Each instruction's own dataflow, from its own semantics.
        assert leader.uses == (), "the leader reads what its first step reads, not the operation's"
        assert set(leader.defines) & original == set(), "the leader makes a temporary, not the result"
        assert set(leader.defines) == set(follower.uses), "the temporary is what joins them"
        assert set(follower.defines) == original
        # And the run as a whole is what the operation was.
        assert original <= set(leader.defines) | set(follower.defines)


def test_an_expansion_invents_values_nothing_else_uses(monkeypatch) -> None:
    """A fresh id per call, and none of them one the body already had."""
    lower, name, body, found = _one_body("hotlop-p-g2")
    had = {one.id for block in body.blocks for op in block.ops for one in (*op.defines, *op.uses)}
    made = lower.Lowering(body, set(), {}, ())
    got = [made.fresh() for _ in range(8)]
    assert len(set(got)) == len(got)
    assert not (set(got) & had)


def test_an_allocatable_value_stays_a_value_through_lowering() -> None:
    """pressx printed R= 6460 for 7500.

    `_place` resolves a value to the register the original instruction
    had, because the MIR emitter applies an allocation only to the ops it
    re-encodes and would otherwise disagree with the ones it carries. The
    LIR path applies it to every instruction, so the same substitution
    leaves the allocator nothing to rewrite.

    The contract is about the boundary, not about the outcome: lowering
    must hand over abstract values. Whether two of them end up in one
    register is the allocation's business and is only wrong if their
    ranges overlap -- which is why this asserts on the lowered form and
    stops there.
    """
    from qbopt import lower
    from qbopt import transform

    found = module.of(omf.parse(Path("fixtures/omf/pressx-v-g3.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
    from qbopt import runtime

    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))

    # By what they compute, not by where they sit: an inserted instruction
    # carries its neighbour's address, so an address names a crowd.
    rows = [one for block in low.blocks for one in block.insns if one.what]
    pairs = [
        (one, rows[index + 1])
        for index, one in enumerate(rows[:-1])
        for what, then in ((one.what, rows[index + 1].what),)
        if what.op is ir.Operation.MOVE
        and len(what.sources) == 1
        and isinstance(what.sources[0], ir.Imm)
        and then.op is ir.Operation.MOVE
        and len(then.sources) == 1
        and isinstance(then.sources[0], ir.Mem)
    ]
    assert len(pairs) == 1, f"{len(pairs)} constant-then-load pairs, so this proves nothing"
    constant, load = pairs[0]
    assert all(isinstance(x, ir.Held) for x in constant.what.dests), (
        f"lowering resolved the constant to {constant.what.dests}"
    )
    assert all(isinstance(x, ir.Held) for x in load.what.dests), f"lowering resolved the load to {load.what.dests}"
    assert constant.what.dests[0].value != load.what.dests[0].value, "the constant and the load name one value"


def test_a_widened_operation_hands_over_values_like_every_other() -> None:
    """arith printed AND= 1544 for 33818120 -- the low word of the long.

    `pairs.widened` writes machine form: it recognises two 16-bit ANDs
    joined by a carry and puts one 32-bit `and eax,...` in `made`. Lowering
    returned that untouched, so a widened operation was the only one still
    naming BC's own registers while everything around it had become
    values. The allocator recorded what those instructions define and had
    no operand to rewrite, so its choice and the emitted register
    disagreed and the high word went.
    """
    from qbopt import lower
    from qbopt import transform

    found = module.of(omf.parse(Path("fixtures/omf/arith-v-g3.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
    from qbopt import runtime

    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
    wide = [
        one
        for block in low.blocks
        for one in block.insns
        if one.op is not None and one.op.made is not None and one.what
    ]
    assert wide, "nothing was widened here, so this proves nothing"
    for one in wide:
        named = [x for x in (*one.what.dests, *one.what.sources) if isinstance(x, (ir.Reg, ir.Held))]
        assert all(isinstance(x, ir.Held) for x in named), (
            f"{one.at:#06x} still names {[x for x in named if not isinstance(x, ir.Held)]}"
        )
        assert {x.value for x in one.what.dests if isinstance(x, ir.Held)} <= set(one.defines)
        assert {x.value for x in one.what.sources if isinstance(x, ir.Held)} <= set(one.uses)


def test_a_restore_keeps_the_idiom_it_stands_for() -> None:
    """`push eax / pop ax / pop dx` has no operands: the pair it names is
    the node's, and select emits fixed bytes for it.

    Valueizing what a pass put in `made` must leave it alone -- routing it
    through `semantics()` gave the restore three operands it never had and
    select emitted nothing, so the widened pair was never split back and
    arith pushed a stale high word.
    """
    from qbopt import lower
    from qbopt import select
    from qbopt import transform

    found = module.of(omf.parse(Path("fixtures/omf/arith-v-g3.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    name, body = next(iter(mir.bodies(found, blocks)))
    body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
    from qbopt import runtime

    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
    kept = [
        one for block in low.blocks for one in block.insns if one.op is not None and isinstance(one.op.node, ir.Restore)
    ]
    assert kept, "nothing was restored here, so this proves nothing"
    for one in kept:
        assert one.what is None or (not one.what.dests and not one.what.sources), (
            f"{one.at:#06x} gave the idiom operands: {one.what.dests} <- {one.what.sources}"
        )

    # And the assembler really does ask for the idiom. `select.emit` is not
    # on this path: a restore is a node, not a semantics, and asm reaches
    # it through its own branch.
    from qbopt import wholeseg

    asked: list[int] = []
    was = select.restore

    def spy(pair: int):
        asked.append(pair)
        return was(pair)

    select.restore = spy
    try:
        out, why = wholeseg.rebuilt(Path("fixtures/omf/arith-v-g3.obj").read_bytes())
    finally:
        select.restore = was
    assert why == wholeseg.REBUILT, why
    assert asked, "the assembler never asked for a restore"
    code = bytes(module.of(omf.parse(out)).code)
    for pair in set(asked):
        assert was(pair).code in code, f"the idiom for pair {pair} is not in the emitted bytes"


@pytest.mark.parametrize(
    ("stem", "at", "kind", "want"),
    [("cmpord-p-g2", 0xB2, "DECREMENT", "dec"), ("addrm-p-g2", 0x86, "INCREMENT", "inc")],
)
def test_an_increment_is_its_own_operation(stem: str, at: int, kind: str, want: str) -> None:
    """`dec ax` is not `ax - 1` written out: it leaves the carry alone
    where a subtraction writes it. Folded into ADD/SUB with the 1 made
    explicit, lowering had a shape with one operand and operands numbering
    two, and select refused -- `dec is not one select.py can emit` on
    three of the flow's programs.

    Measured evidence, not the reason this is safe: the flags are dead at
    all three sites (`Flag.NONE` live after each). The reason is that the
    operation now says what it is.
    """
    from qbopt import lower
    from qbopt import transform

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    for _name, body in mir.bodies(found, blocks):
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        for block in body.blocks:
            for op in block.ops:
                if op.at != at:
                    continue
                assert op.kind is getattr(mir.Kind, kind), f"raised as {op.kind}"
                assert len(op.args) == 1, f"the implicit operand was written out: {op.args}"
                what = lower.current(op, lower.as_a_value)
                assert what.op is ir.Operation.UNARY and what.name == want
                assert len(what.sources) == 1 and isinstance(what.sources[0], ir.Held)
                assert isinstance(what.dests[0], ir.Held)
                assert what.dests[0].value in one_of(op, "defines")
                assert what.sources[0].value in one_of(op, "uses")
                return
    pytest.fail(f"no operation at {at:#06x}")


def one_of(op, side: str) -> set:
    return {value.id for value in getattr(op, side) if not value.flags}


def test_a_stores_address_is_the_value_that_computed_it() -> None:
    """arrprm printed ' 0  0' for ' 7  8'.

    FILLNUMS computes each element's address into a register and stores
    through it. MIR says so -- the two stores name `v1_5` and `v1_7` --
    but `ir.Mem.through` held a register taken from the instruction BC
    wrote, and valueizing rewrote only top-level operands. The second
    store's address was allocated to ax and the store still encoded [bx].
    The first passed by luck: its value happened to land in bx.

    Witnessed on addrm-p-g2, a checked-in object with three such stores
    and every contract it calls established. arrprm's own object is a
    VBDOS build, and VBDOS's B$ENRA is deliberately unestablished -- its
    `bx` path reaches a `call far [di+24h]` no disassembly can follow --
    so the lowering refuses that body before reaching this at all.
    """
    from qbopt import lower
    from qbopt import transform

    found = module.of(omf.parse(Path("fixtures/omf/addrm-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    seen = 0
    for name, body in mir.bodies(found, blocks):
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        from qbopt import runtime

        low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
        for block in low.blocks:
            for one in block.insns:
                what = one.what
                if what is None:
                    continue
                for where in (*what.dests, *what.sources):
                    if not isinstance(where, ir.Mem) or where.base is None:
                        continue
                    seen += 1
                    assert where.base.value in one.uses, f"{one.at:#06x} reaches memory by a value it does not read"
        # every store whose MIR base named a value must carry it
        for block in body.blocks:
            for op in block.ops:
                for ref in op.stores:
                    if ref.base is None:
                        continue
                    made = [one for b in low.blocks for one in b.insns if one.op is op and one.what]
                    for one in made:
                        cell = next((x for x in one.what.dests if isinstance(x, ir.Mem)), None)
                        assert cell is not None and cell.base is not None, (
                            f"{op.at:#06x} lowered its base as {cell.through if cell else None}"
                        )
                        assert cell.base.value == ref.base.id
    assert seen >= 4, f"only {seen} based cells; addrm-p-g2 has four"


def test_a_lowered_cell_names_the_value_that_computed_its_address() -> None:
    """arrprm printed ' 0  0' for ' 7  8'.

    FILLNUMS computes each element's address into a register and stores
    through it. MIR says which value that is -- the two stores name
    different ones -- but the lowered cell kept the register BC wrote, so
    the allocator had nothing to rewrite: the second address was placed in
    ax and the store still encoded [bx].

    Constructed, so this holds without a compiled object to hand.
    """
    from iced_x86 import Register

    from qbopt import lower
    from qbopt.module import Addr
    from qbopt.module import Space
    from qbopt import ir as machine

    # A segment-relative element reached by a register, which is what
    # `_addressed` derives an encoding for.
    base = mir.Value(21, 0, 0, 1, 5)
    ref = mir.MemRef(addr=Addr(Space.SEGMENT, 0x10, base=Register.BX), width=2, base=base)
    stored = mir.Op(
        0x100,
        machine.Operation.MOVE,
        "mov",
        defines=(),
        uses=(base,),
        loads=(),
        stores=(ref,),
        node=None,
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(ref),),
        covers=(0x100, 0x104),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (stored,), ()),), {}, {})
    from qbopt import runtime

    low = lower.lowered("one", body, {}, set(), runtime.per_call({}))
    made = [one for block in low.blocks for one in block.insns if one.what]
    assert made, "the operation did not lower"
    cell = next(
        (where for one in made for where in (*one.what.dests, *one.what.sources) if isinstance(where, machine.Mem)),
        None,
    )
    assert cell is not None, "nothing lowered to a memory operand"
    assert cell.base is not None, f"the cell kept {cell.through} and named no value"
    assert cell.base.value == base.id
    assert base.id in made[0].uses, "the address is not recorded as a value it reads"
    # And nothing has placed it yet. Keeping BC's own register here makes
    # an unallocated operand indistinguishable from a placed one, which is
    # how arrprm's first store passed by luck.
    assert cell.through == Register.NONE, f"a based cell arrived placed in {cell.through}"


def _celled(base, through, at: int = 0x100):
    from iced_x86 import Register

    from qbopt import ir as machine

    del Register
    cell = machine.Mem("[bx]", 2, through, 0, 0, base=base)
    what = machine.Semantics(machine.Operation.MOVE, "mov", (machine.Held(30, 2),), (cell,))
    uses = (base.value,) if base is not None else ()
    return lir.LirBody(
        "one",
        0,
        (
            lir.LirBlock(
                at=0,
                insns=(lir.Insn(at=at, covers=(at, at + 2), what=what, defines=(30,), uses=uses, op=None),),
                succ=(),
            ),
        ),
        origin={},
        pins={},
    )


def test_an_address_value_takes_the_class_a_base_register_must_be_in() -> None:
    """`Mem.base` is the semantic address value and `Mem.through` is only
    how to encode it. A cell reached by a value must confine that value to
    the registers 16-bit addressing can name -- bx, bp, si, di -- and the
    encoding register must not decide it."""
    from iced_x86 import Register

    from qbopt import target
    from qbopt import allocate

    for through in (Register.BX, Register.SI):
        got = allocate.classes(_celled(ir.Held(21, 2), through))
        assert got.get(21) == target.ADDRESSING, f"through={through}: {got.get(21)}"


def test_an_unbased_cell_confines_no_value() -> None:
    from iced_x86 import Register

    from qbopt import allocate

    assert 21 not in allocate.classes(_celled(None, Register.BX))


def test_phi_elimination_renames_a_value_a_cell_is_reached_by() -> None:
    """A cell names the value that computed its address, and a rename has
    to reach it: `_settled` looked for a Held in `Mem.through`, which is a
    register now, so a based cell kept the old id.

    Against the helper and not a body, because no body reaches it: the
    rename only fires for a value read exactly once and that read is the
    phi edge, so no instruction operand names it. Over all 487 fixture
    objects, 583 bodies, `_settled` is called zero times.
    """
    from iced_x86 import Register

    from qbopt import phielim
    from qbopt.module import Addr
    from qbopt.module import Space
    from qbopt import ir as machine

    where = Addr(Space.SEGMENT, 0x10, base=Register.SI)
    cell = machine.Mem(where, 2, Register.NONE, 0, 2, base=machine.Held(21, 2))
    read = lir.Insn(
        at=0x100,
        covers=(0x100, 0x104),
        what=machine.Semantics(machine.Operation.MOVE, "mov", (machine.Held(30, 2),), (cell,)),
        defines=(30,),
        uses=(21,),
        op=None,
    )
    got = phielim._settled(read.what.sources[0], {21: 99})
    assert isinstance(got, machine.Mem)
    assert got.base == machine.Held(99, 2), f"the cell kept {got.base}"
    assert got.through == Register.NONE, "the rename placed it"
    for field in ("addr", "width", "offset", "disp_width"):
        assert getattr(got, field) == getattr(cell, field), field


def test_a_store_reads_the_value_its_cell_is_reached_by_and_writes_none() -> None:
    """`mov [es:bx],7` reads bx and writes no value at all.

    `defines` came from every value a destination operand named, and a
    cell names the value that computed its address -- so a store claimed
    to define the pointer it stores through. FILLNUMS lowered its element
    write with `defines=(21,)` and no use, which says the address is born
    at the store and dead before it, and the following `mov bx,4` -- which
    reads nothing -- inherited `uses=(21,)`.
    """
    from iced_x86 import Register

    from qbopt import lower
    from qbopt.module import Addr
    from qbopt.module import Space
    from qbopt import ir as machine

    base = mir.Value(21, 0, 0, 1, 5)
    # Segment-relative rather than the far cell FILLNUMS had: the space
    # decides the encoding, not what an instruction reads or writes.
    ref = mir.MemRef(addr=Addr(Space.SEGMENT, 0x10, base=Register.BX), width=2, base=base)
    stored = mir.Op(
        0x100,
        machine.Operation.MOVE,
        "mov",
        defines=(),
        uses=(base,),
        loads=(),
        stores=(ref,),
        node=None,
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(ref),),
        covers=(0x100, 0x104),
    )
    # `mov bx,4` after it: reads nothing, and inherited the store's address.
    made = mir.Value(22, 0, 0, 1, 6)
    after = mir.Op(
        0x104,
        machine.Operation.MOVE,
        "mov",
        defines=(made,),
        # The stale read the operation carried, which its own operands do
        # not name: `mov bx,4` reads nothing.
        uses=(base,),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.COPY,
        args=(mir.Const(4, 2),),
        results=(mir.Held(made, 2),),
        covers=(0x104, 0x107),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (stored, after), ()),), {}, {})
    from qbopt import runtime

    low = lower.lowered("one", body, {}, set(), runtime.per_call({}))
    out = [x for block in low.blocks for x in block.insns if x.what]
    one, next_one = out[0], out[1]
    assert one.uses == (base.id,), f"the store does not read its address: uses={one.uses}"
    assert one.defines == (), f"the store claims to write {one.defines}"
    assert base.id not in next_one.uses, f"the move after it inherited {base.id}: uses={next_one.uses}"


def test_call_with_inputs_keeps_its_implicit_result() -> None:
    """nbody read COMMAND$ from an unwritten spill slot and skipped its simulation."""
    from dataclasses import replace
    from iced_x86 import Register
    from qbopt import lower, runtime
    from qbopt import ir as machine

    source, result = mir.Value(1, 0, 0, 1, 1), mir.Value(2, 1, 0, 2, 2)
    call = mir.Op(0, machine.Operation.CALL, "call", (result,), (source,), (), (), None,
                  kind=mir.Kind.CALL, args=(mir.Held(source, 2),), covers=(0, 5))
    push = mir.Op(5, machine.Operation.PUSH, "push", (), (result,), (), (), None,
                  kind=mir.Kind.ARG, args=(mir.Held(result, 2),), covers=(5, 6))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (call, push), ()),),
                       {source: Register.EAX, result: Register.EAX}, {})
    contract = replace(runtime.worst("helper"), inputs=frozenset({runtime.Reg.AX}))
    low = lower.lowered("one", body, {0: "helper"}, set(), {0: contract})
    first, second = low.blocks[0].insns
    assert result.id in first.defines
    assert result.id in second.uses


def test_a_call_still_defines_the_results_its_operands_do_not_name() -> None:
    """A call's semantics names no operand, and its results are the
    operation's own -- the runtime hands them back in registers the call
    mentions nowhere.

    Deriving dataflow from operands alone made such a call define nothing,
    so a spilled call result had reloads and no store: FILLNUMS read
    `[bp-2]` twice with nothing having written it, and arrprm printed
    204996608. Operand roles decide wherever there are operands; an
    operation naming none keeps what it always said.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import lower
    from qbopt import module
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    # divmod, because bools stopped showing it: its only operand-free
    # operation is the terminating B$CENP, and once a call to a routine
    # whose contract reads no register stopped holding every caller value
    # live across it, nothing read that call's results either.
    found = module.of(omf.parse(Path("fixtures/omf/divmod-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    absorbed = set(found.absorbed)
    seen = []
    for name, body in mir.bodies(found, blocks):
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        from qbopt import runtime

        low = lower.lowered(name, body, found.calls, absorbed, runtime.for_module(found))
        # A result nothing reads is deliberately not recorded -- a Held the
        # allocator never hears of. The ones read are the question.
        read = {value for block in low.blocks for one in block.insns for value in one.uses}
        for block in low.blocks:
            for one in block.insns:
                if one.what is None or one.what.dests or one.what.sources or one.op is None:
                    continue
                owed = [value.id for value in one.op.defines if not value.flags and value.id in read]
                if owed:
                    seen.append((name, one.at, tuple(owed), one.defines))
    assert seen, "no operand-free operation with results; the fixture cannot show this"
    bad = [one for one in seen if not one[3]]
    assert not bad, f"{bad[0][0]} {bad[0][1]:#06x}: defines {bad[0][3]} where the operation defines {bad[0][2]}"


def test_a_folded_divide_says_where_its_two_answers_arrive() -> None:
    """The call is gone and the operands name neither register.

    A folded site leaves its quotient in the one the routine returned in
    and its remainder in the one calls.py keeps the other in, and neither
    is anywhere in what the operation reads. Undeclared, the allocation
    put the results wherever it liked, so the only way to emit the site
    was the sequence frozen at the raise -- which is why a divide that had
    been hoisted could not be emitted at all.
    """
    from pathlib import Path

    from qbopt import omf
    from qbopt import lower
    from qbopt import module
    from qbopt import runtime
    from qbopt import transform
    from qbopt import blocks as split
    from qbopt.blocks import code_map
    from qbopt import calls as machine

    found = module.of(omf.parse(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    seen = []
    for name, body in mir.bodies(found, blocks):
        body = transform.widened(transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found))
        low = lower.lowered(name, body, found.calls, found.absorbed, runtime.for_module(found))
        for block in low.blocks:
            for one in block.insns:
                if one.op is None or one.op.kind is not mir.Kind.DIVMOD:
                    continue
                site = found.absorbed[one.op.id][0]
                other = machine.other_result(site)
                pair = (machine.RESULT, other)
                want = pair[::-1] if site.name.upper() == machine.REMAINDER else pair
                seen.append((name, one.at))
                got = tuple(register for _held, register in one.delivers)
                assert got == want, f"{name} {one.at:#06x}: answers said to arrive in {got}, not {want}"
                assert tuple(held.value for held, _r in one.delivers) == one.defines
    assert seen, "no folded divide here, so this proves nothing"


def test_a_call_to_an_unestablished_routine_is_refused() -> None:
    """A conservative dependency is not an argument, and pretending the
    routine reads nothing is worse than refusing the body.

    B$ENRA's code is not in the runtime tree, so `Contract.inputs` is None
    and the raise reads every tracked register as an input -- "every
    register is an input until it is". Constraining that whole set pins
    the register file; constraining none of it lets the allocation move
    arguments the routine really does read, which is what put arrprm's
    `mov cx,0` / `mov bx,0` into dx and di. Neither is emittable, so the
    body keeps BC's own layout.
    """
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import runtime
    from qbopt import ir as machine

    assert runtime.contract("B$ENRA").inputs is None, "B$ENRA's inputs are established now"

    first, second = mir.Value(2, 0, 0, 1, 5), mir.Value(4, 0, 0, 1, 6)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(first, second),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.CALL,
        args=(),
        results=(),
        covers=(0x106, 0x10B),
    )
    body = mir.MirBody(
        0x106,
        (mir.MirBlock(0x106, (), (called,), ()),),
        {first: Register.ECX, second: Register.EBX},
        {},
    )
    import pytest

    with pytest.raises(lower.Unlowered, match="no established inputs"):
        lower.lowered("one", body, {0x106: "B$ENRA"}, set(), runtime.per_call({0x106: "B$ENRA"}))


def test_a_call_argument_is_required_where_the_contract_reads_it() -> None:
    """The slot decides, not where the value happens to live.

    B$FILD reads a long in dx:ax. A pass may hand it a value computed
    anywhere -- reading the origin instead would have required each
    argument back in whichever register it started in, which is not the
    routine's contract and is what pinned the whole register file at every
    call to a routine that declares nothing.
    """
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import runtime
    from qbopt import ir as machine

    assert runtime.slots(runtime.contract("B$FILD")) == (runtime.Reg.AX, runtime.Reg.DX)

    low_half, high_half = mir.Value(2, 0, 0, 1, 5), mir.Value(4, 0, 0, 1, 6)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(low_half, high_half),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.CALL,
        args=(mir.Held(low_half, 2), mir.Held(high_half, 2)),
        results=(),
        covers=(0x106, 0x10B),
    )
    body = mir.MirBody(
        0x106,
        (mir.MirBlock(0x106, (), (called,), ()),),
        # Neither argument is where the routine reads it.
        {low_half: Register.ESI, high_half: Register.EDI},
        {},
    )
    low = lower.lowered("one", body, {0x106: "B$FILD"}, set(), runtime.per_call({0x106: "B$FILD"}))
    call = next(one for block in low.blocks for one in block.insns if one.op is called)
    assert call.requires == (
        (machine.Held(low_half.id, 2), Register.AX),
        (machine.Held(high_half.id, 2), Register.DX),
    ), f"the call requires {call.requires}"


def test_a_declared_contract_its_arguments_do_not_answer_is_refused() -> None:
    """Emitting the call unconstrained would say B$FILD reads nothing,
    which is the one thing known to be false about it."""
    import pytest
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.CALL,
        args=(mir.Held(only, 2),),  # one, where the contract declares two
        results=(),
        covers=(0x106, 0x10B),
    )
    body = mir.MirBody(0x106, (mir.MirBlock(0x106, (), (called,), ()),), {only: Register.ESI}, {})
    with pytest.raises(lower.Unlowered, match="1 arguments for 2 declared inputs"):
        from qbopt import runtime

        lower.lowered("one", body, {0x106: "B$FILD"}, set(), runtime.per_call({0x106: "B$FILD"}))


def test_lowering_a_call_the_caller_chose_no_contract_for_is_refused() -> None:
    """Both boundaries read one map, or they can disagree.

    The raise picks a contract per call site and the lowering has to use
    that one -- looking it up again by name is how a per-family variant
    would be applied on one side and not the other. A call the map does
    not cover is a call nobody decided about.
    """
    import pytest
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import runtime
    from qbopt import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.CALL,
        args=(),
        results=(),
        covers=(0x106, 0x10B),
    )
    body = mir.MirBody(0x106, (mir.MirBlock(0x106, (), (called,), ()),), {only: Register.ECX}, {})
    with pytest.raises(lower.Unlowered, match="no contract for"):
        lower.lowered("one", body, {0x106: "B$FILD"}, set(), {})
    # And with the map the caller built, it is the map's answer that runs.
    with pytest.raises(lower.Unlowered, match="2 declared inputs"):
        lower.lowered("one", body, {0x106: "B$FILD"}, set(), runtime.per_call({0x106: "B$FILD"}))


def test_a_call_marked_interface_unknown_is_refused_on_that_alone() -> None:
    """`args_known` is the fact, not the absence of a contract.

    A call site with no runtime contract also refuses, so a body-level
    test cannot tell the two apart. Here the map holds a contract and the
    operation still says its interface is unestablished -- which is the
    state B$ENRA raises -- and that alone must refuse.
    """
    import pytest
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import runtime
    from qbopt import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.CALL,
        args=(),
        results=(),
        covers=(0x106, 0x10B),
        args_known=False,
    )
    body = mir.MirBody(0x106, (mir.MirBlock(0x106, (), (called,), ()),), {only: Register.ECX}, {})
    # A contract the map does hold, so "no contract" cannot be the reason.
    where = runtime.per_call({0x106: "B$PEI2"})
    assert where[0x106].inputs == frozenset()
    with pytest.raises(lower.Unlowered, match="interface is not established"):
        lower.lowered("one", body, {0x106: "B$PEI2"}, set(), where)


def test_a_phi_chain_nothing_reads_does_not_reach_lir() -> None:
    """The raise puts a phi on every register live around a loop.

    Most are read by nothing once an operation says what it computes, and
    `phielim` materialises a copy on every edge for each -- including an
    edge whose value has no definition. A phi feeding a live phi is live;
    one whose chain ends in no reader is not.
    """
    from iced_x86 import Register

    from qbopt import lower
    from qbopt import runtime
    from qbopt import ir as machine

    kept, dead, feeder, unread = (
        mir.Value(2, 0x20, 0, 1, 1),
        mir.Value(3, 0x20, 0, 2, 1),
        mir.Value(4, 0, 0, 1, 2),
        mir.Value(5, 0, 0, 2, 2),
    )
    made = [
        mir.Op(
            0x10 + step * 3,
            machine.Operation.MOVE,
            "mov",
            defines=(one,),
            uses=(),
            loads=(),
            stores=(),
            node=None,
            kind=mir.Kind.COPY,
            args=(mir.Const(step, 2),),
            results=(mir.Held(one, 2),),
            covers=(0x10 + step * 3, 0x13 + step * 3),
        )
        for step, one in enumerate((feeder, unread))
    ]
    # One reader, of the kept phi's result only.
    reads = mir.Op(
        0x20,
        machine.Operation.MOVE,
        "mov",
        defines=(mir.Value(9, 0x20, 0, 3, 1),),
        uses=(kept,),
        loads=(),
        stores=(),
        node=None,
        kind=mir.Kind.COPY,
        args=(mir.Held(kept, 2),),
        results=(mir.Held(mir.Value(9, 0x20, 0, 3, 1), 2),),
        covers=(0x20, 0x22),
    )
    # phi A -> phi B -> the reader, so keeping B must keep A; and one
    # phi whose chain ends in nobody.
    middle = mir.Value(6, 0x18, 0, 1, 3)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), tuple(made), (0x18,)),
            mir.MirBlock(0x18, (mir.Phi(middle, {0: feeder}),), (), (0x20,)),
            mir.MirBlock(
                0x20,
                (
                    mir.Phi(kept, {0x18: middle}),
                    mir.Phi(dead, {0x18: unread}),
                ),
                (reads,),
                (),
            ),
        ),
        {
            feeder: Register.EAX,
            unread: Register.ECX,
            kept: Register.EAX,
            dead: Register.ECX,
            middle: Register.EAX,
        },
        {},
    )
    low = lower.lowered("one", body, {}, set(), runtime.per_call({}))
    left = {phi.result for block in low.blocks for phi in block.phis}
    assert kept.id in left, "the phi its own reader needs was dropped"
    assert middle.id in left, f"the phi feeding it was dropped: {sorted(left)}"
    assert dead.id not in left, f"a phi nothing reads reached LIR: {sorted(left)}"


def test_a_reused_divide_s_copy_lowers_to_a_move_and_not_to_nothing() -> None:
    """A folded site's id outlives the operation that was folded.

    The copy keeps it so the bytes the site stood for go on being
    accounted for, and lowering read the id alone as "the site's own
    sequence emits this" -- so the copy came out with nothing to emit, and
    every body holding one left the LIR route for the allocator that
    cannot spill with `mov is not one select.py can emit`.
    """
    from pathlib import Path

    from qbopt import ir
    from qbopt import mir
    from qbopt import omf
    from qbopt import lower
    from qbopt import module
    from qbopt import runtime
    from qbopt import transform
    from qbopt.passes import Where
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/lngmix-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)
    where = Where(
        dgroup=found.dgroup,
        calls=found.calls,
        bounds=module.landmarks(found),
        blocks=blocks,
        found=found,
    )
    ((name, body),) = mir.bodies(found, blocks)
    for one in transform.pipeline(where):
        body = one.transform(body)

    copies = [
        op for block in body.blocks for op in block.ops if op.kind is mir.Kind.COPY and op.id in set(found.absorbed)
    ]
    assert len(copies) == 1, f"lngmix folds one divide into a copy; {len(copies)} found"

    low = lower.lowered(name, body, found.calls, set(found.absorbed), runtime.for_module(found))
    made = [one for one in low.insns if one.op is copies[0]]
    assert made, "the copy reached no instruction at all"
    assert made[0].what is not None, "the copy lowered to nothing; the site's id is not the operation"
    assert made[0].what.op is ir.Operation.MOVE


def test_two_address_materializes_a_constant_first_operand() -> None:
    """hotlop printed 420 for 630 when 21 + accumulator lost its 21."""
    from qbopt import ir
    from qbopt import lir
    from qbopt import twoaddr

    result, source = ir.Held(900, 2), ir.Held(901, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.BINARY, "add", (result,), (ir.Imm(21, 2), source)),
        defines=(900,),
        uses=(901,),
    )
    fixed = twoaddr._untied(insn)
    assert fixed is not None
    assert fixed[0].what.sources == (ir.Imm(21, 2),)
    assert fixed[0].uses == ()
    assert fixed[1].what.sources == (result, source)


def test_two_address_multiply_preserves_its_first_factor() -> None:
    """Experimental matrix setup emitted 20 * 20 for 0 * 20 without a destination copy."""
    from qbopt import twoaddr

    result, first, second = ir.Held(900, 2), ir.Held(901, 2), ir.Held(902, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MULTIPLY, "imul", (result,), (first, second)),
        defines=(900,),
        uses=(901, 902),
    )
    fixed = twoaddr._untied(insn)
    assert fixed is not None
    assert fixed[0].what.sources == (first,)
    assert fixed[1].what.sources == (result, second)
    assert set(fixed[1].uses) == {900, 902}


def test_constant_multiply_lowers_without_a_destination_tie() -> None:
    from qbopt import lower
    from qbopt import twoaddr

    source, result = mir.Value(900, 0), mir.Value(901, 1)
    op = mir.Op(
        1,
        ir.Operation.MULTIPLY,
        "",
        (result,),
        (source,),
        kind=mir.Kind.MUL,
        args=(mir.Held(source, 2), mir.Const(20, 2)),
        results=(mir.Held(result, 2),),
    )
    what = lower.semantics(op, place=lower.as_a_value)
    assert what.sources[1:] == (ir.Held(source.id, 2), ir.Imm(20, 2))
    assert twoaddr._untied(lir.Insn(at=1, covers=(1, 1), what=what, defines=(result.id,), uses=(source.id,))) is None


def test_lower_places_a_commutative_constant_in_the_immediate_operand() -> None:
    """hotlop needlessly loaded 21 before adding its accumulator."""
    from qbopt import ir
    from qbopt import mir
    from qbopt import lower

    result, source = mir.Value(900, 0), mir.Value(901, 0)
    op = mir.Op(
        0,
        ir.Operation.BINARY,
        "add",
        (result,),
        (source,),
        kind=mir.Kind.ADD,
        args=(mir.Const(21, 2), mir.Held(source, 2)),
        results=(mir.Held(result, 2),),
    )
    what = lower.semantics(op, place=lower.as_a_value)
    assert what.sources == (ir.Held(source.id, 2), ir.Imm(21, 2))


def test_two_address_copy_ends_the_original_source_use() -> None:
    """hotlop kept its old accumulator live through the add after copying it."""
    from qbopt import ir
    from qbopt import lir
    from qbopt import twoaddr

    result, source = ir.Held(900, 2), ir.Held(901, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.BINARY, "add", (result,), (source, ir.Imm(21, 2))),
        defines=(900,),
        uses=(901,),
    )
    fixed = twoaddr._untied(insn)
    assert fixed[1].uses == (900,)
