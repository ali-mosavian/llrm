"""
qbopt/transform.py's own gate.

What is checked here is mostly the *order* the transforms run in and what
each one had to be right about, because widening was written wrong twice and
neither time could the host suite see it.
"""

import pytest
from pathlib import Path

from iced_x86 import Register

from qbopt import ir
from qbopt import lir
import corpus
from qbopt import mir
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
    assert signature.parameters["drop_loads"].default is True
    assert signature.parameters["drop_stores"].default is True

    # Widening is not a pass any more -- it recognises an idiom and writes
    # machine form -- and wholeseg runs it after every pass, which is the
    # order this was protecting.
    assert "widen" not in transform.PASSES
    assert "drop_stores" in transform.PASSES


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
                phis = [phi for at in loop.body for phi in at_of[at].phis]
                run = transform._invariant_run(
                    ops, carried, [ref for one in ops for ref in one.stores], found.dgroup, found.calls, phis
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
                    # Really read, not merely preserved: `merges` is the
                    # raise's account of which uses are only the previous
                    # contents of what the operation writes.
                    taken = {use for use in one.uses if use not in one.merges}
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
        return mir.Op(at, ir.Operation.COMPARE, name, defines, uses, kind=mir.Kind.SUB)

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
        return mir.Op(
            at, what.op, name, defines, uses, (mir.MemRef(None, 2),), (),
            kind=mir._kind_of(what, (), ()), made=what,
        )

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

    run = transform._invariant_run([load, widening], set(), [], frozenset(), {}, [])
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

    begins = mir.Op(0x10, ir.Operation.MOVE, "mov", (start,), (), kind=mir.Kind.COPY, made=setup)
    counts = mir.Op(0x14, ir.Operation.UNARY, "inc", (again,), (merged,), kind=mir.Kind.ADD, made=step)
    carried = [mir.Phi(merged, {0x00: start, 0x14: again})]

    assert transform._starts(carried) == {start, again}, "a phi carries both"
    # By value, not by register: in SSA a variable written twice in a loop
    # is a phi with an incoming defined inside it.
    assert start in transform._rewritten([begins, counts], carried), "and the counter is written twice"

    both = transform._invariant_run(
        [begins, counts], set(), [], frozenset(), {}, carried, None, transform._starts(carried)
    )
    assert begins not in both, "so what starts the counter stays in the loop"

    # "A phi carries it" on its own permits it, which is what makes the
    # pair the rule rather than one of them.
    # A copy is refused outright now, so `begins` never enters a run at
    # all -- see the note in _invariant_run. What is still checked is that
    # the phi rule refuses it for its own reason.
    assert begins not in transform._invariant_run(
        [begins, counts], set(), [], frozenset(), {}, carried, None, set()
    )
    # The other half no longer does. Asked of values rather than of
    # registers, "the loop writes it again" is "a phi joins it with
    # something defined inside", and for a run of one operation that is
    # already true -- so this case is refused where counting definitions
    # per register allowed it. Requiring two incomings from inside
    # restored it and miscompiled hotlop on nine of twelve
    # configurations, so the refusal stands.


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
    # Whatever starts the counter: `mov ax,1` while folding left it alone,
    # and hotlop now folds the whole product so the last thing before the
    # jump is the constant itself. What must hold either way is that
    # nothing writes that register again between the initialiser and the
    # loop.
    stop = next(i for i, (_, text) in enumerate(seen) if text.startswith("jmp"))
    setters = [i for i in range(stop) if seen[i][1].startswith("mov ax,")]
    assert setters, "nothing starts the counter at all"
    over = [seen[i][1] for i in setters[1:] if i > setters[0]]
    assert not over, f"the counter's start value is overwritten before the loop: {over}"


@pytest.mark.xfail(
    reason="a copy no longer leaves a loop at all. The guard was \"every source is "
    "a register\", so a constant load was real work and could go -- and with the "
    "hoist no longer allocating, letting one go hoists the counter's own "
    "initialiser and hotlop printed 0 for 630 on nine of twelve configurations. "
    "Restored when regalloc splits a live range on LIR.",
    strict=True,
)
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


@pytest.mark.xfail(
    reason="the hoist no longer allocates: a result that crosses the loop edge needs "
    "a register the preheader can spare, and only regalloc can arrange that. It used "
    "to pick one out of regalloc.AVAILABLE and rewrite every reader, which is where "
    "every hoist bug came from. Restored when regalloc splits a live range on LIR.",
    strict=True,
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


def test_nothing_reads_a_register_nothing_wrote() -> None:
    """_writes_to returns an operation it cannot rewrite, and says nothing.

    Its readers have been pointed at the new register by then, so the loop
    reads one nothing ever wrote: lngmix emitted `mov [bp-14h],di` with di
    written nowhere in the program. A widening imul is the same shape and
    is handled by asking lir whether the register is fixed; an operation
    whose semantics name no destination is neither fixed nor re-seatable,
    and fell through the gap.

    Asserted as the thing that is wrong rather than as the guard: a source
    register that no earlier instruction wrote.
    """
    from iced_x86 import Decoder, Formatter, FormatterSyntax, OpKind, RegisterExt

    # These two, because where BC's header stops decoding as junk is a
    # per-fixture fact and here it is known: the first real instruction
    # is at 0x30.
    for name in ("lngmix-p-g2", "lngmxx-p-g2"):
        data = Path(f"fixtures/omf/{name}.obj").read_bytes()
        out, _ = rewrite.rewrite(data, dry_run=False)
        shown = Formatter(FormatterSyntax.NASM)
        code = module.of(omf.parse(out)).code
        # bp and sp arrive set up; BC's header bytes decode as junk, so the
        # scan starts where the first real instruction does.
        # bp and sp are the frame and are never in question; the segment
        # registers are set up before any of this runs.
        given = {
            RegisterExt.full_register(one)
            for one in (Register.BP, Register.SP, Register.DS, Register.ES, Register.SS, Register.CS)
        }
        written = set(given)
        for one in Decoder(16, code, ip=0):
            if one.ip < 0x30:
                continue
            def root(where):
                return RegisterExt.full_register(where) if where != Register.NONE else Register.NONE

            reads = {root(one.memory_base), root(one.memory_index)}
            # Operand 0 of a two-operand instruction is written; every other
            # register operand is read.
            for i in range(one.op_count):
                if one.op_kind(i) == OpKind.REGISTER and i:
                    reads.add(root(one.op_register(i)))
            for where in reads - {Register.NONE}:
                assert where in written, (
                    f"{name} at {one.ip:#06x}: {shown.format(one)} reads a register nothing wrote"
                )
            for i in range(one.op_count):
                if one.op_kind(i) == OpKind.REGISTER:
                    written.add(root(one.op_register(i)))


@pytest.mark.xfail(
    reason="the hoist no longer allocates: a result that crosses the loop edge needs "
    "a register the preheader can spare, and only regalloc can arrange that. It used "
    "to pick one out of regalloc.AVAILABLE and rewrite every reader, which is where "
    "every hoist bug came from. Restored when regalloc splits a live range on LIR.",
    strict=True,
)
def test_a_long_divide_leaves_a_loop_that_never_changes_its_operands() -> None:
    """lngmix divides a constant by a constant, ten times.

    Three things refused it, and fixing any one alone did nothing: a push
    aliased every named variable, the "computes nothing" guard caught `cdq`
    and `idiv` because their sources are all registers, and the restore
    idiom read as reading nothing. 8.8x against a 210 target.
    """
    seen = _rebuilt("lngmix-p-g2")
    start = next(i for i, (_, text) in enumerate(seen) if text.startswith("jmp"))
    inside = [text for _, text in seen[start:]]
    assert len([text for text in inside if text.startswith("idiv")]) < 2, (
        f"both divides are still in the loop: {inside[:10]}"
    )


def test_dead_code_goes_and_the_bytes_are_still_accounted_for() -> None:
    """A move nothing reads, removed, without losing what it stood for.

    layout.py refuses a body it cannot account for every byte of, so a
    deletion hands its bytes to the operation before it.
    """
    into = ir.Reg(register=Register.BX, width=2)
    from_ax = ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(ir.Reg(register=Register.AX, width=2),))
    imm = ir.Semantics(ir.Operation.MOVE, "mov", dests=(into,), sources=(ir.Imm(value=7, width=2),))
    live_one = mir.Op(
        0x10, ir.Operation.MOVE, "mov", (mir.Value(1, 0x10),), (),
        kind=mir.Kind.COPY, made=imm, covers=(0x10, 0x13),
    )
    doomed = mir.Op(
        0x13, ir.Operation.MOVE, "mov", (mir.Value(2, 0x13),), (),
        kind=mir.Kind.COPY, made=from_ax, covers=(0x13, 0x15),
    )
    body = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (live_one, doomed), ()),), {})
    assert transform._removable(doomed, set()), "nothing reads it"
    assert not transform._removable(live_one, {mir.Value(1, 0x10)}), "and this is read"


def test_dead_code_leaves_a_body_it_cannot_read_alone() -> None:
    """An opaque instruction reads registers no semantics mention.

    byref2 printed 0 for 16 when its argument setup was deleted on the
    strength of a use list that could not have been complete.
    """
    what = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ir.Reg(register=Register.BX, width=2),), sources=())
    # Something before it, so the deletion has a survivor to give its bytes
    # to -- without one _absorb refuses and the guard is never reached.
    first = mir.Op(0x10, ir.Operation.MOVE, "mov", (mir.Value(1, 0x10),), (), kind=mir.Kind.COPY, made=what, covers=(0x10, 0x12))
    doomed = mir.Op(0x12, ir.Operation.MOVE, "mov", (mir.Value(2, 0x12),), (), kind=mir.Kind.COPY, made=what, covers=(0x12, 0x14))
    plain = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (first, doomed), ()),), {})
    assert transform.dead(plain) is not plain, "a dead move goes when the body is readable"

    opaque = mir.Op(0x14, ir.Operation.BARRIER, "?", (), (), covers=(0x14, 0x16))
    body = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (first, doomed, opaque), ()),), {})
    assert transform.dead(body) is body, "and stays when the body holds a barrier"


@pytest.mark.xfail(
    reason="press accumulates four invariant products and each crosses the loop "
    "edge wanting a register of its own, which the hoist no longer allocates. "
    "Restored when regalloc splits a live range on LIR.",
    strict=True,
)
def test_the_invariant_sum_leaves_press_with_one_instruction_in_its_loop() -> None:
    """press adds four invariant products into a running total, ten times.

    All of it is constant, so the loop should hold the accumulate and the
    counter and nothing else. What kept a second instruction there was a
    move whose result is overwritten two bytes later, alive on the strength
    of a register half nothing reads.
    """
    seen = _rebuilt("press-p-g2")
    back = [
        (int(text.split()[-1].rstrip("h"), 16), ip)
        for ip, text in seen
        if text.startswith(("jle", "jl ")) and int(text.split()[-1].rstrip("h"), 16) < ip
    ]
    assert back, "nothing loops here, so this proves nothing"
    lo, hi = min(back, key=lambda one: one[1] - one[0])
    inside = [text for ip, text in seen if lo <= ip <= hi]
    assert not [text for text in inside if text.replace(" ", "").startswith("movbx")], (
        f"a dead move is still in the loop: {inside}"
    )


def test_a_branch_on_two_numbers_is_decided_where_it_stands() -> None:
    """bools is three comparisons over four constants and nothing else.

    BC materialises every one into a register through a branch and a dec,
    then compares that register against zero to branch again. All of it is
    decidable: 165 bytes to 129, and 1.6x to 1.35x.
    """
    seen = _rebuilt("bools-p-g2")
    left = [text for _, text in seen if text.startswith(("jle", "jg", "jl ", "jge", "je ", "jne"))]
    assert not left, f"a branch on two constants is still there: {left}"


def test_deciding_a_branch_leaves_every_byte_accounted_for() -> None:
    """Resolving the flow is not the same as pruning it.

    A block nothing can reach any more still occupies bytes, and layout.py
    refuses a body it cannot account for every one of -- three of the bools
    objects came back `0x006d: 3 bytes between the ops are not
    instructions` when the branch fold resolved the body afterwards. The
    edge stays, over-approximating the control flow, which is the safe
    direction for everything that reads it.
    """
    from qbopt import wholeseg

    for name in ("bools-q-O.obj", "bools-q-noO.obj", "bools-q-O-zd.obj"):
        path = Path("fixtures/omf") / name
        if not path.exists():
            continue
        _out, why = wholeseg.rebuilt(path.read_bytes())
        assert why == wholeseg.REBUILT, f"{name}: {why}"


def test_an_accumulator_chain_leaves_the_loop_whole() -> None:
    """pressx is press with its eight values read at runtime.

    Nothing can fold them, so the whole `a*b + c*d + e*f + g*h` is one
    invariant chain and the loop should hold the accumulate and the counter.
    What kept it inside was the move that starts the chain: refusing every
    register move kept `mov bx,ax` out of the run, and without it the three
    `add bx,ax` behind it read a register nothing in the run wrote.
    """
    seen = _rebuilt("pressx-p-g2")
    back = [
        (int(text.split()[-1].rstrip("h"), 16), ip)
        for ip, text in seen
        if text.startswith(("jle", "jl ")) and int(text.split()[-1].rstrip("h"), 16) < ip
    ]
    assert back, "nothing loops here, so this proves nothing"
    lo, hi = min(back, key=lambda one: one[1] - one[0])
    inside = [text for ip, text in seen if lo <= ip <= hi]
    assert not [text for text in inside if text.startswith("imul")], (
        f"an invariant product is still in the loop: {inside}"
    )


def test_a_served_read_names_the_value_and_not_a_register() -> None:
    """`add ax,[y]` served from a register used to say which register.

    It asked `_at_width(root, width)` and wrote the answer down -- the
    allocator's answer, given by a pass, which is rule 5's whole subject and
    forward.py's 22 machine references in one line. It says mir.Held now --
    the value -- and lower.py is where that becomes a register.
    """
    from qbopt.blocks import code_map
    from qbopt import blocks as split

    seen = 0
    for obj in sorted(Path("fixtures/omf").glob("*-p-g2.obj")):
        found = corpus.loaded(obj)
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _name, body in mir.bodies(found, split.partition(found, mapped)):
            after = transform.forwarded(body, found.dgroup, found.calls)
            if after is body:
                continue
            for block in after.blocks:
                for op in block.ops:
                    if op.raised is None or (op.args, op.results) == op.raised or op.loads:
                        continue
                    if not any(isinstance(one, mir.Cell) for one in op.raised[0]):
                        continue
                    held = [one for one in op.args if isinstance(one, mir.Held)]
                    assert held, f"{obj.stem} {op.at:#06x}: served read names a register"
                    seen += 1
    assert seen, "nothing was served, so this proves nothing"



def _bodies(name: str):
    """Every raised body of one fixture, before any pass has run."""
    from qbopt import module
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path(f"fixtures/omf/{name}").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    return found, list(mir.bodies(found, split.partition(found, mapped)))


def test_a_fold_names_the_value_not_the_register() -> None:
    """Fold reused the destination operand BC had written, which is a
    register, and MIR carrying it forward is a pass naming one."""
    found, bodies = _bodies("hotlop-p-g2.obj")
    held = [
        one
        for name, body in bodies
        for block in transform.applied(body, found.dgroup, found.calls, only="fold").blocks
        for op in block.ops
        for one in op.results
        if isinstance(one, mir.Held) and op.raised is not None and (op.args, op.results) != op.raised
    ]
    assert held, "fold no longer says which value it writes"


def test_an_unplaced_held_keeps_its_fold() -> None:
    """Cost of getting this wrong: 693 bytes, then a miscompile.

    A fold says `this value gets this constant`. Where the allocation has
    no register for the value -- the allocator refuses about seventy bodies
    in the corpus -- the operand the original instruction had in the same
    position is what the pass took away, so that is what it emits as.
    Dropping the rewrite instead emits the load it replaced, which is not a
    fold; handing the choice to the allocation instead disagrees with every
    operation emitted from its own bytes, and lngmix printed 110 for 142900
    with its dividend deleted.
    """
    from qbopt import ir
    from qbopt import layout
    from qbopt import lower

    found, bodies = _bodies("hotlop-p-g2.obj")
    for name, body in bodies:
        done = transform.applied(body, found.dgroup, found.calls, only="fold")
        folded = [
            op
            for block in done.blocks
            for op in block.ops
            if op.raised is not None and (op.args, op.results) != op.raised
        ]
        if not folded:
            continue
        got = layout._grounded(done, {})  # nothing placed at all
        by_at = {op.at: op for block in got.blocks for op in block.ops}
        for op in folded:
            what = lower.current(by_at[op.at])
            assert what is not None, f"{name}: {op.at:#x} lost its rewrite"
            assert any(isinstance(one, ir.Imm) for one in what.sources), (
                f"{name}: {op.at:#x} went back to the read it replaced"
            )
            assert all(not isinstance(one, ir.Held) for one in what.dests), (
                f"{name}: {op.at:#x} names no place at all"
            )
        return
    raise AssertionError("no body folded anything; the test measures nothing")


def test_what_leaves_a_loop_is_its_own_variable_and_keeps_its_origin() -> None:
    """Two halves of one fix, and each is useless without the other.

    In MIR a register is a variable, so two values BC kept in one register
    are one variable -- true only while nothing has moved them. The moment
    a computation leaves a loop it is not: hotlpx's product and its counter
    both lived in ax, and re-deriving SSA per register put a phi over them
    that said the loop's reads of the product were reads of the counter.
    Nothing downstream could see a conflict because in MIR there was none.

    So the hoist gives everything the run defines a variable of its own --
    which is what `_insertion` was doing when it picked a spare register,
    said without naming one. And the values keep their origin: "where BC
    had it" is still true of them and is what layout remaps an operand
    through. Drop it and the operand keeps the register the instruction was
    raised with, whatever the allocator decided, and the hoisted load lands
    on the counter again.
    """
    from qbopt import module
    from qbopt import omf
    from qbopt import blocks as split
    from qbopt.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlpx-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)

    seen = 0
    for _name, body in mir.bodies(found, blocks):
        before = {value.variable for value in body.origin}
        after = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        fresh = {value.variable for value in after.origin} - before
        if not fresh:
            continue
        seen += 1
        # Every value of a fresh variable still says where BC had it.
        placed = [value for value in after.origin if value.variable in fresh]
        assert placed, "a fresh variable with no origin cannot be remapped"
        # And no phi joins a fresh variable to one that was there before.
        for block in after.blocks:
            for phi in block.phis:
                names = {one.variable for one in phi.incoming.values()} | {phi.result.variable}
                assert not (names & fresh) or names <= fresh, (
                    f"{phi.result} joins a hoisted value to something else"
                )
    assert seen, "nothing left a loop, so this proves nothing"
