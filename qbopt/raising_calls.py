"""Recover arithmetic arguments from the values present at each stack push."""

from dataclasses import replace

from iced_x86 import Register

from qbopt import calls, flags, ir, mir


def arithmetic(body: mir.MirBody, found, blocks) -> mir.MirBody:
    sites = calls.sites(found, [insn for block in blocks for insn in block.insns], blocks)
    live_flags = flags.live_in(blocks)
    candidates = {
        site.at: site
        for site in sites
        if not site.pushed
        and site.consume
        and site.name in (*calls.DIVIDES, calls.MULTIPLY)
        and not isinstance(calls.absorb(site, mir._flags_after(blocks, live_flags, site.start, site.end)), str)
    }
    if not candidates:
        return body
    values = {value for block in body.blocks for op in block.ops for value in (*op.defines, *op.uses)}
    values |= {value for block in body.blocks for phi in block.phis for value in (phi.result, *phi.incoming.values())}
    values |= set(body.origin)
    readers = {value for block in body.blocks for op in block.ops for value in op.uses if value not in op.merges}
    readers |= {value for block in body.blocks for phi in block.phis for value in phi.incoming.values()}
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)

    def fresh(at):
        nonlocal serial, variable
        serial += 1
        variable += 1
        return mir.Value(serial, at, variable=variable, version=1)

    changed = []
    removed_flags = set()
    for block in body.blocks:
        pushes = {op.at: op for op in block.ops if op.kind is mir.Kind.ARG and len(op.args) == 1}
        replacements = {}
        for call in block.ops:
            site = candidates.get(call.at)
            if site is None or call.kind is not mir.Kind.CALL:
                continue
            groups = calls.grouped(site.consume)
            if groups is None or len(groups) != 2:
                continue
            incoming = [[pushes.get(insn.at) for insn in group] for group in groups]
            if any(
                op is None or id(op) in replacements or not isinstance(op.args[0], (mir.Held, mir.Const, mir.Cell))
                for group in incoming
                for op in group
            ):
                continue
            returns = {body.origin.get(value): value for value in call.defines if not value.flags}
            if Register.EAX not in returns or Register.EDX not in returns:
                continue
            if any(
                value in readers for register, value in returns.items() if register not in (Register.EAX, Register.EDX)
            ):
                continue
            pending, arguments, setup = {}, [], []
            for group in incoming:
                words = []
                for push in group:
                    arg = push.args[0]
                    width = arg.ref.width if isinstance(arg, mir.Cell) else arg.width
                    value = fresh(push.at)
                    held = mir.Held(value, width)
                    pending[id(push)] = (
                        replace(
                            push,
                            kind=mir.Kind.LOAD if isinstance(arg, mir.Cell) else mir.Kind.COPY,
                            op=ir.Operation.MOVE,
                            name="mov",
                            defines=(value,),
                            results=(held,),
                            stores=(),
                            made=None,
                            raised=None,
                            merges={},
                            stack=None,
                        ),
                    )
                    words.append(held)
                if len(words) == 1 and words[0].width == 4:
                    arguments.append(words[0])
                elif len(words) == 2 and all(word.width == 2 for word in words):
                    value = fresh(call.at)
                    setup.append(
                        mir.Op(
                            call.at,
                            mir.Synth.CONCAT_LOW,
                            "concat",
                            (value,),
                            tuple(word.value for word in words),
                            kind=mir.Kind.CONCAT,
                            args=tuple(words),
                            results=(mir.Held(value, 4),),
                            covers=(call.at, call.at),
                        )
                    )
                    arguments.append(mir.Held(value, 4))
                else:
                    break
            if len(arguments) != 2:
                continue
            quotient, remainder = fresh(call.at), fresh(call.at)
            multiply = site.name == calls.MULTIPLY
            answers = (quotient,) if multiply else (quotient, remainder)
            # These runtime routines push their right operand first.
            arithmetic = mir.Op(
                call.at,
                ir.Operation.MULTIPLY if multiply else ir.Operation.DIVIDE,
                "imul" if multiply else "idiv",
                answers,
                tuple(arg.value for arg in reversed(arguments)),
                kind=mir.Kind.MUL if multiply else mir.Kind.DIVMOD,
                args=tuple(reversed(arguments)),
                results=tuple(mir.Held(value, 4) for value in answers),
                covers=call.covers,
                id=call.id,
            )
            answer = remainder if site.name == calls.REMAINDER else quotient
            extracts = tuple(
                mir.Op(
                    call.at,
                    mir.Synth.HALF_TO_LOW,
                    "extract",
                    (returns[register],),
                    (answer,),
                    kind=mir.Kind.EXTRACT,
                    args=(mir.Held(answer, 4), mir.Const(offset, 4)),
                    results=(mir.Held(returns[register], 2),),
                    covers=(call.at, call.at),
                )
                for register, offset in ((Register.EAX, 0), (Register.EDX, 16))
            )
            replacements.update(pending)
            replacements[id(call)] = (*setup, arithmetic, *extracts)
            removed_flags.update(value for value in call.defines if value.flags)
        changed.append(replace(block, ops=tuple(item for op in block.ops for item in replacements.get(id(op), (op,)))))
    # These runtime arithmetic helpers do not consume incoming flags. The
    # original call nodes conservatively carried a flag dependency anyway.
    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(op, uses=tuple(value for value in op.uses if value not in removed_flags))
                    if op.kind is mir.Kind.CALL
                    and found.calls.get(op.at) in (calls.MULTIPLY, calls.DIVIDE, calls.REMAINDER)
                    else op
                    for op in block.ops
                ),
            )
            for block in changed
        ),
    )
