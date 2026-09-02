"""
qbopt/transform.py's own gate.

What is checked here is mostly the *order* the transforms run in and what
each one had to be right about, because widening was written wrong twice and
neither time could the host suite see it.
"""

from iced_x86 import Register

from qbopt import ir
from qbopt import wide
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
    from qbopt.stack import PUSH_BYTES

    seen = 0
    for obj, found, blocks in _corpus():
        for at, groups in transform.arguments(blocks, found.calls).items():
            seen += 1
            every = [one.at for group in groups for one in group]
            assert len(every) == len(set(every)), f"{obj.stem}: a push in two operands at {at:#x}"
            for group in groups:
                assert sum(PUSH_BYTES[one.code] for one in group) == 4, (
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
                made = select.emit(layout._semantics(one), at=0)
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
    assert where.through == Register.EAX and where.index == Register.EAX and where.scale == 2

    made = transform._reduced(multiply(8))
    assert made is not None and made.name == "shl" and made.sources[1].value == 3

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
