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


def _absorbed_ops(obj, found, blocks):
    """Every absorbed site in one object, as (call address, the ops it became).

    The operations one site becomes are exactly the ones sitting inside the
    call's own bytes: absorption replaces the call and nothing else, so the
    pushes keep their addresses and everything in [call, call+5) is new.
    """
    from qbopt import mir

    sites = {
        one.at: one
        for block in blocks
        for one in block.insns
        if (found.calls.get(one.at) or "").upper() in transform.EMITTED
    }
    for _name, body in mir.bodies(found, blocks):
        after = transform.absorbed(body, blocks, found.calls)
        for block in after.blocks:
            for at, call in sites.items():
                ops = [one for one in block.ops if at <= one.at < call.end]
                if len(ops) > 1:
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
        for at, ops in _absorbed_ops(obj, found, blocks):
            name = (found.calls.get(at) or "").upper()
            if name not in seen:
                continue
            seen[name] += 1
            want = ["pop", "pop", "imul", "restore"] if name == "B$MUI4" else [
                "pop", "pop", "cdq", "idiv", "restore"
            ]
            assert [one.name for one in ops] == want, f"{obj.stem} at {at:#x}: {[o.name for o in ops]}"
    assert all(seen.values()), f"nothing absorbed for one of them: {seen}"


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
            if (found.calls.get(at) or "").upper() not in transform.EMITTED:
                continue
            seen += 1
            left, right = groups
            assert max(one.at for one in left) > max(one.at for one in right), (
                f"{obj.stem} at {at:#x}: the left operand is not the one nearest the call"
            )
    assert seen, "no sites, so this proves nothing"


def test_an_absorbed_call_fits_in_the_bytes_it_replaces() -> None:
    """layout.py keys every operation by an address, so a site has as many
    to give as the call has bytes -- five, for the far call BC writes.

    That is the whole reason B$RMI4 is not absorbed here: its answer comes
    back in edx and moving it to eax makes six operations where there is
    room for five. An address budget, not anything about the arithmetic.
    """
    for obj, found, blocks in _corpus():
        for at, ops in _absorbed_ops(obj, found, blocks):
            if (found.calls.get(at) or "").upper() not in transform.EMITTED:
                continue
            call = next(
                one for block in blocks for one in block.insns if one.at == at
            )
            where = [one.at for one in ops]
            assert len(where) == len(set(where)), f"{obj.stem}: two operations on one address at {at:#x}"
            assert min(where) == at and max(where) < call.end, (
                f"{obj.stem} at {at:#x}: {len(ops)} operations for {call.end - at} bytes"
            )


def test_a_site_whose_flags_are_read_is_left_alone() -> None:
    """`imul` and `idiv` leave their own flags and the call left the
    runtime's, so a jcc after the site would read a different answer.

    Checked against what comes out, not against the condition recomputed:
    the call has to still be there.
    """
    from qbopt import mir

    refused = taken = 0
    for obj, found, blocks in _corpus():
        where = transform.arguments(blocks, found.calls)
        for _name, body in mir.bodies(found, blocks):
            read = transform._read_flags(body)
            after = transform.absorbed(body, blocks, found.calls)
            standing = {op.at for block in after.blocks for op in block.ops if op.op is ir.Operation.CALL}
            for block in body.blocks:
                for op in block.ops:
                    if (found.calls.get(op.at) or "").upper() not in transform.EMITTED:
                        continue
                    if op.at not in where:
                        continue
                    if any(one.flags and one in read for one in op.defines):
                        refused += 1
                        assert op.at in standing, (
                            f"{obj.stem}: {op.at:#x} absorbed with its flags read afterwards"
                        )
                    else:
                        taken += 1
                        assert op.at not in standing, f"{obj.stem}: {op.at:#x} not absorbed"
    assert refused and taken, f"refused {refused}, took {taken} -- one of them proves nothing"
