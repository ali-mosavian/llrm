"""Selective whole-module MIR inlining.

Inlining is a CFG operation, not a call peephole: split the caller at the
call, clone the callee's blocks, bind formal parameter loads to the actual
SSA values, and join every return back to the continuation.  The ordinary
body pipeline then simplifies the result.

The initial policy covered leaf procedures called once.  It also admits a
straight-line private leaf at every direct call site when the target-priced
call work exceeds the semantic work duplicated by cloning.  In both cases
the ordinary body pipeline simplifies the result; MIR chooses from semantic
costs and never sees opcodes or registers.
"""

from collections import Counter
from dataclasses import replace
from dataclasses import dataclass

from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import ssa
from qbopt.optimize import edges

_FORBIDDEN = frozenset(
    {
        mir.Kind.CALL,
        mir.Kind.ARG,
        mir.Kind.ESCAPE,
        mir.Kind.OPAQUE,
        mir.Kind.FILL,
        mir.Kind.DIV,
        mir.Kind.REM,
        mir.Kind.DIVMOD,
        mir.Kind.UDIVMOD,
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


@dataclass(frozen=True, slots=True)
class Candidate:
    body: mir.MirBody
    parameters: tuple[mir.MemRef, ...]


def candidates(
    bodies: dict[str, mir.MirBody],
    parameters: dict[str, tuple[mir.MemRef, ...]],
    calls: Counter[str],
    private: frozenset[str],
    pure: frozenset[str],
    call_cost: int,
) -> dict[str, Candidate]:
    """Private pure leaves worth moving into their direct callers.

    A single-use body disappears after expansion, so it only has to fit the
    normal CFG budget.  A repeated body duplicates its semantic work once per
    additional caller.  Admit that only for a straight-line leaf, and only
    when the profile's total direct-call cost is greater than the duplicate
    work.  This lets a short arithmetic helper disappear at every site while
    keeping branchy or code-growing helpers out of the allocator's region.
    """
    budget = max(6, min(24, call_cost // 2))
    out = {}
    for name in private & pure:
        body = bodies[name]
        parms = parameters[name]
        semantic = sum(
            op.kind not in (mir.Kind.NOTHING, mir.Kind.JUMP, mir.Kind.RETURN)
            for block in body.blocks
            for op in block.ops
        )
        repeated = semantic * (calls[name] - 1)
        profitable = calls[name] == 1 or (_straight(body) and repeated < calls[name] * call_cost)
        if calls[name] and semantic <= budget and profitable and _leaf(body, parms):
            out[name] = Candidate(body, parms)
    return out


def constant_sites(
    bodies: dict[str, mir.MirBody],
    parameters: dict[str, tuple[mir.MemRef, ...]],
    calls: dict[int, str],
    constants: dict[int, tuple[mir.Const | None, ...]],
    private: frozenset[str],
    pure: frozenset[str],
    call_cost: int,
) -> dict[int, Candidate]:
    """Private pure leaves worth cloning at one constant direct-call site.

    Whole-body parameter specialization needs every caller to agree.  This
    narrower policy instead admits a call whose known actual exposes local
    SCCP after the normal MIR clone.  The original body remains for dynamic
    callers, so no source-level calling convention or OMF symbol changes.
    As with repeated-leaf inlining, the target profile must price the call
    above the cloned semantic work.
    """
    budget = max(6, min(24, call_cost // 2))
    out = {}
    for at, name in calls.items():
        known = constants.get(at, ())
        if not any(value is not None for value in known):
            continue
        if name not in private or name not in pure or name not in bodies:
            continue
        body, parms = bodies[name], parameters[name]
        semantic = sum(
            op.kind not in (mir.Kind.NOTHING, mir.Kind.JUMP, mir.Kind.RETURN)
            for block in body.blocks
            for op in block.ops
        )
        if semantic < call_cost and semantic <= budget and _leaf(body, parms):
            out[at] = Candidate(body, parms)
    return out


def _straight(body: mir.MirBody) -> bool:
    """Whether cloning the body duplicates no control-flow structure."""
    return len(body.blocks) == 1 and not body.blocks[0].phis and not body.blocks[0].succ


def _parameter(op: mir.Op, parameters: tuple[mir.MemRef, ...]) -> int | None:
    if (
        op.kind is not mir.Kind.LOAD
        or len(op.loads) != 1
        or len(op.results) != 1
        or not isinstance(op.results[0], mir.Held)
    ):
        return None
    found = [index for index, ref in enumerate(parameters) if mir.same_bytes(op.loads[0], ref)]
    return found[0] if len(found) == 1 else None


def _leaf(body: mir.MirBody, parameters: tuple[mir.MemRef, ...]) -> bool:
    """Whether a pure body's remaining memory is only its formal values."""
    if not body.sealed or not body.blocks:
        return False
    returned = []
    for block in body.blocks:
        for op in block.ops:
            if op.barrier or op.kind in _FORBIDDEN or op.stores or op.array is not None:
                return False
            if op.loads and _parameter(op, parameters) is None:
                return False
            if op.kind is mir.Kind.RETURN:
                returned.append(tuple(getattr(arg, "width", 0) for arg in op.args))
    return bool(returned) and len(set(returned)) == 1 and all(returned[0])


def call_counts(bodies: dict[str, mir.MirBody], calls: dict[str, dict[int, str]]) -> Counter[str]:
    """Surviving direct call counts, never stale entries in a source side table."""
    return Counter(
        calls[name][op.at]
        for name, body in bodies.items()
        for block in body.blocks
        for op in block.ops
        if op.kind is mir.Kind.CALL and op.at in calls[name]
    )


def expanded(
    body: mir.MirBody,
    calls: dict[int, str],
    arguments: dict[int, frozenset[int]],
    available: dict[str, Candidate],
    constant: dict[int, Candidate] | None = None,
) -> mir.MirBody:
    """Inline the first legal call site in ``body``, or return it unchanged."""
    constant = constant or {}
    used = {
        value
        for block in body.blocks
        for value in (
            *(value for phi in block.phis for value in phi.incoming.values()),
            *(value for op in block.ops for value in mir.consumed(op)),
        )
    }
    for block in body.blocks:
        for index, call in enumerate(block.ops):
            candidate = constant.get(call.at, available.get(calls.get(call.at, "")))
            if call.kind is not mir.Kind.CALL or candidate is None or call.at not in arguments:
                continue
            made = _at(body, block, index, call, arguments[call.at], candidate, used)
            if made is not None:
                problems = mir.verify(made)
                if problems:
                    raise ValueError(f"MIR inlining broke SSA: {problems[:3]}")
                return made
    return body


def _at(
    body: mir.MirBody,
    caller: mir.MirBlock,
    call_index: int,
    call: mir.Op,
    argument_sites: frozenset[int],
    candidate: Candidate,
    used: set[mir.Value],
) -> mir.MirBody | None:
    callee, parameters = candidate.body, candidate.parameters
    selected = [
        (index, op)
        for index, op in enumerate(caller.ops[:call_index])
        if op.kind is mir.Kind.ARG and op.at in argument_sites
    ]
    # cdecl pushes the last source argument first.
    actuals = [op.args[0] for _, op in reversed(selected) if len(op.args) == 1]
    if len(selected) != len(parameters) or len(actuals) != len(parameters):
        return None

    returns = [op for block in callee.blocks for op in block.ops if op.kind is mir.Kind.RETURN]
    if not returns or len({len(op.args) for op in returns}) != 1:
        return None
    arity = len(returns[0].args)
    results = tuple(result for result in call.results if isinstance(result, mir.Held))
    if arity > len(results):
        return None
    if any(result.value in used for result in results[arity:]):
        return None
    if any(
        not isinstance(arg, mir.Held) or arg.width != result.width
        for returned in returns
        for arg, result in zip(returned.args, results[:arity], strict=False)
    ):
        return None
    semantic = {result.value for result in results[:arity]}
    if any(value in used and value not in semantic for value in call.defines):
        return None

    values = tuple(ssa.values(body))
    callee_values = tuple(ssa.values(callee))
    # The two independently raised bodies both number values from one.  A
    # materialized actual must consequently be fresh against the callee's
    # *source* IDs too: substitution keys are IDs, and colliding with one
    # would make a constant binding look like an unrelated cloned definition.
    next_id = max((value.id for value in (*values, *callee_values)), default=0) + 1
    next_variable = max((value.variable for value in (*values, *callee_values)), default=0) + 1
    versions = Counter()
    for value in values:
        versions[value.variable] = max(versions[value.variable], value.version)

    def fresh(*, flags: bool = False, variable: int | None = None) -> mir.Value:
        nonlocal next_id, next_variable
        if variable is None:
            variable = next_variable
            next_variable += 1
        versions[variable] += 1
        value = mir.Value(next_id, call.at, flags=flags, variable=variable, version=versions[variable])
        next_id += 1
        return value

    pre_ops = [op for index, op in enumerate(caller.ops[:call_index]) if index not in {one for one, _ in selected}]
    materialized: list[mir.Op] = []
    parameter_values: dict[int, mir.Value] = {}
    parameter_widths: dict[int, int] = {}
    for callee_block in callee.blocks:
        for op in callee_block.ops:
            number = _parameter(op, parameters)
            if number is not None:
                parameter_widths[number] = op.results[0].width
    if any(number not in parameter_widths for number in range(len(parameters))):
        # An unused formal needs no binding, but its argument setup is still removable.
        parameter_widths.update(
            (number, parameters[number].width) for number in range(len(parameters)) if number not in parameter_widths
        )

    widths = (parameter_widths[number] for number in range(len(parameters)))
    for number, (actual, width) in enumerate(zip(actuals, widths, strict=True)):
        actual_width = getattr(actual, "width", 0)
        if actual_width < width or isinstance(actual, (mir.Cell, mir.Opaque)):
            return None
        if isinstance(actual, mir.Held):
            parameter_values[number] = actual.value
            continue
        if isinstance(actual, mir.Const) and actual.width != width:
            actual = mir.Const(actual.n, width)
        value = fresh()
        parameter_values[number] = value
        materialized.append(_copy(call.at, actual, mir.Held(value, width), value))

    variable_map: dict[int, int] = {}
    swap: dict[int, mir.Value] = {}
    for callee_block in callee.blocks:
        for op in callee_block.ops:
            number = _parameter(op, parameters)
            if number is not None:
                swap[op.results[0].value.id] = parameter_values[number]
    formal_values = frozenset(swap)
    for value in callee_values:
        if value.id in swap:
            continue
        variable = variable_map.setdefault(value.variable, next_variable)
        if variable == next_variable:
            next_variable += 1
        swap[value.id] = fresh(flags=value.flags, variable=variable)

    first_label = edges.fresh(body)
    labels = {block.at: first_label + number for number, block in enumerate(callee.blocks)}
    continuation = first_label + len(callee.blocks)
    return_edges: list[tuple[int, tuple[mir.Held, ...]]] = []
    cloned = []
    for callee_block in callee.blocks:
        phis = tuple(
            mir.Phi(
                swap[phi.result.id],
                {labels[source]: swap[value.id] for source, value in phi.incoming.items()},
            )
            for phi in callee_block.phis
        )
        ops = []
        returned: tuple[mir.Held, ...] | None = None
        for op in callee_block.ops:
            if _parameter(op, parameters) is not None:
                continue
            read = ssa.substituted(op, swap)
            if op.kind is mir.Kind.RETURN:
                if not all(isinstance(arg, mir.Held) for arg in read.args):
                    return None
                returned = tuple(read.args)
                continue
            ops.append(
                replace(
                    read,
                    at=call.at,
                    defines=tuple(swap[value.id] for value in op.defines),
                    results=tuple(
                        replace(result, value=swap[result.value.id]) if isinstance(result, mir.Held) else result
                        for result in read.results
                    ),
                    source_backed=False,
                    id=None,
                    raised=None,
                    absorbed=(),
                    symbol=False,
                    target=labels.get(op.target, op.target),
                    cases=tuple((number, labels.get(target, target)) for number, target in op.cases),
                )
            )
        succ = tuple(labels.get(target, target) for target in callee_block.succ)
        if returned is not None:
            if succ:
                return None
            return_edges.append((labels[callee_block.at], returned))
            succ = (continuation,)
            ops.append(_jump(call.at, continuation))
        cloned.append(mir.MirBlock(labels[callee_block.at], phis, tuple(ops), succ))

    if not return_edges:
        return None
    result_phis = []
    if len(return_edges) == 1:
        return_at, returned = return_edges[0]
        clone = next(one for one in cloned if one.at == return_at)
        copies = tuple(
            _copy(call.at, value, result, result.value) for value, result in zip(returned, results[:arity], strict=True)
        )
        cloned[cloned.index(clone)] = replace(clone, ops=(*clone.ops[:-1], *copies, clone.ops[-1]))
    else:
        by_block: dict[int, list[mir.Op]] = {at: [] for at, _ in return_edges}
        incoming: list[dict[int, mir.Value]] = [dict() for _ in range(arity)]
        for at, returned in return_edges:
            for number, (value, result) in enumerate(zip(returned, results[:arity], strict=True)):
                edge_value = fresh(variable=result.value.variable)
                by_block[at].append(_copy(call.at, value, mir.Held(edge_value, result.width), edge_value))
                incoming[number][at] = edge_value
        for number, result in enumerate(results[:arity]):
            result_phis.append(mir.Phi(result.value, incoming[number]))
        cloned = [
            replace(block, ops=(*block.ops[:-1], *by_block.get(block.at, ()), block.ops[-1]))
            if block.at in by_block
            else block
            for block in cloned
        ]

    pre = replace(
        caller,
        ops=(*pre_ops, *materialized, _jump(call.at, labels[callee.entry])),
        succ=(labels[callee.entry],),
    )
    after = caller.ops[call_index + 1 :]
    continued = mir.MirBlock(continuation, tuple(result_phis), after, caller.succ)
    blocks = []
    for block in body.blocks:
        if block.at == caller.at:
            blocks.extend((pre, *cloned, continued))
            continue
        if block.at in caller.succ:
            block = replace(
                block,
                phis=tuple(
                    replace(
                        phi,
                        incoming={continuation if at == caller.at else at: value for at, value in phi.incoming.items()},
                    )
                    for phi in block.phis
                ),
            )
        blocks.append(block)

    pointer_values = set(body.pointer_values)
    pointer_values.update(swap[value.id] for value in callee.pointer_values if value.id in swap)
    pointer_seeds = dict(body.pointer_seeds)
    pointer_seeds.update(
        (swap[value.id], seed)
        for value, seed in callee.pointer_seeds.items()
        if value.id in swap and value.id not in formal_values and swap[value.id] not in pointer_seeds
    )
    return replace(
        body,
        blocks=tuple(blocks),
        cloned=True,
        pointer_values=frozenset(pointer_values),
        pointer_seeds=pointer_seeds,
    )


def _copy(at: int, source: mir.Arg, result: mir.Held, value: mir.Value) -> mir.Op:
    return mir.Op(
        at,
        ir.Operation.NOTHING,
        "",
        (value,),
        (source.value,) if isinstance(source, mir.Held) else (),
        kind=mir.Kind.COPY,
        args=(source,),
        results=(result,),
        symbol=isinstance(source, mir.Symbol),
        reads_complete=True,
    )


def _jump(at: int, target: int) -> mir.Op:
    return mir.Op(
        at,
        ir.Operation.NOTHING,
        "",
        (),
        (),
        kind=mir.Kind.JUMP,
        target=target,
        symbol=False,
        reads_complete=True,
    )
