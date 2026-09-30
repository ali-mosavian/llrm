"""
What an inner loop should cost, derived from the case and the target's own
tables, with no compiler involved.

Machine facts, from llrm's target description:
- REGISTERS, CALL_REGISTERS: `register_capacity` and `call_register_capacity`
  (crates/backend/llrm-core/src/backend/cpu.rs), the same for 386 to Core.
- PARTNERS: the native address form's `partners`
  (crates/target/llrm-x86-code16/src/target.rs): bx or bp with si or di, so
  one index register pairs with at most two bases.
- SEGMENTS: ES, FS and GS, the segment registers a 386 or later has free.

A use of a counter is affine when its value is `start + t * coefficient` on
iteration t, the coefficient a constant or a constant times one invariant.
An address's coefficient is in bytes. Uses with one coefficient form a class;
a class needs one induction variable for every PARTNERS register-held bases
(at least one), since a constant base goes in a displacement and a frame base
is bp. The exit test reuses one of them. The loop fits when those, its
register bases, its accumulators and invariants and one temporary fit in
REGISTERS, its far bases in SEGMENTS, and it calls nothing. Past that only
correctness and the reference compilers' bar apply.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from spec import (
    Case, Float, Ptr, Struct, Const, Var, Bin, Neg, Cast, Cmp, Not, Logic, Load, AddrOf, PtrAdd, Deref, Len, CallE,
    Assign, For, While, DoWhile, If, CallS, Break, Return, Continue, walk, exprs, stmt_exprs,
)

REGISTERS = 6
CALL_REGISTERS = 2
PARTNERS = 2
SEGMENTS = 3


@dataclass(frozen=True)
class Want:
    """One inner loop's bound. `ivs`/`invariant_loads` None: no bound.
    `shape`: the reference form applies (one counter from -stride*n up to
    zero, the branch on its step, bases biased once before the loop)."""

    ivs: int | None
    invariant_loads: int | None
    fits: bool
    shape: bool
    classes: tuple = ()
    why: str = ""


class NotAffine(Exception):
    pass


def _coef_add(a: dict, b: dict, sign: int = 1) -> dict:
    out = dict(a)
    for key, value in b.items():
        out[key] = out.get(key, 0) + sign * value
    return {k: v for k, v in out.items() if v}


def _scale(a: dict, k: int) -> dict:
    return {key: value * k for key, value in a.items() if value * k}


def inner_loops(stmts) -> list:
    return [s for s in walk(stmts) if isinstance(s, (For, While, DoWhile))
            and not any(isinstance(x, (For, While, DoWhile)) for x in walk(s.body))]


class Loop:
    """One innermost loop of a case, as the derivation sees it."""

    def __init__(self, case: Case, loop, lang: str):
        self.case, self.loop, self.lang = case, loop, lang
        self.types = case.types()
        self.stmts = walk(loop.body)
        assigned = [s for s in self.stmts if isinstance(s, Assign) and isinstance(s.place, Var)]
        self.assigned = {s.place.name for s in assigned}
        if isinstance(loop, For):
            self.assigned.add(loop.var)
        # iteration-affine scalars: one unconditional `v = v + c` a trip
        self.steps: dict[str, dict] = {}
        top = [s for s in loop.body if isinstance(s, Assign) and isinstance(s.place, Var)]
        for name in self.assigned:
            writes = [s for s in assigned if s.place.name == name]
            if isinstance(loop, For) and name == loop.var:
                if not writes:
                    try:
                        self.steps[name] = self._invariant_coef(loop.step)
                    except NotAffine:
                        pass
                continue
            if len(writes) == 1 and writes[0] in top:
                step = self._self_step(name, writes[0].value)
                if step is not None:
                    self.steps[name] = step

    def _self_step(self, name: str, value) -> dict | None:
        if isinstance(value, (Bin, PtrAdd)):
            a, b, op = (value.a, value.b, value.op) if isinstance(value, Bin) else (value.ptr, value.by, "+")
            if a == Var(name) and op in "+-":
                try:
                    step = self._invariant_coef(b)
                    return step if op == "+" else _scale(step, -1)
                except NotAffine:
                    return None
        return None

    def _invariant_coef(self, e) -> dict:
        """An invariant step's value as a coefficient: {1: k} or {name: k}."""
        if isinstance(e, Const):
            return {1: e.value} if e.value else {}
        if isinstance(e, Var) and e.name not in self.assigned:
            return {e.name: 1}
        if isinstance(e, Neg):
            return _scale(self._invariant_coef(e.a), -1)
        if isinstance(e, Cast):
            return self._invariant_coef(e.a)
        if isinstance(e, Bin) and e.op == "*":
            a, b = self._invariant_coef(e.a), self._invariant_coef(e.b)
            if set(a) == {1}:
                return _scale(b, a[1])
            if set(b) == {1}:
                return _scale(a, b[1])
        raise NotAffine(e)

    def coef(self, e) -> dict:
        """How much `e` changes a trip, or NotAffine."""
        if isinstance(e, (Const, Len)):
            return {}
        if isinstance(e, Var):
            if e.name in self.steps:
                return self.steps[e.name]
            if e.name in self.assigned:
                raise NotAffine(e)
            return {}
        if isinstance(e, Bin):
            if e.op in "+-" and len(e.op) == 1:
                return _coef_add(self.coef(e.a), self.coef(e.b), 1 if e.op == "+" else -1)
            if e.op == "*":
                a, b = self.coef(e.a), self.coef(e.b)
                if a and b:
                    raise NotAffine(e)
                if not a and not b:
                    return {}
                varying, other = (a, e.b) if a else (b, e.a)
                factor = self._invariant_coef(other)
                if set(factor) == {1}:
                    return _scale(varying, factor[1])
                if set(varying) == {1} and len(factor) == 1:
                    name = next(iter(factor))
                    return {name: varying[1] * factor[name]}
                raise NotAffine(e)
            if e.op == "<<" and isinstance(e.b, Const):
                return _scale(self.coef(e.a), 1 << e.b.value)
            if not self.coef(e.a) and not self.coef(e.b):
                return {}
            raise NotAffine(e)
        if isinstance(e, Neg):
            return _scale(self.coef(e.a), -1)
        if isinstance(e, Cast):
            return self.coef(e.a)
        if isinstance(e, (Load, Deref)):
            if self._invariant_load(e):
                return {}
            raise NotAffine(e)
        if isinstance(e, AddrOf):
            return self.coef(e.index)
        if isinstance(e, PtrAdd):
            return _coef_add(self.coef(e.ptr), self.coef(e.by))
        raise NotAffine(e)

    def _invariant_load(self, e) -> bool:
        written = {s.place.array for s in self.stmts if isinstance(s, Assign) and isinstance(s.place, Load)}
        if isinstance(e, Load):
            try:
                return e.array not in written and all(not self.coef(one) for one in e.index) and not any(
                    isinstance(s, Assign) and isinstance(s.place, Deref) for s in self.stmts)
            except NotAffine:
                return False
        return False

    def element_offset(self, e: Load) -> dict:
        dims = self.case.array(e.array).dims
        total: dict = {}
        for d, index in enumerate(e.index):
            weight = 1
            for extent in dims[d + 1:]:
                weight *= extent
            total = _coef_add(total, _scale(self.coef(index), weight))
        return total

    def uses(self) -> list[tuple[str, tuple, str | None]]:
        """(kind, coefficient key, base) for each use of something that
        steps: kind address or value; base the array needing a register.
        Sets `address_parts` (the subexpressions of addresses) and
        `irregular` (accesses whose address is not affine)."""
        out = []
        address_parts = set()
        self.irregular = 0
        for s in [*self.stmts, self.loop]:
            for e in (stmt_exprs(s) if s is not self.loop else self._head()):
                if isinstance(e, Load):
                    try:
                        offset = self.element_offset(e)
                    except NotAffine:
                        self.irregular += 1
                        continue
                    address_parts.update(id(x) for one in e.index for x in exprs(one))
                    if not offset:
                        continue
                    array = self.case.array(e.array)
                    out.append(("address", self._key(_scale(offset, array.elem.size)), self._base(array)))
                    address_parts.update(id(x) for one in e.index for x in exprs(one))
                elif isinstance(e, Deref):
                    try:
                        offset = _coef_add(self.coef(e.ptr), self.coef(e.at))
                    except NotAffine:
                        self.irregular += 1
                        continue
                    kind = self.types.get(e.ptr.name) if isinstance(e.ptr, Var) else None
                    size = kind.elem.size if isinstance(kind, Ptr) else 1
                    arrays = {x.array for s2 in walk(self.case.body) if isinstance(s2, Assign) and s2.place == e.ptr
                              for x in exprs(s2.value) if isinstance(x, AddrOf)}
                    base = self._base(self.case.array(next(iter(arrays)))) if len(arrays) == 1 else "near"
                    if offset:
                        out.append(("address", self._key(_scale(offset, size)), base))
                    address_parts.update(id(x) for x in exprs(e.ptr))
                    address_parts.update(id(x) for x in exprs(e.at))
        for s in self.stmts:
            if isinstance(s, Assign) and isinstance(s.place, Var) and s.place.name in self.steps:
                continue  # an induction variable's own step
            for e in stmt_exprs(s):
                if isinstance(e, Var) and e.name in self.steps and id(e) not in address_parts:
                    if isinstance(self.types[e.name], Ptr):
                        continue
                    out.append(("value", self._key(self.steps[e.name]), None))
        self.address_parts = address_parts
        return out

    def _head(self) -> list:
        loop = self.loop
        if isinstance(loop, For):
            return exprs(loop.end)
        return exprs(loop.cond)

    @staticmethod
    def _key(coef: dict) -> tuple:
        return tuple(sorted(coef.items(), key=lambda kv: str(kv[0])))

    def _base(self, array) -> str | None:
        """The register a use of `array` needs for its base: none for a
        global (a displacement) or a local (bp), near or far otherwise."""
        if array.where in ("global", "local"):
            return None
        if self.lang == "c":
            return array.name + (":near" if array.ptr == "near" else ":far")
        return array.name + ":far"


def want(case: Case, lang: str) -> list[Want]:
    """One Want per innermost loop, in source order."""
    return [_want(case, loop, lang) for loop in inner_loops(case.body)]


def _want(case: Case, loop, lang: str) -> Want:
    view = Loop(case, loop, lang)
    uses = view.uses()
    classes: dict[tuple, set] = {}
    for kind, key, base in uses:
        classes.setdefault(key, set())
        if base:
            classes[key].add(base)
    if not classes:
        classes[((1, 1),)] = set()
    ivs = sum(max(1, -(-len(bases) // PARTNERS)) for bases in classes.values())
    bases = set().union(*classes.values())
    far = {b for b in bases if b.endswith(":far")}
    symbolic = sum(1 for key in classes if any(k != 1 for k, _ in key))
    accumulators = {
        s.place.name for s in view.stmts
        if isinstance(s, Assign) and isinstance(s.place, Var) and s.place.name not in view.steps
        and not isinstance(case.types()[s.place.name], Float)
    }
    # an invariant inside an address folds into the base before the loop
    invariants = {
        e.name for s in view.stmts for e in stmt_exprs(s)
        if isinstance(e, Var) and e.name not in view.assigned and not isinstance(case.types()[e.name], Ptr)
        and id(e) not in view.address_parts
    }
    calls = any(isinstance(s, CallS) for s in view.stmts) or any(
        isinstance(e, CallE) for s in view.stmts for e in stmt_exprs(s))
    memory = any(isinstance(e, (Load, Deref)) for s in view.stmts for e in stmt_exprs(s))
    live = ivs + len(bases) + len(accumulators) + len(invariants) + symbolic + int(memory)
    fits = live <= REGISTERS and len(far) <= SEGMENTS and not calls
    why = (f"{len(classes)} stride class(es) {sorted(classes, key=str)}, {len(bases)} register base(s), "
           f"{len(accumulators)} accumulator(s), {len(invariants)} invariant(s): {live} of {REGISTERS} registers"
           f"{', a call' if calls else ''}")
    # BASIC passes scalars by reference: a store may alias one, so it reloads.
    stores = any(isinstance(s, Assign) and isinstance(s.place, (Load, Deref)) for s in view.stmts)
    aliased = lang == "bas" and stores and any(name in dict(case.params) for name in invariants)
    counted = _counted(view)
    shape = (fits and len(classes) == 1 and counted and bool(uses) and not view.irregular
             and all(k == "address" for k, _, _ in uses))
    return Want(ivs if fits else None, (None if aliased else 0) if fits else None, fits, shape,
                tuple(sorted(map(str, classes))), why)


def _counted(view: Loop) -> bool:
    """The loop ends on its counter against an invariant, nothing else."""
    loop = view.loop
    if any(isinstance(s, (Break, Return, Continue)) for s in view.stmts):
        return False
    if isinstance(loop, For):
        try:
            return not view.coef(loop.end)
        except NotAffine:
            return False
    cond = loop.cond
    if isinstance(cond, Cmp):
        try:
            sides = [view.coef(cond.a), view.coef(cond.b)]
            return sum(1 for one in sides if one) == 1
        except NotAffine:
            return False
    return False
