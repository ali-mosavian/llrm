"""Sparse value propagation with distinct pending and overdefined states.

All CFG edges participate. Executable-edge discovery belongs to the later
conditional solver; this establishes the value lattice without guessing which
paths run or importing LLVM's undef semantics for BASIC runtime inputs.
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


def propagated(body: mir.MirBody, seeds: dict) -> dict:
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
    pending = deque(recipes)
    queued = set(recipes)
    supported = {*consts.ARITH, *consts.UNARY, mir.Kind.COPY, mir.Kind.EXTRACT,
                 mir.Kind.SIGN_EXTEND, mir.Kind.CONCAT, mir.Kind.SMULHI}
    while pending:
        value = pending.popleft()
        queued.remove(value)
        if value in seeds or states[value] is State.OVERDEFINED:
            continue
        recipe = recipes[value]
        candidate = State.PENDING
        if isinstance(recipe, mir.Phi):
            for incoming in recipe.incoming.values():
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
        for consumer in consumers[value] - queued:
            pending.append(consumer)
            queued.add(consumer)
    return {value: state for value, state in states.items() if isinstance(state, consts.Known)}
