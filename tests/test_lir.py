"""What each operation requires of a register."""

from pathlib import Path

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.objectfile import module
from qbopt.backend import target
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def test_preselected_memory_keeps_its_address_value() -> None:
    """nbody widened posY(other) at 0x125 and read through stale SI: PX0=-4385 for 1258."""
    from qbopt.backend import lower
    from qbopt.objectfile.module import Addr, Space

    pointer, result = mir.Value(1, 0, 0, 1, 1), mir.Value(2, 0, 0, 1, 2)
    addr = Addr(Space.SEGMENT, 0, base=Register.SI)
    cell = mir.Cell(mir.MemRef(addr, 4, base=pointer, base_width=2))
    op = mir.Op(0, ir.Operation.MOVE, "mov", (result,), (pointer,), (cell.ref,), (), None,
                kind=mir.Kind.COPY, args=(cell,), results=(mir.Held(result, 4),))
    selected = lower.current(op, lower.as_a_value)
    assert selected.sources[0].base == ir.Held(pointer.id, 2)


def test_x87_memory_operands_still_need_address_registers() -> None:
    """nbody's FLD pointer was allocated to AX, which cannot address 16-bit memory."""
    what = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                        (ir.Mem(None, 4, through=Register.SI),))
    assert target.reads(what)[Register.ESI].where == frozenset(ir.ROOT[x] for x in target.ADDRESSING)


def test_string_copy_keeps_its_implicit_address_registers() -> None:
    """fpdeep printed DSQ=0 for 144: movsw lost the SI/DI addresses of its double copy."""
    from qbopt.backend import lower
    from qbopt.abi import runtime

    found = module.of(omf.parse(Path("fixtures/omf/fpdeep-p-g2.obj").read_bytes()))
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, split.partition(found, code_map(found)), contracts)
    name, body = raised[0]
    lowered = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )
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


def test_an_opaque_address_keeps_the_registers_it_is_written_in() -> None:
    """hotlpx rebuilt twice printed S=250 for 630: `lea ax,[ebx+ebx*4]` pinned
    nothing, and the product it reads was allocated to ax. The first rebuild
    may choose another legal source register, so the decoded address—not an
    historical register name—is the oracle for the requirement."""
    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.rewrite import rewrite

    found = module.of(omf.parse(rewrite(Path("fixtures/omf/hotlpx-p-g2.obj").read_bytes(), dry_run=False)[0]))
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, split.partition(found, code_map(found)), contracts)
    name, body = raised[0]
    lowered = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )
    (lea,) = [
        one
        for block in lowered.blocks
        for one in block.insns
        if one.op is not None and one.op.kind is mir.Kind.ADDRESS
    ]
    address = lea.node.semantics.sources[0]
    expected = {
        ir.ROOT[register]
        for register in (address.through, address.index)
        if register != Register.NONE
    }
    assert expected, "the fixture no longer has a register-based address"
    assert {ir.ROOT[register] for _, register in lea.requires} == expected


def test_a_procedure_hands_back_dx_ax() -> None:
    """procs p-ot's TWICE& left its answer in bx and ax, and its callers read dx:ax."""
    from qbopt.backend import lower
    from qbopt.abi import runtime

    found = module.of(omf.parse(Path("fixtures/omf/procs-p-ot.obj").read_bytes()))
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, split.partition(found, code_map(found)), contracts)
    name, body = next(one for one in raised if "TWICE" in one[0])
    lowered = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )
    (ret,) = [
        one
        for block in lowered.blocks
        for one in block.insns
        if one.op is not None and one.op.kind is mir.Kind.RETURN
    ]
    assert {register for _, register in ret.requires} == {Register.AX, Register.DX}


def _semantics(op: mir.Op, nodes: dict[int, object]) -> ir.Semantics | None:
    from qbopt.backend import lower

    return lower.current(op, node=nodes.get(op.id))


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

    raised = mir.bodies(found, split.partition(found, mapped))
    for name, body in raised:
        for block in body.blocks:
            written: set = set()
            for op in block.ops:
                # `cwd` before `idiv` is raised as a sign extension the
                # divide no longer names; dx still holds what BC put there.
                held = {ir.ROOT.get(body.origin.get(one, -1), -1) for one in op.uses} | written
                written |= {ir.ROOT.get(body.origin.get(one, -1), -1) for one in op.defines}
                # Recognition can replace BC's call sequence with a new
                # machine-independent operation (for example DIVMOD). Its
                # eventual fixed-register requirements belong to our
                # lowering and allocator, not to BC's historical assignment.
                if not op.source_backed:
                    continue
                what = _semantics(op, raised.source.nodes)
                if what is None:
                    continue
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

    from qbopt.model import ir
    from qbopt.backend import select

    for root, low in (
        (Register.EAX, Register.AL),
        (Register.EBX, Register.BL),
        (Register.ECX, Register.CL),
        (Register.EDX, Register.DL),
    ):
        assert select.AT_WIDTH[root][1] is low, f"{root} at one byte is not its low half"
    assert ir.ROOT[Register.AH] is Register.EAX, "the high byte still roots to eax"


def _one_body(stem: str):
    from qbopt.backend import lower

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    raised = mir.bodies(found, blocks)
    name, body = next(iter(raised))
    return lower, name, body, found, raised.source


def test_lowering_is_one_instruction_per_operation_unless_something_expands() -> None:
    """The default, and it has to stay exactly what it was: every operation
    is one instruction, carrying its own address, span and operation."""
    lower, name, body, found, source = _one_body("hotlop-p-g2")
    from qbopt.abi import runtime

    low = lower.lowered(
        name,
        body,
        found.calls,
        source.absorbed,
        runtime.for_module(found),
        source.coverage,
        nodes=source.nodes,
        occurrences=source.occurrences,
    )
    ops = [op for block in body.blocks for op in block.ops]
    insns = [one for block in low.blocks for one in block.insns]
    assert len(insns) == len(ops)
    for one, op in zip(insns, ops, strict=True):
        owned = tuple(span for identity in op.absorbed for span in source.occurrences[identity])
        assert one.at == op.at and one.op == op
        expected = {byte for low, high in owned for byte in range(low, high)}
        actual = {
            byte
            for low, high in (*((one.covers,) if one.covers is not None else ()), *one.spread)
            for byte in range(low, high)
        }
        assert actual == expected


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
    lower, name, body, found, source = _one_body("hotlop-p-g2")
    kind = next(op.kind for block in body.blocks for op in block.ops if op.kind is mir.Kind.ADD)

    def two(op, making):
        temp = ir.Held(making.fresh(), 4)
        made = next(one for one in op.defines if not one.flags)
        return (
            ir.Semantics(ir.Operation.MOVE, "mov", (temp,), (ir.Imm(1, 4),)),
            ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(made.id, 4),), (temp,)),
        )

    monkeypatch.setitem(lower._EXPANDS, kind, two)
    from qbopt.abi import runtime

    low = lower.lowered(
        name,
        body,
        found.calls,
        source.absorbed,
        runtime.for_module(found),
        source.coverage,
        nodes=source.nodes,
        occurrences=source.occurrences,
    )
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
    lower, name, body, found, _source = _one_body("hotlop-p-g2")
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
    from qbopt.backend import lower
    from qbopt.optimize import transform

    # harr: pressx no longer loads a constant and then a cell in a row.
    found = module.of(omf.parse(Path("fixtures/omf/harr-v-g3.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    raised = mir.bodies(found, blocks)
    name, body = next(iter(raised))
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    from qbopt.abi import runtime

    low = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        runtime.for_module(found),
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )

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


@pytest.mark.parametrize(
    ("stem", "at", "kind", "want"),
    [("addrm-p-g2", 0x86, "INCREMENT", "inc")],
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
    from qbopt.backend import lower

    found = module.of(omf.parse((Path("fixtures/omf") / f"{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    raised = mir.bodies(found, blocks)
    expected = getattr(mir.Kind, kind)
    for _name, body in raised:
        for block in body.blocks:
            for op in block.ops:
                if op.at != at or op.kind is not expected:
                    continue
                assert len(op.args) == 1, f"the implicit operand was written out: {op.args}"
                what = lower.current(op, lower.as_a_value, node=raised.source.nodes.get(op.id))
                assert what.op is ir.Operation.UNARY and what.name == want
                assert len(what.sources) == 1 and isinstance(what.sources[0], ir.Held)
                assert isinstance(what.dests[0], ir.Held)
                assert what.dests[0].value in one_of(op, "defines")
                assert what.sources[0].value in one_of(op, "uses")
                return
    pytest.fail(f"no {expected.value} operation at {at:#06x}")


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
    from qbopt.backend import lower
    from qbopt.optimize import transform

    found = module.of(omf.parse(Path("fixtures/omf/addrm-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    seen = 0
    raised = mir.bodies(found, blocks)
    for name, body in raised:
        body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        from qbopt.abi import runtime

        low = lower.lowered(
            name,
            body,
            found.calls,
            raised.source.absorbed,
            runtime.for_module(found),
            raised.source.coverage,
            nodes=raised.source.nodes,
            occurrences=raised.source.occurrences,
        )
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
    assert seen >= 3, f"only {seen} based cells; addrm-p-g2 has three"


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

    from qbopt.backend import lower
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space
    from qbopt.model import ir as machine

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
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(ref),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (stored,), ()),), {}, {})
    from qbopt.abi import runtime

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

    from qbopt.model import ir as machine

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

    from qbopt.backend import target
    from qbopt.backend import allocate

    for through in (Register.BX, Register.SI):
        got = allocate.classes(_celled(ir.Held(21, 2), through))
        assert got.get(21) == target.ADDRESSING, f"through={through}: {got.get(21)}"


def test_an_unbased_cell_confines_no_value() -> None:
    from iced_x86 import Register

    from qbopt.backend import allocate

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

    from qbopt.backend import phielim
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space
    from qbopt.model import ir as machine

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

    from qbopt.backend import lower
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space
    from qbopt.model import ir as machine

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
        kind=mir.Kind.STORE,
        args=(mir.Const(7, 2),),
        results=(mir.Cell(ref),),
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
        kind=mir.Kind.COPY,
        args=(mir.Const(4, 2),),
        results=(mir.Held(made, 2),),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (stored, after), ()),), {}, {})
    from qbopt.abi import runtime

    low = lower.lowered("one", body, {}, set(), runtime.per_call({}))
    out = [x for block in low.blocks for x in block.insns if x.what]
    one, next_one = out[0], out[1]
    assert one.uses == (base.id,), f"the store does not read its address: uses={one.uses}"
    assert one.defines == (), f"the store claims to write {one.defines}"
    assert base.id not in next_one.uses, f"the move after it inherited {base.id}: uses={next_one.uses}"


def test_word_concatenation_lowers_high_then_low_without_register_assumptions() -> None:
    from qbopt.backend import lower
    from qbopt.model import ir as machine

    high, low, result = mir.Value(901, 0), mir.Value(902, 0), mir.Value(903, 1)
    op = mir.Op(1, mir.Synth.CONCAT_LOW, "concat", (result,), (high, low),
                kind=mir.Kind.CONCAT, args=(mir.Held(high, 2), mir.Held(low, 2)),
                results=(mir.Held(result, 4),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),), {})
    lowered = lower.lowered("concat", body, {}, set(), {})
    insns = lowered.blocks[0].insns
    assert [one.what.op for one in insns] == [machine.Operation.PUSH, machine.Operation.PUSH, machine.Operation.POP]
    assert insns[0].what.sources == (machine.Held(high.id, 2),)
    assert insns[1].what.sources == (machine.Held(low.id, 2),)
    assert insns[2].what.dests == (machine.Held(result.id, 4),)
    assert not any(one.requires or one.delivers or one.clobbers for one in insns)


def test_call_with_inputs_keeps_its_implicit_result() -> None:
    """nbody read COMMAND$ from an unwritten spill slot and skipped its simulation."""
    from dataclasses import replace
    from iced_x86 import Register
    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

    source, result = mir.Value(1, 0, 0, 1, 1), mir.Value(2, 1, 0, 2, 2)
    call = mir.Op(0, machine.Operation.CALL, "call", (result,), (source,), (), (), None,
                  kind=mir.Kind.CALL, args=(mir.Held(source, 2),))
    push = mir.Op(5, machine.Operation.PUSH, "push", (), (result,), (), (), None,
                  kind=mir.Kind.ARG, args=(mir.Held(result, 2),))
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

    from qbopt.objectfile import omf
    from qbopt.backend import lower
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    # divmod, because bools stopped showing it: its only operand-free
    # operation is the terminating B$CENP, and once a call to a routine
    # whose contract reads no register stopped holding every caller value
    # live across it, nothing read that call's results either.
    found = module.of(omf.parse(Path("fixtures/omf/divmod-p-g2.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    seen = []
    raised = mir.bodies(found, blocks)
    for name, body in raised:
        body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        from qbopt.abi import runtime

        low = lower.lowered(
            name,
            body,
            found.calls,
            raised.source.absorbed,
            runtime.for_module(found),
            raised.source.coverage,
            nodes=raised.source.nodes,
            occurrences=raised.source.occurrences,
        )
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

    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

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
        kind=mir.Kind.CALL,
        args=(),
        results=(),
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

    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

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
        kind=mir.Kind.CALL,
        args=(mir.Held(low_half, 2), mir.Held(high_half, 2)),
        results=(),
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

    from qbopt.backend import lower
    from qbopt.model import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        kind=mir.Kind.CALL,
        args=(mir.Held(only, 2),),  # one, where the contract declares two
        results=(),
    )
    body = mir.MirBody(0x106, (mir.MirBlock(0x106, (), (called,), ()),), {only: Register.ESI}, {})
    with pytest.raises(lower.Unlowered, match="1 arguments for 2 declared inputs"):
        from qbopt.abi import runtime

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

    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        kind=mir.Kind.CALL,
        args=(),
        results=(),
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

    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

    only = mir.Value(2, 0, 0, 1, 5)
    called = mir.Op(
        0x106,
        machine.Operation.CALL,
        "call",
        defines=(),
        uses=(only,),
        loads=(),
        stores=(),
        kind=mir.Kind.CALL,
        args=(),
        results=(),
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

    from qbopt.backend import lower
    from qbopt.abi import runtime
    from qbopt.model import ir as machine

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
            kind=mir.Kind.COPY,
            args=(mir.Const(step, 2),),
            results=(mir.Held(one, 2),),
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
        kind=mir.Kind.COPY,
        args=(mir.Held(kept, 2),),
        results=(mir.Held(mir.Value(9, 0x20, 0, 3, 1), 2),),
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


def test_two_address_materializes_a_constant_first_operand() -> None:
    """hotlop printed 420 for 630 when 21 + accumulator lost its 21."""
    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import twoaddr

    result, source = ir.Held(900, 2), ir.Held(901, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.BINARY, "add", (result,), (ir.Imm(21, 2), source)),
        defines=(900,),
        uses=(901,),
    )
    fixed = twoaddr._untied(insn, iter(range(1000, 2000)).__next__)
    assert fixed is not None
    assert fixed[0].what.sources == (ir.Imm(21, 2),)
    assert fixed[0].uses == ()
    assert fixed[1].what.sources == (result, source)


def test_two_address_multiply_preserves_its_first_factor() -> None:
    """Experimental matrix setup emitted 20 * 20 for 0 * 20 without a destination copy."""
    from qbopt.backend import twoaddr

    result, first, second = ir.Held(900, 2), ir.Held(901, 2), ir.Held(902, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MULTIPLY, "imul", (result,), (first, second)),
        defines=(900,),
        uses=(901, 902),
    )
    fixed = twoaddr._untied(insn, iter(range(1000, 2000)).__next__)
    assert fixed is not None
    assert fixed[0].what.sources == (first,)
    assert fixed[1].what.sources == (result, second)
    assert set(fixed[1].uses) == {900, 902}


def test_constant_multiply_lowers_without_a_destination_tie() -> None:
    from qbopt.backend import lower
    from qbopt.backend import twoaddr

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
    insn = lir.Insn(at=1, covers=(1, 1), what=what, defines=(result.id,), uses=(source.id,))
    assert twoaddr._untied(insn, iter(range(1000, 2000)).__next__) is None


@pytest.mark.parametrize("width,factor", [(2, 3), (2, 10), (2, 20), (2, 32769), (4, 20), (4, 2147483649)])
def test_two_bit_product_expansion_preserves_wrapping(width, factor):
    """HOTLPX's factor twenty is a modular low product, including negative word inputs."""
    from itertools import count
    from types import SimpleNamespace
    from qbopt.backend import lower
    source, result = mir.Value(900, 0), mir.Value(901, 0)
    op = mir.Op(0, ir.Operation.MULTIPLY, "imul", (result,), (source,), kind=mir.Kind.MUL,
                args=(mir.Held(source, width), mir.Const(factor, width)), results=(mir.Held(result, width),))
    parts = lower._scaled(op, SimpleNamespace(fresh=count(1000).__next__))
    assert parts and not any(one.op is ir.Operation.MULTIPLY for one in parts)
    mask = (1 << (width * 8)) - 1
    for number in (0, 1, 7, mask // 2, mask // 2 + 1, mask - 1, mask):
        values = {source.id: number}
        for part in parts:
            left, right = (values[arg.value] if isinstance(arg, ir.Held) else arg.value for arg in part.sources)
            values[part.dests[0].value] = ((left << right) if part.name == "shl" else left + right) & mask
        assert values[result.id] == (number * factor) & mask


@pytest.mark.parametrize("preserve", [True, False])
def test_two_bit_scale_keeps_observed_multiply_flags(preserve):
    """Shift/add must not replace IMUL when a following consumer observes overflow or carry."""
    from qbopt.backend import lower
    source, result = mir.Value(900, 0), mir.Value(901, 0)
    op = mir.Op(0, ir.Operation.MULTIPLY, "imul", (result,), (source,), kind=mir.Kind.MUL,
                args=(mir.Held(source, 2), mir.Const(20, 2)), results=(mir.Held(result, 2),))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (op,), ()),))
    lowering = lower.Lowering(body, {source.id, result.id}, {}, {}, {})
    parts = lowering.expand(op, preserve_flags=preserve)
    assert any(one.what.op is ir.Operation.MULTIPLY for one in parts) == preserve


def test_lower_places_a_commutative_constant_in_the_immediate_operand() -> None:
    """hotlop needlessly loaded 21 before adding its accumulator."""
    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.backend import lower

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
    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import twoaddr

    result, source = ir.Held(900, 2), ir.Held(901, 2)
    insn = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.BINARY, "add", (result,), (source, ir.Imm(21, 2))),
        defines=(900,),
        uses=(901,),
    )
    fixed = twoaddr._untied(insn, iter(range(1000, 2000)).__next__)
    assert fixed[1].uses == (900,)
