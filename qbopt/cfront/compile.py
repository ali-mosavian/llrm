"""C through Open Watcom's front end and qbopt's backend, to an object or jwasm source.

    python -m qbopt.cfront pal.c -o pal.obj -I src [--dump DIR] [--opt]

An `.obj` output is written here; anything else is jwasm source.

The front end is owshim/bin/wccq (owshim/build.sh). A `.cgs` stream it
already wrote is accepted in place of the source.
"""

import os
import argparse
import tempfile
import subprocess
from pathlib import Path
from collections.abc import Callable
from collections.abc import Iterator

from qbopt import flow
from qbopt.model import lir
from qbopt.model import mir
from qbopt.cfront import hir
from qbopt.backend import masm
from qbopt.backend import jumps
from qbopt.backend import lower
from qbopt.backend import machinedce
from qbopt.cfront import stream
from qbopt.cfront import libfunc
from qbopt.backend import phielim
from qbopt.backend import omfwrite
from qbopt.backend import prologue
from qbopt.cfront import raise_hir
from qbopt.backend import lower_int64
from qbopt.backend import cpu as targets
from qbopt.backend import frame as frames
from qbopt.objectfile.module import Space

WCCQ = Path(__file__).resolve().parents[2] / "owshim" / "bin" / "wccq"
# Borland's medium model: far code, near data, cdecl, byte-packed structs,
# 16-bit enums, x87 inline, no stack probes, no default library. -fp3 is for
# inline assembly: qcport's own uses 387 instructions.
FLAGS = (
    "-mm", "-3", "-fpi87", "-fp3", "-zp1", "-ei", "-ecc", "-s", "-zl", "-zq",
    f"-fi={Path(__file__).with_name('borland.h')}",
)  # fmt: skip

type Watch = Callable[[str, str, object], None]


def _address_taken_procedures(unit: hir.Unit) -> frozenset[str]:
    """Internal procedure symbols used as values rather than direct callees."""
    direct = set()
    for call in unit.calls.values():
        if not call.target.startswith("n"):
            continue
        target = hir.handle(call.target)
        node = unit.nodes.get(target)
        if (
            node is not None
            and node.call == "CGFEName"
            and node.args
            and node.args[0].startswith("y")
            and hir.handle(node.args[0]) == call.symbol
        ):
            direct.add(target)

    referenced = set()
    sequences = [node.args for node in unit.nodes.values()]
    sequences += [statement.args for proc in unit.procs for statement in proc.body]
    sequences += [(node,) for call in unit.calls.values() for node, _type in call.parms]
    for args in sequences:
        referenced.update(hir.handle(arg) for arg in args if arg.startswith("n") and arg[1:].isdigit())

    taken = {
        symbol.object_name
        for at, node in unit.nodes.items()
        if node.call == "CGFEName"
        and node.args
        and node.args[0].startswith("y")
        and (symbol := unit.symbols.get(hir.handle(node.args[0]))) is not None
        and symbol.proc
        and (at not in direct or at in referenced)
    }
    taken.update(
        unit.symbols[symbol].object_name
        for symbol in unit.backs.values()
        if symbol in unit.symbols and unit.symbols[symbol].proc
    )
    taken.update(
        unit.symbols[fixup.symbol].object_name
        for symbol in unit.symbols.values()
        if symbol.code is not None
        for fixup in symbol.code.fixups
        if fixup.symbol in unit.symbols and unit.symbols[fixup.symbol].proc
    )
    taken.update(
        symbol.object_name
        for segment in unit.segments.values()
        for call, args in segment.items
        if call == "DGFEPtr"
        and (symbol := unit.symbols.get(hir.handle(args[0]))) is not None
        and symbol.proc
    )
    return frozenset(taken)


def _reachable_procedures(
    procedures: list[raise_hir.Raised], bodies: dict[str, mir.MirBody], roots: frozenset[str]
) -> frozenset[str]:
    """Defined procedure bodies reachable through calls that survived MIR."""
    defined = {one.name for one in procedures}
    if not roots:
        return frozenset(defined)
    by_name = {one.name: one for one in procedures}
    reached, pending = set(), list(roots & defined)
    while pending:
        name = pending.pop()
        if name in reached:
            continue
        reached.add(name)
        procedure = by_name[name]
        sites = {op.at for block in bodies[name].blocks for op in block.ops if op.kind is mir.Kind.CALL}
        pending.extend(
            target
            for at, target in procedure.calls.items()
            if at in sites and target in defined and target not in reached
        )
    return frozenset(reached)


def _data_labels(unit: hir.Unit) -> dict[int, tuple[int, int, int]]:
    """Each named data object's `(segment, first item, after item)` span.

    A `DGLabel` is the one record in the C stream that binds following bytes
    to a source-level object.  Keeping that boundary rather than guessing
    from byte sizes lets fresh emission delete whole private objects while
    leaving alignment, pointers, and anonymous literal data conservative.
    """
    out = {}
    for segment in unit.segments.values():
        labels = [
            (at, unit.backs.get(hir.handle(args[0])))
            for at, (call, args) in enumerate(segment.items)
            if call == "DGLabel" and args
        ]
        for number, (start, symbol) in enumerate(labels):
            if symbol in unit.symbols:
                after = labels[number + 1][0] if number + 1 < len(labels) else len(segment.items)
                out[symbol] = (segment.id, start, after)
    return out


def _referenced_data(body: mir.MirBody, candidates: frozenset[int]) -> set[int]:
    """Private data symbols that the emitted MIR directly names.

    MIR represents an address both as a relocation operand and as the cell
    it reads or writes.  Looking at both forms makes this independent of
    folding: a surviving address calculation, load, store, or array request
    roots the object equally.  Selector relocations are included for an
    object in its own far-data segment.
    """
    found = set()

    def symbol(one: mir.Symbol) -> None:
        if one.space is Space.SEGMENT and one.index in candidates:
            found.add(one.index)
        if one.space is Space.GROUP and one.index - raise_hir.SELECTOR in candidates:
            found.add(one.index - raise_hir.SELECTOR)

    def ref(one: mir.MemRef) -> None:
        if one.addr is not None:
            symbol(mir.Symbol(one.addr.space, one.addr.index, one.addr.disp, one.width))
        for named in (one.symbolic, one.allocation):
            if named is not None:
                symbol(named)

    for block in body.blocks:
        for op in block.ops:
            for operand in (*op.args, *op.results):
                if isinstance(operand, mir.Symbol):
                    symbol(operand)
                elif isinstance(operand, mir.Cell):
                    ref(operand.ref)
            for reference in (*op.loads, *op.stores):
                ref(reference)
            for reference, _value in op.memory_values:
                ref(reference)
            if op.array is not None:
                symbol(op.array.descriptor)
    return found


def _reachable_data(unit: hir.Unit, bodies: dict[str, mir.MirBody]) -> frozenset[int]:
    """Named data proven observable from emitted code, linkage, or data.

    OMF has no private-data reachability metadata.  We therefore delete only
    a labelled non-procedure symbol that is neither imported nor public, and
    only after roots from every emitted body and inline-assembly relocation
    have been closed over data-initializer pointers.  Everything unlabelled,
    externally visible, or of unknown provenance stays emitted.
    """
    labels = _data_labels(unit)
    candidates = frozenset(
        symbol
        for symbol in labels
        if not unit.symbols[symbol].proc and not unit.symbols[symbol].imported and not unit.symbols[symbol].exported
    )
    kept = set(labels) - candidates
    for body in bodies.values():
        kept.update(_referenced_data(body, candidates))
    # Inline assembly's source bytes are intentionally opaque to MIR.  Its
    # relocation table is the exact equivalent reference evidence, so it is
    # a root rather than a reason to disable data DCE for the whole module.
    for symbol in unit.symbols.values():
        if symbol.code is not None:
            kept.update(fixup.symbol for fixup in symbol.code.fixups if fixup.symbol in candidates)

    changed = True
    while changed:
        changed = False
        for symbol in tuple(kept):
            span = labels.get(symbol)
            if span is None:
                continue
            segment, start, after = span
            for call, args in unit.segments[segment].items[start:after]:
                target = None
                if call == "DGFEPtr":
                    target = hir.handle(args[0])
                elif call == "DGBackPtr":
                    target = unit.backs.get(hir.handle(args[0]))
                if target in candidates and target not in kept:
                    kept.add(target)
                    changed = True
    return frozenset(kept)


def recorded(source: Path, includes: list[str]) -> str:
    """The code-generator stream wccq records for one C file."""
    with tempfile.TemporaryDirectory() as scratch:
        out = Path(scratch) / "unit.cgs"
        searched = tuple(f"-I{Path(one).resolve()}" for one in includes)
        command = [str(WCCQ), *FLAGS, *searched, f"-fo={scratch}/unit.obj", str(source.resolve())]
        # In the scratch directory, where wccq also leaves its .err file.
        environment = {**os.environ, "QBOPT_CG_STREAM": str(out)}
        done = subprocess.run(command, env=environment, capture_output=True, text=True, cwd=scratch)
        if done.returncode != 0 or not out.exists():
            raise hir.Unsupported(f"wccq failed on {source}:\n{done.stdout}{done.stderr}")
        return out.read_text()


def assembled(
    text: str,
    module: str,
    *,
    optimise: bool = False,
    dump: Path | None = None,
    cpu: str | targets.Profile = "386",
    watch: Watch | None = None,
) -> masm.Module:
    target = targets.profile(cpu)
    unit = hir.unit(stream.parse(text))
    _write(dump, "stream", text)
    _write(dump, "hir", hir.text(unit))
    procedures, mirs, lirs = [], [], []
    shared = raise_hir.Shared()
    raised_procedures = [raise_hir.raised(unit, proc, shared) for proc in unit.procs]
    from qbopt.analysis import alias

    aliases = {one.name: alias.Procedure(one.body, one.calls, one.arguments) for one in raised_procedures}
    callees = {name for one in raised_procedures for name in one.calls.values()}
    modref = alias.summaries(aliases, libfunc.summaries(callees))
    bodies = {one.name: alias.calls_annotated(aliases[one.name], modref) for one in raised_procedures}
    from qbopt.analysis import interprocedural

    address_taken = _address_taken_procedures(unit)
    call_arguments = {
        one.name: interprocedural.argument_sites(bodies[one.name], one.contracts) for one in raised_procedures
    }
    for raised in raised_procedures:
        if watch is not None:
            watch("mir-raised", raised.name, bodies[raised.name])

    if optimise:
        private = frozenset(
            one.name for one in raised_procedures if not one.symbol.exported and one.name not in address_taken
        )
        constants = interprocedural.constant_parameters(
            {one.name: (one.calls, one.constants) for one in raised_procedures}, private
        )
        for raised in raised_procedures:
            if raised.name not in constants:
                continue
            bodies[raised.name] = interprocedural.specialize_parameters(
                bodies[raised.name], raised.parameters, constants[raised.name]
            )
            if watch is not None:
                watch("mir-ipa-args", raised.name, bodies[raised.name])

    def run_optimiser(raised: raise_hir.Raised, body: mir.MirBody, prefix: str = "") -> mir.MirBody:
        from qbopt.optimize import rotate
        from qbopt.optimize import transform

        def observe(stage: str, after: mir.MirBody, name: str = raised.name) -> None:
            stage = f"{prefix}{stage}"
            _write(dump, f"passes/{name}.{stage}", _mir_text(name, after))
            if watch is not None:
                watch(f"mir-{stage}", name, after)

        body = transform.applied(
            body,
            frozenset(),
            raised.calls,
            found=None,
            # Borland's medium-model C ABI preserves SI and DI from the
            # six value registers. A recurrence live through a call has
            # two places available, not the full register file.
            registers=target.register_capacity,
            call_registers=target.call_register_capacity,
            index_scales=target.address_scales,
            costs=target.operations,
            watch=observe if dump is not None or watch is not None else None,
        )
        body = rotate.entered(body)
        if dump is not None or watch is not None:
            observe("rotate", body)
        return body

    if optimise:
        bodies = {one.name: run_optimiser(one, bodies[one.name]) for one in raised_procedures}

        # Inline only after each independent body has reached its local fixed
        # point.  The splice itself is MIR, and its result goes straight back
        # through that same pipeline; there is no second interprocedural
        # optimizer hidden below the MIR boundary.
        from qbopt.optimize import inline

        pure = interprocedural.pure_procedures({one.name: (bodies[one.name], one.calls) for one in raised_procedures})
        inline_round = 0
        while True:
            counts = inline.call_counts(bodies, {one.name: one.calls for one in raised_procedures})
            available = inline.candidates(
                bodies,
                {one.name: one.parameters for one in raised_procedures},
                counts,
                private,
                pure,
                target.cost("call_far"),
            )
            changed = False
            for raised in raised_procedures:
                before = bodies[raised.name]
                constant = inline.constant_sites(
                    bodies,
                    {one.name: one.parameters for one in raised_procedures},
                    raised.calls,
                    raised.constants,
                    private,
                    pure,
                    target.cost("call_far"),
                )
                after = inline.expanded(before, raised.calls, call_arguments[raised.name], available, constant)
                if after is before:
                    continue
                stage = f"inline{inline_round}"
                _write(dump, f"passes/{raised.name}.{stage}", _mir_text(raised.name, after))
                if watch is not None:
                    watch(f"mir-{stage}", raised.name, after)
                bodies[raised.name] = run_optimiser(raised, after, f"{stage}.")
                changed = True
                inline_round += 1
            if not changed:
                break

        propagated = {one.name: frozenset() for one in raised_procedures}
        return_round = 0

        def propagate_constant_returns() -> None:
            """Materialize every newly constant result, retaining seen calls."""
            nonlocal return_round
            while True:
                returns = interprocedural.constant_returns(bodies)
                changed = False
                for raised in raised_procedures:
                    before = bodies[raised.name]
                    after, done = interprocedural.propagate_returns(
                        before, raised.calls, returns, propagated[raised.name]
                    )
                    propagated[raised.name] = done
                    if after is before:
                        continue
                    bodies[raised.name] = run_optimiser(raised, after, f"ipa{return_round}.")
                    changed = True
                if not changed:
                    return
                return_round += 1

        # A return fact may make the actual of a different direct call
        # constant.  Alternate that current-MIR proof with return propagation
        # until neither side discovers a new fact; source-side constants alone
        # cannot close this chain.
        propagate_constant_returns()
        argument_round = 0
        while True:
            constants = interprocedural.current_parameter_constants(
                bodies,
                {one.name: one.calls for one in raised_procedures},
                call_arguments,
                {one.name: one.parameters for one in raised_procedures},
                private,
            )
            changed = False
            for raised in raised_procedures:
                constants_for_body = constants.get(raised.name)
                if constants_for_body is None:
                    continue
                before = bodies[raised.name]
                after = interprocedural.specialize_parameters(before, raised.parameters, constants_for_body)
                if after is before:
                    continue
                bodies[raised.name] = run_optimiser(raised, after, f"ipa-args{argument_round}.")
                changed = True
            if changed:
                argument_round += 1
                propagate_constant_returns()

            # A single current-MIR constant may be worth cloning even where
            # another caller keeps the private body dynamic.  This is the
            # same costed call-site policy used for source literals, with its
            # facts now coming from the interprocedural fixed point.
            counts = inline.call_counts(bodies, {one.name: one.calls for one in raised_procedures})
            available = inline.candidates(
                bodies,
                {one.name: one.parameters for one in raised_procedures},
                counts,
                private,
                pure,
                target.cost("call_far"),
            )
            inlined = False
            for raised in raised_procedures:
                before = bodies[raised.name]
                current = interprocedural.current_call_constants(
                    before,
                    raised.calls,
                    call_arguments[raised.name],
                    {one.name: one.parameters for one in raised_procedures},
                )
                constant = inline.constant_sites(
                    bodies,
                    {one.name: one.parameters for one in raised_procedures},
                    raised.calls,
                    current,
                    private,
                    pure,
                    target.cost("call_far"),
                )
                after = inline.expanded(before, raised.calls, call_arguments[raised.name], available, constant)
                if after is before:
                    continue
                bodies[raised.name] = run_optimiser(raised, after, f"ipa-inline{argument_round}.")
                inlined = True
            if inlined:
                propagate_constant_returns()
            if not changed and not inlined:
                break
        pure = interprocedural.pure_procedures({one.name: (bodies[one.name], one.calls) for one in raised_procedures})
        readonly = interprocedural.readonly_procedures(
            {one.name: (bodies[one.name], one.calls) for one in raised_procedures}
        )
        for raised in raised_procedures:
            before = bodies[raised.name]
            after = interprocedural.remove_dead_pure_calls(before, raised.calls, readonly, call_arguments[raised.name])
            if after is not before:
                bodies[raised.name] = run_optimiser(raised, after, "ipa-pure.")
        # A direct private body whose every path stops (for example an exact
        # infinite loop) makes the tail of every call site unreachable. Keep
        # the physical call, but remove only the code that would require it
        # to return, then repeat because its caller may now be terminal too.
        while True:
            noreturn = interprocedural.noreturn_procedures(
                {one.name: (bodies[one.name], one.calls) for one in raised_procedures}, private
            )
            changed = False
            for raised in raised_procedures:
                before = bodies[raised.name]
                after = interprocedural.terminal_calls(before, raised.calls, noreturn)
                if after is before:
                    continue
                bodies[raised.name] = run_optimiser(raised, after, "ipa-noreturn.")
                changed = True
            if not changed:
                break
        roots = frozenset(one.name for one in raised_procedures if one.symbol.exported) | address_taken
        reachable = _reachable_procedures(raised_procedures, bodies, roots)
        raised_procedures = [one for one in raised_procedures if one.name in reachable]

    for raised in raised_procedures:
        body = bodies[raised.name]
        mirs.append(_mir_text(raised.name, body))
        if optimise:
            mirs.append(_mir_text(raised.name + " (opt)", body))
        legalized = lower_int64.expanded(body, raised.calls, raised.contracts, raised.hints)
        body = legalized.body
        if dump and body is not raised.body:
            _write(dump, f"passes/{raised.name}.int64-lower", _mir_text(raised.name, body))
        low = flow.verified(
            lower.lowered(
                raised.name,
                body,
                legalized.calls,
                {},
                legalized.contracts,
                {},
                cpu=target,
                hints=legalized.hints,
            ),
            "lower",
            in_ssa=True,
        )
        if watch is not None:
            watch("lir-lower", raised.name, low)
        lirs.append(_lir_text(raised.name, low))
        frame = frames.of(low, legalized.calls)
        in_ssa = True
        for number, phase in enumerate(flow.machine(flow._pinned(low), frame, legalized.calls, cpu=target)):
            if not isinstance(phase, prologue.Prologue):
                if isinstance(phase, phielim.PhiElimination):
                    in_ssa = False
                low = flow.checked(low, phase, in_ssa=in_ssa)
                _write(dump, f"phases/{raised.name}.{number:02d}-{type(phase).__name__}", _lir_text(raised.name, low))
                if watch is not None:
                    watch(f"lir-{phase.name or type(phase).__name__}", raised.name, low)
        low = jumps.placed(low)
        baseline = jumps.threaded(low)
        # Merging one physical tail may make the condition selecting between
        # its former copies dead; deleting that compare can in turn make the
        # predecessor tails identical. Settle those two machine facts before
        # final threading chooses fall-throughs.
        for _round in range(max(1, len(low.blocks) + len(low.insns))):
            before = low
            low = machinedce.eliminated(jumps.merged(low))
            if low is before:
                break
        low = jumps.preferred(baseline, jumps.threaded(low))
        if watch is not None:
            watch("lir-layout", raised.name, low)
        lirs.append(_lir_text(raised.name + " (allocated)", low))
        reserve = -min(min(frame.slots.values(), default=0), frame.floor)
        callees = {
            at: masm.Callee(one.object_name, one.far, raised.inline.get(at, ())) for at, one in raised.callees.items()
        }
        callees.update({at: masm.Callee(legalized.calls[at], False, code) for at, code in legalized.inline.items()})
        procedures.append(masm.Procedure(raised.name, raised.symbol.exported, raised.symbol.far, low, reserve, callees))
    _write(dump, "mir", "\n".join(mirs))
    _write(dump, "lir", "\n".join(lirs))
    built = masm.Module(
        code=f"{module.upper()}_TEXT",
        names=raise_hir.names(unit, shared),
        externs=_externs(unit) + tuple((one.object_name, "far") for one in shared.runtime.values()),
        publics=tuple(one.object_name for one in unit.symbols.values() if one.exported),
        data=(
            *(
                _data(unit, _reachable_data(unit, {one.name: bodies[one.name] for one in raised_procedures}))
                if optimise
                else _data(unit)
            ),
            *_literals(shared),
        ),
        procedures=tuple(procedures),
        private=frozenset(one.name for one in unit.segments.values() if one.attr & hir.PRIVATE),
    )
    _write(dump, "asm", masm.text(built))
    return built


def compiled(
    text: str,
    module: str,
    *,
    optimise: bool = False,
    dump: Path | None = None,
    cpu: str | targets.Profile = "386",
    watch: Watch | None = None,
) -> str:
    """The module as jwasm source."""
    return masm.text(assembled(text, module, optimise=optimise, dump=dump, cpu=cpu, watch=watch))


def _externs(unit: hir.Unit) -> tuple[tuple[str, str], ...]:
    return tuple(
        (
            one.object_name,
            ("far" if one.far else "near") if one.proc else "byte" if unit.grouped(one) else "far-byte",
        )
        for one in unit.symbols.values()
        if one.imported and one.code is None and one.name not in raise_hir.EMITTED
    )


def _data(unit: hir.Unit, kept: frozenset[int] | None = None) -> Iterator[tuple[str, tuple[masm.Datum, ...]]]:
    """Each data segment's items."""
    for segment in unit.segments.values():
        if not segment.items or segment.attr & 0x1:  # EXEC: code has no data items
            continue
        items = []
        spans = _data_labels(unit)
        dropped = {
            start
            for symbol, (segment_id, start, _after) in spans.items()
            if kept is not None and segment_id == segment.id and symbol not in kept
        }
        skip = False
        for at, (call, args) in enumerate(segment.items):
            if at in dropped:
                skip = True
            elif call == "DGLabel":
                skip = False
            if skip:
                continue
            match call, args:
                case "DGLabel", (back,):
                    symbol = unit.backs[hir.handle(back)]
                    items.append(masm.Label(unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"))
                case "DGUBytes", (size,):
                    items.append(masm.Fill(int(size), None if segment.name == "_BSS" else 0))
                case "DGIBytes", (size, byte):
                    items.append(masm.Fill(int(size), int(byte)))
                case "DGBytes", (_size, data):
                    items.append(bytes.fromhex(data))
                case "DGInteger", (value, type_):
                    # The shim prints a negative item as its 32-bit two's complement.
                    width = raise_hir.WIDTHS.get(type_, 2)
                    items.append((int(value) & ((1 << (8 * width)) - 1)).to_bytes(width, "little"))
                case "DGFEPtr", (symbol, type_, offset):
                    far = type_ in raise_hir.FAR_POINTERS or type_ in ("TY_LONG_CODE_PTR", "TY_CODE_PTR")
                    items.append(masm.Pointer(unit.symbols[hir.handle(symbol)].object_name, int(offset), far))
                case "DGBackPtr", (back, _segment, offset, type_):
                    symbol = unit.backs[hir.handle(back)]
                    name = unit.symbols[symbol].object_name if symbol else f"L_b{hir.handle(back)}"
                    items.append(masm.Pointer(name, int(offset), type_ in raise_hir.FAR_POINTERS))
                case "DGAlign", (align,):
                    items.append(masm.Align(int(align)))
                case _:
                    raise hir.Unsupported(f"data item {call} {' '.join(args)}")
        yield segment.name, tuple(items)


def _literals(shared: raise_hir.Shared) -> tuple[tuple[str, tuple[masm.Datum, ...]], ...]:
    """The float constants the raise placed, in DGROUP's constant segment."""
    lines = []
    for packed, number in shared.literals.items():
        lines += [masm.Label(f"L_f{number}"), bytes(packed)]
    return (("CONST", tuple(lines)),) if lines else ()


def _mir_text(name: str, body: mir.MirBody) -> str:
    out = [f"== {name}"]
    for block in body.blocks:
        out.append(f"block {block.at} -> {block.succ}")
        out += [f"  phi {phi}" for phi in block.phis]
        for op in block.ops:
            extra = f" test={op.test} target={op.target}" if op.test or op.target is not None else ""
            out.append(f"  {op.at:4} {op.kind} {op.args} -> {op.results}{extra}")
    return "\n".join(out) + "\n"


def _lir_text(name: str, body: lir.LirBody) -> str:
    out = [f"== {name}"]
    for block in body.blocks:
        out.append(f"block {block.at} -> {block.succ}")
        out += [f"  phi {phi}" for phi in block.phis]
        out += [f"  {one.at:4} {one.what} req={one.requires} del={one.delivers}" for one in block.insns]
    return "\n".join(out) + "\n"


def _write(dump: Path | None, stage: str, text: str) -> None:
    if dump is not None:
        (dump / stage).parent.mkdir(parents=True, exist_ok=True)
        (dump / stage).write_text(text)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m qbopt.cfront", description=__doc__.splitlines()[0])
    parser.add_argument("source", type=Path)
    parser.add_argument("-o", "--output", type=Path)
    parser.add_argument("-I", "--include", action="append", default=[])
    parser.add_argument("--dump", type=Path)
    parser.add_argument("--opt", action="store_true")
    parser.add_argument("--cpu", choices=targets.names(), default="386", help="code-generation tuning target")
    args = parser.parse_args(argv)
    text = args.source.read_text() if args.source.suffix == ".cgs" else recorded(args.source, args.include)
    output = args.output or args.source.with_suffix(".asm")
    built = assembled(text, args.source.stem, optimise=args.opt, dump=args.dump, cpu=args.cpu)
    if output.suffix.lower() == ".obj":
        output.write_bytes(omfwrite.written(built, args.source.name))
    else:
        output.write_text(masm.text(built))
    return 0
