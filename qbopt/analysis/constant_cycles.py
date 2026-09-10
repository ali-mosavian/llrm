"""Sparse value propagation with distinct pending and overdefined states.

An optional successor evaluator discovers executable edges. Pending values are
never interpreted as LLVM undef: unresolved reachable values become overdefined
before the final result, and unresolved branches retain every successor.
"""

from collections import defaultdict, deque
from enum import Enum, auto

from qbopt.analysis import consts
from qbopt.model import mir


class State(Enum):
    PENDING = auto()
    OVERDEFINED = auto()


def _meet(first, second):
    if first is State.PENDING:
        return second
    if second is State.PENDING or first == second:
        return first
    return State.OVERDEFINED


def propagated(body: mir.MirBody, seeds: dict, successors=None) -> dict:
    recipes = {phi.result: phi for block in body.blocks for phi in block.phis}
    recipes.update({value: op for block in body.blocks for op in block.ops
                    if (value := consts._defined(op)) is not None})
    states = {value: seeds.get(value, State.PENDING) for value in recipes}
    states.update(seeds)
    consumers = defaultdict(set)
    for value, recipe in recipes.items():
        inputs = (recipe.incoming.values() if isinstance(recipe, mir.Phi)
                  else (arg.value for arg in recipe.args if isinstance(arg, mir.Held)))
        for incoming in inputs:
            consumers[incoming].add(value)
    blocks = {block.at: block for block in body.blocks}
    owners = {phi.result: block.at for block in body.blocks for phi in block.phis}
    owners.update({value: block.at for block in body.blocks for op in block.ops
                   if (value := consts._defined(op)) is not None})
    live = set(blocks) if successors is None else {body.entry}
    edges = set()
    pending = deque(value for value in recipes if owners[value] in live)
    queued = set(pending)

    def enqueue(values):
        for value in values:
            if value not in queued and owners[value] in live:
                pending.append(value)
                queued.add(value)

    def activate(source, target):
        if target not in blocks or (source, target) in edges:
            return False
        edges.add((source, target))
        if target not in live:
            live.add(target)
            enqueue(value for value in recipes if owners[value] == target)
        else:
            enqueue(phi.result for phi in blocks[target].phis)
        return True
    supported = {*consts.ARITH, *consts.UNARY, mir.Kind.COPY, mir.Kind.EXTRACT,
                 mir.Kind.SIGN_EXTEND, mir.Kind.CONCAT, mir.Kind.SMULHI}
    while True:
        if not pending:
            facts = {value: state for value, state in states.items() if isinstance(state, consts.Known)}
            changed = False
            deferred = []
            if successors is not None:
                for at in tuple(live):
                    selected = successors(blocks[at], facts, states)
                    if selected is None:
                        deferred.append(at)
                        continue
                    for target in selected:
                        changed |= activate(at, target)
            if pending or changed:
                continue
            unresolved = [value for value in recipes
                          if owners[value] in live and states[value] is State.PENDING]
            for value in unresolved:
                states[value] = State.OVERDEFINED
                enqueue(consumers[value])
            if unresolved:
                continue
            for at in deferred:
                for target in blocks[at].succ:
                    changed |= activate(at, target)
            if changed or pending:
                continue
            return facts
        value = pending.popleft()
        queued.remove(value)
        if value in seeds or states[value] is State.OVERDEFINED:
            continue
        recipe = recipes[value]
        candidate = State.PENDING
        if isinstance(recipe, mir.Phi):
            if successors is not None and owners[value] == body.entry:
                candidate = State.OVERDEFINED  # entry also executes before any backedge
            for predecessor, incoming in recipe.incoming.items():
                if successors is not None and (predecessor, owners[value]) not in edges:
                    continue
                candidate = _meet(candidate, states.get(incoming, State.OVERDEFINED))
        elif recipe.kind not in supported or recipe.loads or recipe.stores or recipe.barrier:
            candidate = State.OVERDEFINED
        else:
            facts = {arg.value: state for arg in recipe.args if isinstance(arg, mir.Held)
                     and isinstance(state := states.get(arg.value), consts.Known)}
            candidate = consts._result(recipe, facts)
            if candidate is None:
                inputs = [states.get(arg.value, State.OVERDEFINED)
                          for arg in recipe.args if isinstance(arg, mir.Held)]
                candidate = (State.PENDING if State.PENDING in inputs and State.OVERDEFINED not in inputs
                             else State.OVERDEFINED)
        merged = _meet(states[value], candidate)
        if merged == states[value]:
            continue
        states[value] = merged
        enqueue(consumers[value])
