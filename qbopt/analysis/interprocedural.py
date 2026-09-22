"""Facts that cross direct procedure boundaries.

The C raise deliberately gives every procedure an independent MIR body.  A
direct call is consequently opaque to ordinary SCCP even when every return in
the named body has the same value.  This module computes that fact over the
whole compilation unit and materialises it after the call.  The call itself
stays unless a separate purity proof says its effects are unobservable.
"""

from dataclasses import replace

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.analysis import consts
from qbopt.objectfile.module import Space

Returns = dict[str, tuple[mir.Const, ...]]
Parameters = dict[str, tuple[mir.Const | None, ...]]

_MAY_TRAP = frozenset({mir.Kind.DIV, mir.Kind.REM, mir.Kind.DIVMOD, mir.Kind.UDIVMOD, mir.Kind.FIXED_DIV})
_FLOATING = frozenset(
    {
        mir.Kind.FADD,
        mir.Kind.FSUB,
        mir.Kind.FMUL,
        mir.Kind.FDIV,
        mir.Kind.FNEG,
        mir.Kind.FABS,
        mir.Kind.FSQRT,
        mir.Kind.FLOAD,
        mir.Kind.FSTORE,
        mir.Kind.FCOMPARE,
        mir.Kind.FCHECK,
    }
)


def constant_parameters(
    procedures: dict[str, tuple[dict[int, str], dict[int, tuple[mir.Const | None, ...]]]],
    eligible: frozenset[str],
) -> Parameters:
    """Parameter constants agreed by every direct call to a private body."""
    actuals: dict[str, list[tuple[mir.Const | None, ...]]] = {name: [] for name in eligible}
    for calls, constants in procedures.values():
        for at, target in calls.items():
            if target in actuals and at in constants:
                actuals[target].append(constants[at])

    return _agreed_parameters(actuals)


def current_parameter_constants(
    bodies: dict[str, mir.MirBody],
    calls: dict[str, dict[int, str]],
    arguments: dict[str, dict[int, frozenset[int]]],
    parameters: dict[str, tuple[mir.MemRef, ...]],
    eligible: frozenset[str],
) -> Parameters:
    """Parameter constants proved at every *surviving* private call.

    Source-time facts are enough for a literal call, but not for a call whose
    actual becomes constant only after another private return is summarized.
    Read the current MIR instead: SCCP owns the value fact, while the C call
    contract supplies the exact ARG operations belonging to each call.  A
    missing or malformed contract is an unknown call, rather than permission
    to specialize a body that may still receive a different value.
    """
    actuals: dict[str, list[tuple[mir.Const | None, ...]]] = {name: [] for name in eligible}
    for owner, body in bodies.items():
        for at, values in current_call_constants(
            body, calls.get(owner, {}), arguments.get(owner, {}), parameters
        ).items():
            target = calls.get(owner, {}).get(at)
            if target in actuals:
                actuals[target].append(values)

    return _agreed_parameters(actuals)


def current_call_constants(
    body: mir.MirBody,
    calls: dict[int, str],
    arguments: dict[int, frozenset[int]],
    parameters: dict[str, tuple[mir.MemRef, ...]],
) -> dict[int, tuple[mir.Const | None, ...]]:
    """Current SCCP facts for every direct call with a known C contract.

    The result is deliberately per-call instead of per-callee: a costed
    inlining decision may use one constant call even when a second dynamic
    call prevents whole-body parameter specialization.
    """
    facts = consts.known(body)
    out = {}
    for block in body.blocks:
        for index, call in enumerate(block.ops):
            if call.kind is not mir.Kind.CALL or (target := calls.get(call.at)) not in parameters:
                continue
            width = len(parameters[target])
            sites = arguments.get(call.at)
            selected = (
                [op for op in block.ops[:index] if op.kind is mir.Kind.ARG and op.at in sites and len(op.args) == 1]
                if sites is not None
                else []
            )
            # cdecl lays down the final source argument first.
            values = tuple(_constant_argument(op.args[0], facts) for op in reversed(selected))
            out[call.at] = values if len(values) == width else (None,) * width
    return out


def _constant_argument(argument: object, facts: dict[mir.Value, consts.Known]) -> mir.Const | None:
    if isinstance(argument, mir.Const):
        return mir.Const(consts.masked(argument.n, argument.width), argument.width)
    if (
        isinstance(argument, mir.Held)
        and (fact := facts.get(argument.value)) is not None
        and fact.width >= argument.width
    ):
        return mir.Const(consts.masked(fact.n, argument.width), argument.width)
    return None


def _agreed_parameters(actuals: dict[str, list[tuple[mir.Const | None, ...]]]) -> Parameters:
    """Facts shared by every call in an already-normalized actual map."""
    out: Parameters = {}
    for name, sites in actuals.items():
        if not sites or len({len(site) for site in sites}) != 1:
            continue
        agreed = []
        for index in range(len(sites[0])):
            values = {site[index] for site in sites}
            agreed.append(values.pop() if len(values) == 1 and None not in values else None)
        if any(value is not None for value in agreed):
            out[name] = tuple(agreed)
    return out


def specialize_parameters(
    body: mir.MirBody,
    parameters: tuple[mir.MemRef, ...],
    constants: tuple[mir.Const | None, ...],
) -> mir.MirBody:
    """Seed agreed parameter bytes at procedure entry for ordinary SCCP."""
    known = tuple(
        (ref, constant)
        for ref, constant in zip(parameters, constants, strict=False)
        if constant is not None and constant.width == ref.width
    )
    added = tuple(one for one in known if one not in body.initial)
    return replace(body, initial=(*body.initial, *added)) if added else body


def constant_returns(bodies: dict[str, mir.MirBody]) -> Returns:
    """The common integer tuple produced by every return of each body.

    Absence is the conservative answer for void, floating, mixed-width or
    disagreeing returns.  Values are read from SCCP's fixed point, so copies,
    promoted locals, phis and folded expressions need no special cases here.
    """
    out: Returns = {}
    for name, body in bodies.items():
        facts = consts.known(body)
        returned = []
        complete = True
        for block in body.blocks:
            for op in block.ops:
                if op.kind is not mir.Kind.RETURN:
                    continue
                values = []
                for arg in op.args:
                    if isinstance(arg, mir.Const):
                        values.append(mir.Const(consts.masked(arg.n, arg.width), arg.width))
                    elif (
                        isinstance(arg, mir.Held)
                        and (fact := facts.get(arg.value)) is not None
                        and fact.width >= arg.width
                    ):
                        values.append(mir.Const(consts.masked(fact.n, arg.width), arg.width))
                    else:
                        complete = False
                        break
                if not complete or not values:
                    break
                returned.append(tuple(values))
            if not complete:
                break
        if complete and returned and len(set(returned)) == 1:
            out[name] = returned[0]
    return out


def propagate_returns(
    body: mir.MirBody,
    calls: dict[int, str],
    returns: Returns,
    done: frozenset[int] = frozenset(),
) -> tuple[mir.MirBody, frozenset[int]]:
    """Define a direct call's known result from its module-level summary.

    Fresh values receive the physical call result.  Constant copies define
    the original SSA names immediately afterwards, allowing the ordinary
    body pipeline to propagate through phis and fold consumers.  Unmodelled
    extra results (for example DX after a 16-bit C result in AX) stay intact.
    """
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0)
    variable = max((value.variable for value in values), default=0)
    completed = set(done)
    changed = False
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            known = returns.get(calls.get(op.at, "")) if op.kind is mir.Kind.CALL and op.at not in completed else None
            integer_results = tuple(result for result in op.results if isinstance(result, mir.Held))
            if known is None or not known or len(known) > len(integer_results):
                ops.append(op)
                continue
            pairs = tuple(zip(known, integer_results, strict=False))
            if any(constant.width != result.width for constant, result in pairs):
                ops.append(op)
                continue

            replacements = {}
            copies = []
            for constant, result in pairs:
                serial += 1
                variable += 1
                fresh = mir.Value(serial, op.at, variable=variable, version=1)
                replacements[result.value] = fresh
                copies.append(
                    mir.Op(
                        op.at,
                        ir.Operation.NOTHING,
                        "",
                        (result.value,),
                        (),
                        kind=mir.Kind.COPY,
                        args=(constant,),
                        results=(result,),
                        symbol=False,
                    )
                )
            results = tuple(
                mir.Held(replacements.get(result.value, result.value), result.width)
                if isinstance(result, mir.Held)
                else result
                for result in op.results
            )
            defines = tuple(replacements.get(value, value) for value in op.defines)
            ops.append(replace(op, results=results, defines=defines, raised=None))
            ops.extend(copies)
            completed.add(op.at)
            changed = True
        blocks.append(replace(block, ops=tuple(ops)))
    made = replace(body, blocks=tuple(blocks)) if changed else body
    problems = mir.verify(made)
    if problems:
        raise ValueError(f"interprocedural return propagation broke SSA: {problems[:3]}")
    return made, frozenset(completed)


def _acyclic_returning(body: mir.MirBody) -> bool:
    """Whether every CFG path ends in RETURN without revisiting a block."""
    blocks = {block.at: block for block in body.blocks}
    visiting, visited = set(), set()

    def visit(at: int) -> bool:
        if at in visiting or at not in blocks:
            return False
        if at in visited:
            return True
        visiting.add(at)
        block = blocks[at]
        if block.succ:
            okay = all(visit(one) for one in block.succ)
        else:
            okay = bool(block.ops) and block.ops[-1].kind is mir.Kind.RETURN
        visiting.remove(at)
        if okay:
            visited.add(at)
        return okay

    return visit(body.entry)


def _local_effects(body: mir.MirBody, calls: dict[int, str], pure: frozenset[str]) -> bool:
    if not _acyclic_returning(body):
        return False
    for block in body.blocks:
        for op in block.ops:
            if op.barrier or op.kind in _MAY_TRAP | _FLOATING | {mir.Kind.ESCAPE, mir.Kind.OPAQUE, mir.Kind.FILL}:
                return False
            if op.kind is mir.Kind.CALL:
                if calls.get(op.at) not in pure:
                    return False
                continue
            refs = (*op.loads, *op.stores)
            if any(ref.space not in (Space.FRAME, Space.STACK) for ref in refs):
                return False
    return True


def pure_procedures(procedures: dict[str, tuple[mir.MirBody, dict[int, str]]]) -> frozenset[str]:
    """Direct procedures with no observable effects and guaranteed return.

    The least fixed point admits an acyclic call chain once all its callees
    are admitted.  Recursive SCCs remain conservative because removing one
    would otherwise remove possible nontermination.
    """
    pure: frozenset[str] = frozenset()
    while True:
        made = pure | frozenset(name for name, (body, calls) in procedures.items() if _local_effects(body, calls, pure))
        if made == pure:
            return pure
        pure = made


def readonly_procedures(procedures: dict[str, tuple[mir.MirBody, dict[int, str]]]) -> frozenset[str]:
    """Acyclic user bodies whose unused calls have no observable effect.

    This is intentionally broader than ``pure_procedures``: an ordinary,
    direct read of this module's static data is not observable in C when its
    result is unused.  It remains narrower than a general no-fault proof:
    pointer-based, far/externally selected, volatile and floating reads stay
    out, as do all non-frame writes.  Callers may use this fact only to erase
    a dead result; it is not an inlining or alias-preservation permission.
    """
    readonly: frozenset[str] = frozenset()
    while True:
        made = readonly | frozenset(
            name for name, (body, calls) in procedures.items() if _readonly_effects(body, calls, readonly)
        )
        if made == readonly:
            return readonly
        readonly = made


def _readonly_effects(body: mir.MirBody, calls: dict[int, str], readonly: frozenset[str]) -> bool:
    if not _acyclic_returning(body):
        return False
    local = frozenset({Space.FRAME, Space.STACK})
    for block in body.blocks:
        for op in block.ops:
            if op.barrier or op.kind in _MAY_TRAP | _FLOATING | {mir.Kind.ESCAPE, mir.Kind.OPAQUE, mir.Kind.FILL}:
                return False
            if op.kind is mir.Kind.CALL:
                if calls.get(op.at) not in readonly:
                    return False
                continue
            if any(ref.volatile for ref in (*op.loads, *op.stores)):
                return False
            # Internal frame writes disappear with the call.  Any write to a
            # nonlocal object remains observable, even if it is otherwise an
            # exact direct reference.
            if any(ref.space not in local for ref in op.stores):
                return False
            for ref in op.loads:
                if ref.space in local:
                    continue
                # A direct near static data reference is guaranteed to name
                # this module's mapped data.  Do not infer the same from an
                # arbitrary pointer, external selector or far access.
                if ref.space is not Space.SEGMENT or ref.base is not None or ref.segment is not None:
                    return False
    return True


def noreturn_procedures(
    procedures: dict[str, tuple[mir.MirBody, dict[int, str]]], eligible: frozenset[str]
) -> frozenset[str]:
    """Direct private procedures that cannot reach a normal return.

    This is the named-body spelling of the shared MIR control proof used by
    the object path.  Start with all private candidates and remove a body
    only when a normal return remains reachable.  This greatest fixed point
    proves a closed recursive SCC terminal when every member stops through a
    member of that same SCC; an unknown, external, public, or returning edge
    removes its owner instead of being assumed terminal.
    """
    from qbopt.analysis import noreturn

    proven = frozenset(name for name in eligible if name in procedures)
    while True:
        found = frozenset(
            name
            for name, (body, calls) in procedures.items()
            if name in eligible
            if noreturn._cannot_return(body, frozenset(at for at, target in calls.items() if target in proven))
        )
        if found == proven:
            return proven
        proven = found


def terminal_calls(body: mir.MirBody, calls: dict[int, str], noreturn: frozenset[str]) -> mir.MirBody:
    """Apply the shared MIR terminal-call cleanup to named direct C calls."""
    from qbopt.analysis import noreturn as control

    return control.after_terminal_calls(body, terminal_sites(calls, noreturn))


def terminal_sites(calls: dict[int, str], noreturn: frozenset[str]) -> frozenset[int]:
    """Direct call sites whose named callee cannot return."""
    return frozenset(at for at, target in calls.items() if target in noreturn)


def argument_sites(body: mir.MirBody, contracts: dict[int, object]) -> dict[int, frozenset[int]]:
    """Associate each call with the exact stack ARG operations that feed it."""
    out = {}
    for block in body.blocks:
        for index, op in enumerate(block.ops):
            if op.kind is not mir.Kind.CALL or op.at not in contracts:
                continue
            contract = contracts[op.at]
            cleanup = getattr(contract, "cleanup", None)
            if cleanup is None:
                continue
            needed = cleanup + getattr(contract, "caller_cleanup", 0)
            if needed == 0:
                out[op.at] = frozenset()
                continue
            found, total = set(), 0
            for prior in reversed(block.ops[:index]):
                if prior.kind is mir.Kind.CALL:
                    break
                if prior.kind is not mir.Kind.ARG or len(prior.args) != 1:
                    continue
                width = getattr(prior.args[0], "width", 0)
                if width <= 0:
                    break
                found.add(prior.at)
                total += width
                if total >= needed:
                    break
            if total == needed:
                out[op.at] = frozenset(found)
    return out


def remove_dead_pure_calls(
    body: mir.MirBody,
    calls: dict[int, str],
    pure: frozenset[str],
    arguments: dict[int, frozenset[int]],
) -> mir.MirBody:
    """Remove effect-free calls whose result no operation still reads."""
    used = {
        value
        for block in body.blocks
        for value in (
            *(value for phi in block.phis for value in phi.incoming.values()),
            *(value for op in block.ops for value in mir.consumed(op)),
        )
    }
    removed = {
        op.at
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL
        and calls.get(op.at) in pure
        and not any(value in used for value in op.defines)
        and op.at in arguments
    }
    if not removed:
        return body
    discarded_arguments = set().union(*(arguments[at] for at in removed))
    made = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    op
                    for op in block.ops
                    if not (
                        (op.kind is mir.Kind.CALL and op.at in removed)
                        or (op.kind is mir.Kind.ARG and op.at in discarded_arguments)
                    )
                ),
            )
            for block in body.blocks
        ),
    )
    problems = mir.verify(made)
    if problems:
        raise ValueError(f"pure call removal broke SSA: {problems[:3]}")
    return made
