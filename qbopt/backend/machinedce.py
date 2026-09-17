"""Eliminate pure allocated instructions whose complete result is dead.

MIR dead-code elimination reasons about values before allocation. Lowering,
splitting and physical rewrites can leave a machine computation whose virtual
definition still exists but whose physical register and flag results are all
dead. This pass answers only that machine question. Source memory reads,
control flow, trapping arithmetic, x87 work and relocations are deliberately
outside it.
"""

from dataclasses import replace

from iced_x86 import Register
from iced_x86 import RegisterExt

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model.passes import LIRTransform
from qbopt.objectfile.module import Space


_PURE = frozenset(
    {
        ir.Operation.MOVE,
        ir.Operation.EXCHANGE,
        ir.Operation.ADDRESS,
        ir.Operation.BINARY,
        ir.Operation.MULTIPLY,
        ir.Operation.COMPARE,
        ir.Operation.UNARY,
        ir.Operation.FUNNEL,
        ir.Operation.EXTEND,
    }
)
_STATEFUL_REGISTERS = frozenset({Register.ES, Register.CS, Register.SS, Register.DS, Register.FS, Register.GS})


class MachineDCE(LIRTransform):
    """Remove an allocated computation with no live physical result."""

    name = "machine-dce"

    def transform(self, body: lir.LirBody) -> lir.LirBody:
        return eliminated(body)


def _relocated(where: ir.Loc) -> bool:
    if isinstance(where, ir.Imm):
        return where.address is not None
    if isinstance(where, ir.Address):
        return where.addr is not None and where.addr.space is not Space.FRAME
    return False


def _stateful_destination(where: ir.Loc) -> bool:
    """Architectural state whose writes are not ordinary dead values."""
    return isinstance(where, ir.Reg) and (
        RegisterExt.full_register32(where.register) == Register.ESP or where.register in _STATEFUL_REGISTERS
    )


def _pure(one: lir.Insn) -> bool:
    """Whether removing this occurrence can remove only registers and flags."""
    what = one.what
    return bool(
        what is not None
        and what.op in _PURE
        and what.target is None
        and not what.indirect
        and all(isinstance(arg, (ir.Reg, ir.Imm, ir.Address)) for arg in (*what.dests, *what.sources))
        and not any(_relocated(arg) for arg in (*what.dests, *what.sources))
        and not any(_stateful_destination(dest) for dest in what.dests)
        and not one.clobbers
        and not one.clobbers_high
        and not one.requires
        and not one.delivers
        and not one.spread
        and one.group is None
        and one.symbol is not True
        and not one.frame_adjust
        and not one.spill_reload
        and not one.spill_store
        and not getattr(one.op, "barrier", False)
    )


def _once(body: lir.LirBody) -> lir.LirBody:
    from qbopt.backend import liveness
    from qbopt.backend.peephole import _branch_reads
    from qbopt.backend.peephole import _register_effects

    exits = liveness.dead_at_exit(body)
    blocks = []
    changed = False
    for block in body.blocks:
        dead = set(exits[block.at])
        redundant: set[int] = set()
        for one in reversed(block.insns):
            what = one.what
            if liveness._terminator(what):
                assert what is not None
                if what.op is ir.Operation.BRANCH:
                    dead -= _branch_reads(what)
                continue
            effects = _register_effects(one, flags=True)
            if effects is None:
                effects = liveness._declared(one)
            if effects is None:
                dead.clear()
                continue
            reads, writes = effects
            if writes and writes <= dead and _pure(one):
                redundant.add(id(one))
                changed = True
                continue
            dead = (dead | writes) - reads
        blocks.append(
            replace(
                block,
                insns=tuple(lir.anchor(one) if id(one) in redundant else one for one in block.insns),
            )
            if redundant
            else block
        )
    return replace(body, blocks=tuple(blocks)) if changed else body


def eliminated(body: lir.LirBody) -> lir.LirBody:
    """Remove dead pure machine work to a fixed point across CFG edges."""
    for _round in range(max(1, len(body.insns))):
        after = _once(body)
        if after is body:
            return body
        body = after
    return body
