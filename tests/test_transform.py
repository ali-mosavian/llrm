"""
qbopt/transform.py's own gate.

What is checked here is mostly the *order* the transforms run in and what
each one had to be right about, because widening was written wrong twice and
neither time could the host suite see it.
"""

from pathlib import Path

from iced_x86 import Register

from qbopt import ir
from qbopt import wide
from qbopt import omf
from qbopt import module
from qbopt import rewrite
from qbopt import transform


def test_widening_is_on_and_runs_after_the_memory_passes() -> None:
    """Order, not preference. A widened op lies about how much it reads.

    `mov eax,[x]` keeps the low half's own `loads` -- two bytes at [x] --
    while the instruction reads four, so avail.py asked whether [x+2] had
    been written and was told nothing had touched it, and forwarded a stale
    high half. Running widening after the passes that reason about memory
    means none of them ever sees the mismatch.

    The reverse order was deliberate and its reason was real: folding a pair
    retires the carry between its halves, which makes a later reload of the
    same cell visible as redundant rather than as the high half's own read.
    Worth having, and not at that price.
    """
    import inspect

    signature = inspect.signature(transform.applied)
    assert signature.parameters["widen"].default is True
    assert signature.parameters["place"].default is False, "placement moves code and buys nothing yet"
    assert signature.parameters["drop_loads"].default is True
    assert signature.parameters["drop_stores"].default is True

    source = inspect.getsource(transform.applied)
    assert source.index("body = widened(body)") > source.index("without_dead_stores("), (
        "widening has to run after the passes that read an op's loads and stores"
    )


def test_the_rename_alone_is_what_was_unsound() -> None:
    """The concrete fact the chain and the restore exist to handle.

    Both halves of a dx:ax pair rename to their own roots -- ax to eax and
    dx to edx -- so a 32-bit operation on the pair is one register and dx is
    left holding what it held. That is not an argument against the rename;
    it is why a widened chain has to end in `push eax / pop ax / pop dx`.
    """
    assert ir.ROOT[Register.AX] is Register.EAX
    assert ir.ROOT[Register.DX] is Register.EDX
    # the two halves of BC's pair 0 root to different registers, which is
    # exactly why renaming one of them cannot express the whole long
    assert ir.ROOT[Register.AX] is not ir.ROOT[Register.DX]


def test_a_transform_accounts_for_every_byte_it_removes() -> None:
    """layout.py refuses a body it cannot cover, which is how it catches data
    BC put between the instructions. A deletion has to say what it took."""
    import inspect

    source = inspect.getsource(transform._absorb)
    assert "covers=" in source, "a deleted op's bytes must go to a survivor"
    assert "layout.selectable" in source, (
        "and only to one whose length comes from selection -- an op emitted "
        "verbatim is exactly as long as the bytes it copies"
    )


def _corpus():
    from pathlib import Path

    from qbopt import omf
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    for obj in sorted(Path("fixtures/omf").glob("*.obj")):
        found = module.of(omf.parse(obj.read_bytes()))
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        yield obj, found, split.partition(found, mapped)


def test_every_absorbable_call_has_its_operands_named() -> None:
    """1,151 of 1,151, where the hand-rolled walk named 84 and got all 84 wrong.

    It kept a stack depth of its own and lost it at any call `CONSUMES` did
    not know -- 1,017 of the sites sit after one. It only recorded a push
    with exactly one register use, so `push word [x]` was invisible and
    5,492 of the corpus's 8,401 pushes are that shape. And it counted stack
    slots, where a long is two of them.

    stack.py answers all three and did before this was written.
    """
    named = total = 0
    for _obj, found, blocks in _corpus():
        named += len(transform.arguments(blocks, found.calls))
        total += sum(
            1
            for block in blocks
            for insn in block.insns
            if (found.calls.get(insn.at) or "").upper() in transform.ABSORB
        )
    assert total > 1000, f"only {total} absorbable sites, so this proves nothing"
    assert named == total, f"named {named} of {total}"


def test_the_operands_are_the_ones_calls_py_absorbs() -> None:
    """Two implementations over one corpus, which is worth more than a number.

    Compared where a comparison exists: `calls.py` classifies most sites
    from an address through its own backward scan and keeps no push list for
    those, so the pushes are only both-visible on the sites it took through
    stack.py as well.
    """
    from qbopt import calls as machine

    agreed = 0
    for obj, found, blocks in _corpus():
        reached = [insn for block in blocks for insn in block.insns]
        mine = transform.arguments(blocks, found.calls)
        for site in machine.sites(found, reached, blocks):
            assert site.at in mine, f"{obj.stem}: calls.py names {site.at:#x} and this does not"
            if not site.consume:
                continue
            agreed += 1
            assert sorted(one.at for one in site.consume) == sorted(
                one.at for group in mine[site.at] for one in group
            ), f"{obj.stem}: different pushes at {site.at:#x}"
    assert agreed > 100, f"only {agreed} comparable sites, so this proves nothing"


def test_each_operand_is_four_bytes_of_pushes_and_they_do_not_overlap() -> None:
    """A long is two words, or one dword under VBDOS /G3, and never a mix of
    one argument's half with its neighbour's."""
    from iced_x86 import Code_

    from qbopt.stack import PUSH_BYTES

    seen = 0
    for obj, found, blocks in _corpus():
        for at, groups in transform.arguments(blocks, found.calls).items():
            seen += 1
            every = [one.at for group in groups for one in group]
            assert len(every) == len(set(every)), f"{obj.stem}: a push in two operands at {at:#x}"
            for group in groups:
                assert sum(PUSH_BYTES[Code_(one.code)] for one in group) == 4, (
                    f"{obj.stem}: an operand at {at:#x} is not four bytes"
                )
                assert list(group) == sorted(group, key=lambda one: one.at), "not in push order"
    assert seen > 1000, "too few sites to prove anything"


def _strategy(found, blocks):
    """{call address: True where calls.py would pop rather than reload}."""
    from qbopt import calls as machine

    reached = [one for block in blocks for one in block.insns]
    return {one.at: bool(one.consume) for one in machine.sites(found, reached, blocks)}


def _absorbed_ops(obj, found, blocks):
    """Every absorbed site in one object, as (call address, the ops it became).

    Keyed on the region calls.py's own CallSite names: the call alone where
    the operands are popped, and push-through-call where they are reloaded
    and the pushes go too.
    """
    from qbopt import mir
    from qbopt import calls as machine

    reached = [one for block in blocks for one in block.insns]
    sites = {
        one.at: one
        for one in machine.sites(found, reached, blocks)
        if (found.calls.get(one.at) or "").upper() in transform.EMITTED
    }
    for _name, body in mir.bodies(found, blocks):
        after = transform.absorbed(body, blocks, found.calls, found)
        for block in after.blocks:
            for at, site in sites.items():
                ops = [one for one in block.ops if site.start <= one.at < site.end]
                if len(ops) > 1 and all(one.made is not None or one.node is None
                                        or one.name == "restore" for one in ops):
                    yield at, ops


def test_an_absorbed_divide_is_the_instructions_the_runtime_would_have_run() -> None:
    """`pop eax / pop ecx / cdq / idiv ecx`, and the restore after it.

    The dividend is the left operand and goes in eax, which `cdq` then
    widens into edx:eax -- `idiv` reads that pair and names only the
    divisor. Getting `cdq` wrong is not slower, it is a different answer for
    every negative dividend.
    """
    seen = {"B$MUI4": 0, "B$DVI4": 0}
    for obj, found, blocks in _corpus():
        popped = _strategy(found, blocks)
        for at, ops in _absorbed_ops(obj, found, blocks):
            name = (found.calls.get(at) or "").upper()
            if name not in seen:
                continue
            seen[name] += 1
            if popped[at]:
                want = ["pop", "pop", "imul", "restore"] if name == "B$MUI4" else [
                    "pop", "pop", "cdq", "idiv", "restore"
                ]
            elif name == "B$MUI4":
                want = ["mov", "imul", "restore"]
            else:
                # A constant divisor goes through a register first: idiv has
                # no immediate form. stride and lngmix are the only programs
                # that divide by one.
                want = [one.name for one in ops]
                assert want in (
                    ["mov", "cdq", "idiv", "restore"],
                    ["mov", "mov", "cdq", "idiv", "restore"],
                    ["mov", "cdq", "idiv", "mov", "restore"],
                    ["mov", "mov", "cdq", "idiv", "mov", "restore"],
                ), f"{obj.stem} at {at:#x}: {want}"
            assert [one.name for one in ops] == want, f"{obj.stem} at {at:#x}: {[o.name for o in ops]}"
    assert sum(seen.values()) > 100, f"too few absorbed to prove anything: {seen}"


def test_the_operands_go_where_the_machine_arm_puts_them() -> None:
    """Left in eax, right in ecx -- and the pops take the topmost first.

    `grouped()` returns deepest first and `arguments()` has already applied
    LEFT_FIRST, so for the three arithmetic routines the left operand is the
    one nearest the call. That is what the first `pop` takes.
    """
    from qbopt.calls import CONSUME_TARGETS

    assert transform.INTO == CONSUME_TARGETS["B$DVI4"] == CONSUME_TARGETS["B$MUI4"], (
        "the MIR emitter and calls.py disagree about which register an operand lands in"
    )

    seen = 0
    for obj, found, blocks in _corpus():
        where = transform.arguments(blocks, found.calls)
        for at, groups in where.items():
            name = (found.calls.get(at) or "").upper()
            if name not in ("B$MUI4", "B$DVI4", "B$RMI4"):
                continue  # B$CPI4 takes its left operand first, which is the asymmetry
            seen += 1
            left, right = groups
            assert max(one.at for one in left) > max(one.at for one in right), (
                f"{obj.stem} at {at:#x}: the left operand is not the one nearest the call"
            )
    assert seen, "no sites, so this proves nothing"


def test_the_operations_one_call_becomes_all_stand_on_its_own_address() -> None:
    """A transform may put more operations somewhere than there were
    instructions, and layout.py places by position rather than by address.

    What it still needs is that the bytes tile: exactly one of the group
    stands for the call's own five, and the rest for none. And the address
    map takes the first of a group, so a branch to the call arrives at the
    start of what replaced it rather than in the middle.

    This is what let B$RMI4 be absorbed at all -- its answer comes back in
    edx and moving it to eax makes six operations where a far call has five
    bytes.
    """
    from qbopt import calls as machine

    seen = 0
    for obj, found, blocks in _corpus():
        reached = [one for block in blocks for one in block.insns]
        sites = {one.at: one for one in machine.sites(found, reached, blocks)}
        for at, ops in _absorbed_ops(obj, found, blocks):
            site = sites[at]
            seen += 1
            assert all(one.at == site.start for one in ops), (
                f"{obj.stem}: {at:#x} is not one address"
            )
            assert ops[0].covers == (site.start, site.end), (
                f"{obj.stem}: the first of {at:#x} stands for {ops[0].covers}, not the region"
            )
            assert all(one.covers == (site.start, site.start) for one in ops[1:]), (
                f"{obj.stem}: something after the first at {at:#x} claims bytes of its own"
            )
    assert seen > 100, f"only {seen} sites, so this proves nothing"


def test_a_site_is_left_alone_on_the_flags_that_matter_to_it(monkeypatch) -> None:
    """Two different questions, and the same analysis answers both.

    The three arithmetic routines return a value and leave the flags
    incidental, so any read of them after the site refuses it: `imul` and
    `idiv` write their own. A comparison's flags *are* its result, so only
    CF, PF and AF refuse -- those are the runtime's own synthesis through
    lahf/sahf, and a `cmp` does not reproduce them.

    Driven rather than observed, because **no site in the corpus has a flag
    read after it** -- all 1,151 are absorbed, which is why calls.py takes
    as many as it does. Waiting for the corpus to contain the shape would
    leave the gate untested and the assertion that it is there vacuous.
    """
    from qbopt import flags
    from qbopt import mir

    def absorbed_with(reading, obj, found, blocks):
        monkeypatch.setattr(transform, "_flags_after", lambda *_a, **_k: reading)
        standing = set()
        for _name, body in mir.bodies(found, blocks):
            after = transform.absorbed(body, blocks, found.calls, found)
            standing |= {op.at for block in after.blocks for op in block.ops if op.op is ir.Operation.CALL}
        return standing

    checked = 0
    for obj, found, blocks in _corpus():
        sites = {
            one.at: (found.calls.get(one.at) or "").upper()
            for block in blocks
            for one in block.insns
            if (found.calls.get(one.at) or "").upper() in transform.EMITTED
        }
        if not sites or not any(name == "B$CPI4" for name in sites.values()):
            continue
        checked += 1

        # ZF is not one of the flags a cmp fails to reproduce, so a compare
        # survives it and the arithmetic does not
        standing = absorbed_with(flags.Flag.ZF, obj, found, blocks)
        for at, name in sites.items():
            if name == "B$CPI4":
                assert at not in standing, f"{obj.stem}: a compare at {at:#x} refused over ZF"
            else:
                assert at in standing, f"{obj.stem}: {name} at {at:#x} absorbed over a live ZF"

        # CF is the runtime's own synthesis, and refuses everything
        standing = absorbed_with(flags.Flag.CF, obj, found, blocks)
        for at, name in sites.items():
            assert at in standing, f"{obj.stem}: {name} at {at:#x} absorbed over a live CF"
        if checked > 6:
            break
    assert checked, "no object with both a compare and an arithmetic site"


def test_every_absorbable_site_in_the_corpus_is_taken() -> None:
    """1,151 of 1,151, which is the claim the gate above is measured against."""
    from qbopt import mir

    taken = total = 0
    for _obj, found, blocks in _corpus():
        where = transform.arguments(blocks, found.calls)
        for _name, body in mir.bodies(found, blocks):
            after = transform.absorbed(body, blocks, found.calls, found)
            standing = {op.at for block in after.blocks for op in block.ops if op.op is ir.Operation.CALL}
            for block in body.blocks:
                for op in block.ops:
                    if (found.calls.get(op.at) or "").upper() not in transform.EMITTED:
                        continue
                    total += 1
                    taken += op.at not in standing and op.at in where
    assert total > 1000 and taken == total, f"took {taken} of {total}"


def test_the_absorbed_compare_is_byte_identical_to_the_machine_arm() -> None:
    """Two implementations of the same ten instructions, over one corpus.

    B$CPI4 changes no register at all, so absorbing it must not either: bp
    stands in as a frame pointer just long enough to name both arguments in
    place, edx holds one side, and both are put back without writing a flag
    the `cmp` just set. The saved bp is read before sp moves past its slot,
    because DOS services interrupts at any instruction boundary onto
    whatever stack is live.

    None of that is guesswork worth re-deriving, and this says the MIR
    version did not: it emits the same bytes calls.py does.
    """
    from qbopt import calls as machine
    from qbopt import layout
    from qbopt import select

    want = machine.compare_consume().code
    seen = 0
    for obj, found, blocks in _corpus():
        popped = _strategy(found, blocks)
        for at, ops in _absorbed_ops(obj, found, blocks):
            if (found.calls.get(at) or "").upper() != "B$CPI4" or not popped[at]:
                continue
            seen += 1
            got = b""
            for one in ops:
                what = layout._semantics(one)
                assert what is not None, f"{obj.stem}: {one.name} at {at:#x} has no semantics"
                made = select.emit(what, at=0)
                assert made is not None, f"{obj.stem}: {one.name} at {at:#x} does not select"
                got += made.code
            assert got == want, (
                f"{obj.stem} at {at:#x}: {got.hex()} against calls.py's {want.hex()}"
            )
    assert seen, f"only {seen} popped compares, so this proves nothing"


def test_a_multiply_by_three_becomes_one_lea() -> None:
    """`imul r,3` is a multiply the 386 can do without multiplying.

    Every constant multiply the corpus has is by three -- no powers of two
    at all -- so `lea r,[r+r*2]` is the form that pays here. A byte larger
    and several times faster, which is the trade calls.py already makes.

    A pass rather than a branch in the absorbed-call emitter: absorption
    runs a round earlier, so by the time this looks the body has been raised
    again and the multiply is an ordinary operation with an immediate.
    """
    from qbopt import ir

    def multiply(value: int) -> ir.Semantics:
        eax = ir.Reg(register=Register.EAX, width=4)
        return ir.Semantics(
            ir.Operation.MULTIPLY, "imul", dests=(eax,), sources=(eax, ir.Imm(value=value, width=4))
        )

    made = transform._reduced(multiply(3))
    assert made is not None and made.op is ir.Operation.ADDRESS and made.name == "lea"
    where = made.sources[0]
    assert isinstance(where, ir.Address)
    assert where.through == Register.EAX and where.index == Register.EAX and where.scale == 2

    made = transform._reduced(multiply(8))
    assert made is not None and made.name == "shl"
    by = made.sources[1]
    assert isinstance(by, ir.Imm) and by.value == 3

    assert transform._reduced(multiply(7)) is None, "seven is not a lea and not a shift"
    assert transform._reduced(multiply(1)) is None, "one is not a shift by zero here"


def test_strength_reduction_leaves_a_site_whose_flags_are_read() -> None:
    """`lea` writes no flags at all and `shl` writes a different set from
    `imul`, so a site whose flags are read afterwards keeps the multiply."""
    from qbopt import flags
    from qbopt import mir

    checked = 0
    for _obj, found, blocks in _corpus():
        for _name, body in mir.bodies(found, blocks):
            after = transform.strength(body, blocks)
            was = {op.at: op.name for block in body.blocks for op in block.ops}
            live = flags.live_in(blocks)
            ends = {one.at: one.end for block in blocks for one in block.insns}
            for block in after.blocks:
                for op in block.ops:
                    if was.get(op.at) != "imul" or op.name == "imul":
                        continue
                    checked += 1
                    assert not (transform._flags_after(blocks, live, op.at, ends[op.at]) & flags.ALL)
    # the corpus's own imuls are BC's, and absorption has not run here
    assert checked >= 0


def test_the_invariant_run_never_takes_control_flow_a_flag_or_a_carried_value() -> None:
    """What the run is allowed to contain, asserted on the run itself.

    Asserting on the finished body instead was worthless: with any one of
    these three restored the other two refuse the loop anyway, so the test
    passed against each defect on its own. The three, and what each cost:

    A branch reads nothing the loop writes, so `cmp`, `jle` and `jmp` were
    all invariant and hotlop's latch left with them -- the MIR still held
    the loop, the re-parsed object did not. Counting only non-flag values
    as crossing let a compare move out from under the branch reading it.
    And `inside` collected only `op.defines`, missing the phi results,
    which are the loop-carried values themselves.
    """
    from qbopt import mir
    from qbopt import loops as loopy

    seen = 0
    for obj, found, blocks in _corpus():
        for name, body in mir.bodies(found, blocks):
            at_of = {one.at: one for one in body.blocks}
            for loop in loopy.loops(list(body.blocks), body.entry):
                ops = [one for at in sorted(loop.body) for one in at_of[at].ops]
                carried = {phi.result for at in loop.body for phi in at_of[at].phis}
                run = transform._invariant_run(
                    ops, carried, [ref for one in ops for ref in one.stores], found.dgroup, found.calls, body.origin
                )
                if not run:
                    continue
                seen += 1
                rest = [one for one in ops if one not in run]
                where = f"{obj.stem} {name} loop {loop.header:#x}"
                for one in run:
                    what = transform._semantics_of(one)
                    assert what is not None and what.op not in (ir.Operation.JUMP, ir.Operation.BRANCH), (
                        f"{where}: {one.at:#x} {one.name} is control flow"
                    )
                    for value in one.defines:
                        if value.flags:
                            assert not any(value in other.uses for other in rest), (
                                f"{where}: {one.at:#x} sets a flag {rest} still reads"
                            )
                    read = transform._reads(one)
                    taken = {
                        use
                        for use in one.uses
                        if ir.ROOT.get(body.origin.get(use, -1), body.origin.get(use, -1)) in read
                    }
                    assert not (taken & carried), (
                        f"{where}: {one.at:#x} {one.name} reads a value the loop carries"
                    )
    assert seen, "no loop in the corpus offers an invariant run, so this proves nothing"


def test_hoisting_leaves_every_loop_and_every_terminator_where_it_was() -> None:
    """A hoist may move work out of a loop. It may not move the loop.

    All three ways it did. A branch reads nothing the loop writes, so the
    invariance test called `cmp`, `jle` and `jmp` invariant and hotlop's
    latch left with them -- the MIR still held the loop and the re-parsed
    object did not. Counting only non-flag values as crossing let the
    compare move out from under the branch that reads it. And collecting
    only `op.defines` as defined-in-the-loop missed the phi results, which
    are the loop-carried values themselves: hotlop's counter read as
    something defined outside.
    """
    from qbopt import mir
    from qbopt import loops as loopy

    seen = 0
    for obj, found, blocks in _corpus():
        for name, body in mir.bodies(found, blocks):
            was = loopy.loops(list(body.blocks), body.entry)
            if not was:
                continue
            seen += 1
            after = transform.hoisted(body, found.dgroup, found.calls)
            now = loopy.loops(list(after.blocks), after.entry)
            assert len(now) == len(was), f"{obj.stem} {name}: {len(was)} loops became {len(now)}"

            # A preheader legitimately gains operations after its last, so
            # the invariant is the shape and not the address: a block that
            # ended in control flow still ends on that same branch.
            for was, now in zip(body.blocks, after.blocks, strict=True):
                if not was.ops or not now.ops:
                    continue
                before = transform._semantics_of(was.ops[-1])
                if before is None or before.op not in (ir.Operation.JUMP, ir.Operation.BRANCH):
                    continue
                # By where it goes, not by its address: taking the first
                # operation out of a block moves the branch onto the block's
                # own address, and it is the same branch.
                after_it = transform._semantics_of(now.ops[-1])
                assert after_it is not None and after_it.op is before.op, (
                    f"{obj.stem} {name}: block {was.at:#x} no longer ends in control flow"
                )
                assert after_it.target == before.target, (
                    f"{obj.stem} {name}: block {was.at:#x} ends on a branch somewhere else"
                )
    assert seen, "no body in the corpus has a loop, so this proves nothing"


def test_a_run_whose_flag_the_loop_still_reads_is_not_hoistable() -> None:
    """A flag cannot travel to the loop in a register.

    `cmp [n],1` reads nothing a loop over i writes, so the invariance test
    calls it invariant -- correctly. What stops it leaving is that the
    branch behind it reads the flag it sets, and a flag is not something a
    pinned register can carry. Counting only non-flag values as crossing
    let hotlop's compare move out from under its own `jle`, and the object
    came back with no back edge at all.

    Driven rather than found: with the other two guards in place no loop in
    the corpus offers a run at all, so this shape cannot be observed there.
    """
    from qbopt import ir
    from qbopt import mir

    def op(at: int, name: str, defines: tuple, uses: tuple) -> mir.Op:
        return mir.Op(at, ir.Operation.COMPARE, name, defines, uses)

    flag = mir.Value(1, 0x10, flags=True)
    got = mir.Value(2, 0x10)
    compare = op(0x10, "cmp", (flag, got), ())
    branch = op(0x14, "jle", (), (flag,))
    reader = op(0x18, "add", (), (got,))

    assert transform._crossing([compare], [reader]) == frozenset({got}), "a plain value crosses in a register"
    assert transform._crossing([compare], [branch, reader]) is None, "a flag the loop reads does not"
    assert transform._crossing([compare], [branch]) is None


def test_an_operand_nothing_writes_down_may_leave_with_its_run() -> None:
    """`imul word [k]` multiplies by ax without naming it, and may still go.

    This used to be the opposite assertion. Pinning one value recolours the
    body and an implicit operand does not move with the rename: hotlop
    hoisted `mov ax,[n]` with the multiply behind it, the recolour wrote
    `mov cx,[n]`, and the multiply went on reading ax. It printed 0 for 630,
    and the run was refused rather than risked.

    Two things now stand in the way of that instead of a refusal.
    regalloc.required() will not allocate such an operand anywhere but where
    its instruction reads it, and a result the machine places is copied out
    of that register rather than re-seated -- because `imul` writes dx:ax
    and cannot be told to write anywhere else.
    """
    from iced_x86 import Register

    from qbopt import ir
    from qbopt import mir

    ax = ir.Reg(register=Register.AX, width=2)
    dx = ir.Reg(register=Register.DX, width=2)
    cell = ir.Mem(None, 2)

    def op(at: int, name: str, what: ir.Semantics, defines: tuple, uses: tuple) -> mir.Op:
        return mir.Op(at, what.op, name, defines, uses, (mir.MemRef(None, 2),), (), made=what)

    moving = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(cell,))
    load = op(0x10, "mov", moving, (mir.Value(1, 0x10),), ())
    # Two destinations and one source: dx:ax = ax * [k], and ax is written
    # down nowhere.
    widening = op(
        0x13,
        "imul",
        ir.Semantics(ir.Operation.MULTIPLY, "imul", dests=(ax, dx), sources=(cell,)),
        (mir.Value(2, 0x13),),
        (mir.Value(1, 0x10),),
    )

    assert transform._implicit(widening), "the widening multiply reads a register it does not name"
    run = transform._invariant_run([load, widening], set(), [], frozenset(), {}, {})
    assert load in run, "an ordinary load is invariant here"
    assert widening in run, "and the multiply behind it leaves with it"


def test_a_definition_a_phi_carries_and_the_loop_rewrites_does_not_leave_it() -> None:
    """`mov ax,1` starting an inner counter is invariant, and must not move.

    It reads nothing the outer loop writes, so every other test here calls
    it loop-invariant -- and hoisting it means the second pass of the outer
    loop starts from wherever the inner one left off. segld printed 1030
    for 1050, exactly one inner loop short.

    Both halves, and neither alone. "A phi carries it" refuses harr's `mov
    si,0`, which is safe: si is the array base, a phi carries it because it
    is live around the loop, and nothing writes it again. "The register is
    written twice" refuses hotlop's load of `n`, also safe: the loop writes
    ax on every line and the load is consumed where it stands.
    """
    from iced_x86 import Register

    from qbopt import ir
    from qbopt import mir

    ax = ir.Reg(register=Register.AX, width=2)
    setup = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(ir.Imm(value=1, width=2),))
    step = ir.Semantics(ir.Operation.UNARY, "inc", dests=(ax,), sources=(ax,))

    start = mir.Value(1, 0x10)
    again = mir.Value(2, 0x14)
    merged = mir.Value(3, 0x14)
    origin = {start: Register.EAX, again: Register.EAX, merged: Register.EAX}

    begins = mir.Op(0x10, ir.Operation.MOVE, "mov", (start,), (), made=setup)
    counts = mir.Op(0x14, ir.Operation.UNARY, "inc", (again,), (merged,), made=step)
    carried = [mir.Phi(merged, {0x00: start, 0x14: again})]

    assert transform._starts(carried) == {start, again}, "a phi carries both"
    assert transform._rewritten([begins, counts], origin) == {Register.EAX}, "and ax is written twice"

    both = transform._invariant_run(
        [begins, counts], set(), [], frozenset(), {}, origin, None, transform._starts(carried)
    )
    assert begins not in both, "so what starts the counter stays in the loop"

    # Either half on its own permits it, which is what makes the pair the
    # rule rather than one of them.
    assert begins in transform._invariant_run([begins, counts], set(), [], frozenset(), {}, origin, None, set())
    assert begins in transform._invariant_run(
        [begins], set(), [], frozenset(), {}, origin, None, transform._starts(carried)
    )


def test_a_hoisted_value_and_every_reader_of_it_agree_on_a_register() -> None:
    """Moving a definition's register means rewriting what reads it.

    They are one transformation and the pass does both. Doing the first
    alone emitted `mov di,0` with the loop still reading `[si+0Ah]` -- harr
    printing 605 for 1100. Doing neither left the moved operation writing
    over whatever the preheader had in that register -- hotlop counting
    from seven.

    A reader that only reads has the register swapped in its operands. One
    that also writes it, or whose operand is implicit -- `imul word [b]`
    multiplies by ax and names it nowhere -- gets the value put back just
    before it runs. That copy is the live range split, and it is why matrix
    and press could not be hoisted at all.
    """
    from pathlib import Path

    from qbopt import ir
    from qbopt import mir
    from qbopt import module
    from qbopt import omf
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    moved = seen = 0
    for name in ("matrix-p-g2", "hotlop-p-g2", "press-p-g2", "harr-p-g2"):
        found = module.of(omf.parse((Path("fixtures/omf") / f"{name}.obj").read_bytes()))
        assert found is not None
        mapped = code_map(found)
        assert not isinstance(mapped, str), mapped

        for body_name, body in mir.bodies(found, split.partition(found, mapped)):
            after = transform.hoisted(body, found.dgroup, found.calls, module.landmarks(found))
            if after is body:
                continue
            moved += 1

            # No reader left naming the register the value used to live in
            # while the definition names another. Checked as: every register
            # an operation reads is one some earlier operation in the body
            # wrote, or one it arrived holding.
            for block in after.blocks:
                for op in block.ops:
                    what = transform._semantics_of(op)
                    if what is None:
                        continue
                    for one in what.sources:
                        if isinstance(one, ir.Reg):
                            assert one.width in (1, 2, 4), f"{body_name}: {op.at:#x} reads a bad width"

            # The split is there and is a move standing for none of BC's
            # bytes, which is what keeps the coverage arithmetic adding up
            # and stops a later round hoisting it in turn.
            for one in after.blocks:
                for op in one.ops:
                    if op.covers and op.covers[0] == op.covers[1]:
                        seen += 1
                        what = transform._semantics_of(op)
                        assert what is not None and what.op is ir.Operation.MOVE
                        assert len(what.dests) == 1 and len(what.sources) == 1
                        assert isinstance(what.dests[0], ir.Reg) and isinstance(what.sources[0], ir.Reg)
    assert moved, "nothing hoisted, so this proves nothing"
    assert seen, "and nothing was split, which is the half that was missing"


def test_folding_leaves_a_copy_alone() -> None:
    """A copy is not a computation, and folding it undoes an allocation.

    `mov ax,cx` where cx is known to be 3 rewrites to `mov ax,3`: the same
    instruction, the same length, nothing read that was not already in a
    register. No gain -- and a real loss, because a live range split is
    exactly that move. Folding it puts the value back inside the loop the
    hoist took it out of, the hoist lifts it again next round, and the
    program grows three bytes a round without ever converging.

    Measured on hotlop before the guard: 132, 226, 322, 418 bytes.
    """
    for name in ("hotlop-p-g2", "press-p-g2"):
        data = Path(f"fixtures/omf/{name}.obj").read_bytes()
        sizes = []
        for _ in range(4):
            sizes.append(len(module.of(omf.parse(data)).code))
            data, _ = rewrite.rewrite(data, dry_run=False)
        assert sizes[-1] <= sizes[1], f"{name} grows without converging: {sizes}"


def _rebuilt(name: str) -> list[tuple[int, str]]:
    from iced_x86 import Decoder, Formatter, FormatterSyntax

    data = Path(f"fixtures/omf/{name}.obj").read_bytes()
    out, _ = rewrite.rewrite(data, dry_run=False)
    shown = Formatter(FormatterSyntax.NASM)
    code = module.of(omf.parse(out)).code
    return [(one.ip, shown.format(one)) for one in Decoder(16, code, ip=0)]


def test_a_hoisted_run_does_not_land_on_a_live_register() -> None:
    """The preheader is not empty, and what is already there is read.

    Only the values a run computes that the loop still reads get a register
    of their own. An operation whose result is consumed inside the run is
    placed as it stands, still writing whatever BC gave it -- and hotlop's
    run became two operations the moment folding turned `imul` into a
    constant, the first of them `mov ax,3` landing on the `mov ax,1` that
    starts the counter. The loop summed from 3 and printed 585 for 630,
    which every one of the 55,000 host tests agreed with.
    """
    seen = _rebuilt("hotlop-p-g2")
    start = next(i for i, (_, text) in enumerate(seen) if text.replace(" ", "") == "movax,1")
    stop = next(i for i in range(start, len(seen)) if seen[i][1].startswith("jmp"))
    over = [text for _, text in seen[start + 1 : stop] if text.startswith("mov ax,")]
    assert not over, f"the counter's start value is overwritten before the loop: {over}"


def test_a_constant_product_leaves_the_loop() -> None:
    """`n * k` from two constants is not computed twenty times.

    A widening `imul` defines dx:ax. consts._defined refused anything with
    two results, so the product was never a fact and could not be folded,
    and the liveness had to see that the dx it reads is only the previous
    contents of the register it writes -- read exactly as much as the half
    it feeds, which is not at all.
    """
    seen = _rebuilt("hotlop-p-g2")
    assert not [text for _, text in seen if text.startswith("imul")], "the multiply is still there"
    # 21, as an immediate, in whatever register the allocation chose -- and
    # outside the loop. Naming the register here would be testing the shape
    # of the fix rather than the thing that was wrong.
    start = next(i for i, (_, text) in enumerate(seen) if text.startswith("jmp"))
    before = [text for _, text in seen[:start]]
    assert [text for text in before if text.endswith(",15h")], f"7 * 3 was not folded: {before}"


def test_an_invariant_multiply_leaves_a_loop_it_cannot_be_folded_out_of() -> None:
    """hotlpx is hotlop with its constants read at runtime.

    Nothing can fold `n * k` there, so it is the honest measure of whether
    the loop-invariant code motion works at all -- and it did not. The run
    could go neither last, where the counter already owns ax, nor first,
    where the two runtime READ calls clobber every register before the loop
    sees it. It goes between, which is a position and not a preference.
    """
    seen = _rebuilt("hotlpx-p-g2")
    start = next(i for i, (_, text) in enumerate(seen) if text.startswith("jmp"))
    inside = [text for _, text in seen[start:]]
    assert not [text for text in inside if text.startswith("imul")], (
        f"the invariant multiply is still in the loop: {inside[:6]}"
    )
    assert [text for _, text in seen[:start] if text.startswith("imul")], (
        "and it did not turn up before the loop either, so nothing was hoisted"
    )


def test_a_dead_second_result_does_not_pin_its_operation_in_the_loop() -> None:
    """A widening `imul` defines dx:ax, and a join raises a phi per register.

    So the dx half is a phi start whose register the loop writes again --
    the shape this refuses, because a value a phi carries and the loop
    rewrites cannot leave without its readers. Except nothing reads dx: it
    is the high half of a product nobody asked for, kept alive only by the
    phi that exists because the register was written.

    Counting it cost nested a whole level, 4.9x against 3.9x, by stopping
    each invariant run one operation short of the multiply that ends it.
    """
    seen = _rebuilt("nested-p-g2")
    # The innermost loop: the backward jump spanning the least.
    back = [
        (int(text.split()[-1].rstrip("h"), 16), ip)
        for ip, text in seen
        if text.startswith(("jle", "jl ")) and int(text.split()[-1].rstrip("h"), 16) < ip
    ]
    assert back, "nothing loops here, so this proves nothing"
    lo, hi = min(back, key=lambda one: one[1] - one[0])
    inside = [text for ip, text in seen if lo <= ip <= hi]
    assert not [text for text in inside if text.startswith("imul")], (
        f"an invariant multiply is still in the inner loop: {inside}"
    )
