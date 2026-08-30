"""
Re-measures docs/residue.md's patterns B, D, E, F, I against the CURRENT
rewritten object -- a mechanical, scripted walk of qbopt/ir.py's own node
stream, not a re-read of the disassembly by eye. Built because residue.md's
own counts for these five were stale twice over: once from the comparison-
absorption correctness fix (`f2b6f05`), and again from G+H's own closure
(commit 3), both of which changed what code sits next to what's left.

    uv run python tools/residue_census.py

Regenerates NBODYQ.OBJ fresh from build/bench/v-g3/NBODY.OBJ via
qbopt.rewrite.rewrite() -- never trusts a possibly-stale copy on disk -- then
decodes it with qbopt.ir.decode_module() and walks the node stream:

  B  a Restore node immediately followed by a push of its own pair's high
     and low halves and a pop of the same pair's 32-bit root -- the value
     round-trips through the stack back into the register it started in.
  D  two adjacent Opaque nodes that are the immediate-operand forms of the
     same low/high ALU pair lift.PAIRED already recognises in register/memory
     form (add ax,imm16 / adc dx,imm8 and siblings).
  F  an Opaque `cwd` node whose value is not the four-instruction constant
     idiom calls.widened_constant_at() already covers -- every register- or
     computed-source INTEGER-to-LONG sign extension.
  I  a Restore node whose high-half register (dx for pair 0, bx for pair 1)
     is not read by anything reachable after it before being overwritten --
     real block-graph backward liveness over qbopt.blocks' own Block/succ
     graph, not a same-block-only heuristic.
  E  regions tools/rewrite --report itself refuses with a "grows N bytes to
     M" reason -- the planner saw a smaller region than the statement really
     is because something interleaves, exactly what residue.md's E describes.

None of this changes qbopt/ itself; every pattern here is read off Effects
ir.py already computes (root-registers, not raw sub-registers) or off bytes
already in the object.
"""

import sys
from pathlib import Path
from dataclasses import dataclass

from iced_x86 import Code
from iced_x86 import Register
from iced_x86 import Register_

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt import ir
from qbopt import omf
from qbopt import module
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.lift import HALF_OF
from qbopt.declen import WRITES
from qbopt.flags import CLOBBERS
from qbopt.rewrite import rewrite
from qbopt import blocks as blocks_mod

ROOT = Path(__file__).resolve().parents[1]
BASE_OBJ = ROOT / "build" / "bench" / "v-g3" / "NBODY.OBJ"


# ---------------------------------------------------------------------------
# Setup: a fresh rewrite, decoded, every time -- never the file on disk.


@dataclass(frozen=True, slots=True)
class Built:
    data: bytes
    found: module.Module
    mapped: blocks_mod.CodeMap
    blocks: list[Block]
    bodies: tuple[ir.BodyIR, ...]
    nodes: list[ir.Node]  # every body's nodes, flattened, address order per body


def build() -> Built:
    base = BASE_OBJ.read_bytes()
    data, regions = rewrite(base, dry_run=False)
    taken = sum(1 for r in regions if r.taken)
    print(f"rewrite: {len(base)} -> {len(data)} bytes, {taken}/{len(regions)} regions taken")
    found = module.of(omf.parse(data))
    assert found is not None, "module.of() found no code segment in the fresh rewrite"
    mapped = blocks_mod.code_map(found)
    assert not isinstance(mapped, str), f"code_map failed: {mapped}"
    block_list = blocks_mod.partition(found, mapped)
    bodies = ir.decode_module(found)
    assert not isinstance(bodies, str), f"decode_module failed: {bodies}"
    nodes = [n for body in bodies for n in body.nodes]
    return Built(data, found, mapped, block_list, bodies, nodes)


# ---------------------------------------------------------------------------
# B -- restore immediately undone by a re-push of its own halves.

PAIR_REGS = {0: (Register.DX, Register.AX, Register.EAX), 1: (Register.BX, Register.CX, Register.ECX)}


@dataclass(frozen=True, slots=True)
class Instance:
    at: int
    end: int
    bytes_wasted: int
    detail: str


def find_b(built: Built) -> list[Instance]:
    found: list[Instance] = []
    nodes = built.nodes
    for i, node in enumerate(nodes):
        if not isinstance(node, ir.Restore):
            continue
        hi, lo, root = PAIR_REGS[node.pair]
        window = nodes[i + 1 : i + 4]
        if len(window) < 3:
            continue
        a, b, c = window
        if not (isinstance(a, ir.Opaque) and isinstance(b, ir.Opaque) and isinstance(c, ir.Opaque)):
            continue
        at = node.end
        contiguous = a.insn.at == at and b.insn.at == a.insn.end and c.insn.at == b.insn.end
        if not contiguous:
            continue
        is_push_hi = a.insn.code == Code.PUSH_R16 and a.insn.insn.op0_register == hi
        is_push_lo = b.insn.code == Code.PUSH_R16 and b.insn.insn.op0_register == lo
        is_pop_root = c.insn.code == Code.POP_R32 and c.insn.insn.op0_register == root
        if is_push_hi and is_push_lo and is_pop_root:
            wasted = (node.end - node.at) + (c.insn.end - a.insn.at)
            root_name = "eax" if root == Register.EAX else "ecx"
            found.append(Instance(node.at, c.insn.end, wasted, f"pair={node.pair} restore+repush -> {root_name}"))
    return found


# ---------------------------------------------------------------------------
# D -- immediate-operand ALU pair, missing from lift.PAIRED.

# low-form -> (high-form, name), the immediate-operand mirror of PAIRED.
IMM_LOW_HIGH = {
    Code.ADD_AX_IMM16: (Code.ADC_AX_IMM16, "add"),
    Code.ADD_RM16_IMM16: (Code.ADC_RM16_IMM16, "add"),
    Code.ADD_RM16_IMM8: (Code.ADC_RM16_IMM8, "add"),
    Code.SUB_AX_IMM16: (Code.SBB_AX_IMM16, "sub"),
    Code.SUB_RM16_IMM16: (Code.SBB_RM16_IMM16, "sub"),
    Code.SUB_RM16_IMM8: (Code.SBB_RM16_IMM8, "sub"),
    Code.AND_AX_IMM16: (Code.AND_AX_IMM16, "and"),
    Code.AND_RM16_IMM16: (Code.AND_RM16_IMM16, "and"),
    Code.AND_RM16_IMM8: (Code.AND_RM16_IMM8, "and"),
    Code.OR_AX_IMM16: (Code.OR_AX_IMM16, "or"),
    Code.OR_RM16_IMM16: (Code.OR_RM16_IMM16, "or"),
    Code.OR_RM16_IMM8: (Code.OR_RM16_IMM8, "or"),
    Code.XOR_AX_IMM16: (Code.XOR_AX_IMM16, "xor"),
    Code.XOR_RM16_IMM16: (Code.XOR_RM16_IMM16, "xor"),
    Code.XOR_RM16_IMM8: (Code.XOR_RM16_IMM8, "xor"),
}
IMM_HIGH_FAMILY = {}
for _low, (_high, _name) in IMM_LOW_HIGH.items():
    IMM_HIGH_FAMILY.setdefault(_name, set()).add(_high)


def _imm_dest(insn: Insn) -> Register_ | None:
    reg = insn.insn.op0_register
    return reg if reg in HALF_OF else None


def find_d(built: Built) -> list[Instance]:
    found: list[Instance] = []
    nodes = built.nodes
    for i in range(len(nodes) - 1):
        a, b = nodes[i], nodes[i + 1]
        if not (isinstance(a, ir.Opaque) and isinstance(b, ir.Opaque)):
            continue
        pairing = IMM_LOW_HIGH.get(a.insn.code)
        if pairing is None:
            continue
        if a.insn.at + a.insn.length != b.insn.at:
            continue  # not contiguous
        _high, name = pairing
        if b.insn.code not in IMM_HIGH_FAMILY[name]:
            continue
        ra, rb = _imm_dest(a.insn), _imm_dest(b.insn)
        if ra is None or rb is None:
            continue
        pa, ha = HALF_OF[ra]
        pb, hb = HALF_OF[rb]
        if pa != pb or ha == hb:
            continue
        span = a.insn.at, b.insn.end
        detail = f"{name} imm pair, {a.insn.insn}/{b.insn.insn}"
        found.append(Instance(span[0], span[1], b.insn.end - a.insn.at, detail))
    return found


# ---------------------------------------------------------------------------
# F -- sign extension (cwd) lift.py and calls.py cannot see.


def _is_widened_constant_head(nodes: list[ir.Node], i: int) -> bool:
    """Whether nodes[i] (the cwd) is the immediate-constant idiom
    calls.widened_constant_at() already recognises -- mov ax,imm16 / cwd /
    push dx / push ax, contiguous. Mirrors that function's own checks."""
    if i == 0:
        return False
    prev = nodes[i - 1]
    node = nodes[i]
    if not (isinstance(prev, ir.Opaque) and isinstance(node, ir.Opaque)):
        return False
    if prev.insn.code != Code.MOV_R16_IMM16 or prev.insn.insn.op0_register != Register.AX:
        return False
    if prev.insn.end != node.insn.at:
        return False
    tail = nodes[i + 1 : i + 3]
    if len(tail) < 2:
        return False
    push_dx, push_ax = tail
    if not (isinstance(push_dx, ir.Opaque) and isinstance(push_ax, ir.Opaque)):
        return False
    ok = (
        push_dx.insn.code == Code.PUSH_R16
        and push_dx.insn.insn.op0_register == Register.DX
        and push_dx.insn.at == node.insn.end
        and push_ax.insn.code == Code.PUSH_R16
        and push_ax.insn.insn.op0_register == Register.AX
        and push_ax.insn.at == push_dx.insn.end
    )
    return ok


def find_f(built: Built) -> list[Instance]:
    found: list[Instance] = []
    nodes = built.nodes
    for i, node in enumerate(nodes):
        if not (isinstance(node, ir.Opaque) and node.insn.code == Code.CWD):
            continue
        if _is_widened_constant_head(nodes, i):
            continue  # calls.py already covers this exact shape
        prev = nodes[i - 1] if i > 0 else None
        source = prev.insn.insn if isinstance(prev, (ir.Opaque, ir.Long)) else None
        found.append(Instance(node.insn.at, node.insn.end, 0, f"cwd, source={source}"))
    return found


# ---------------------------------------------------------------------------
# I -- restores whose high half nothing reads. Real block-graph backward
# liveness for one register root at a time, mirroring qbopt/flags.py's own
# live_in()/live_after() shape exactly, generalised from Flag bits to a
# single Register_ root.


# deliberately NOT ir.ROOT: ir.py roots every sub-register to its 32-bit
# parent and (correctly, for its own purpose) counts a write to a
# sub-register as also a *use* of the root, because BC relies on the
# untouched high bits surviving. That is exactly the "ax/dx as one joint
# unit instead of two independent registers" bug residue.md's own intro
# warns a prior script-driven attempt already made: a plain `pop dx` would
# come out as "uses edx" and look like a read of dx's OLD value, when the
# whole point of asking here is whether dx's old value is still wanted at
# all. So this checks the literal 16- and 32-bit registers iced reports,
# with no rooting -- DL/DH are folded in defensively (this benchmark never
# writes them) rather than risk mis-reading a byte-sized alias.
GROUP = {
    Register.EDX: {Register.EDX, Register.DX, Register.DL, Register.DH},
    Register.EBX: {Register.EBX, Register.BX, Register.BL, Register.BH},
}
FULL_WIDTH = {Register.EDX, Register.DX, Register.EBX, Register.BX}


def _touches(insn: Insn, target: Register_) -> tuple[bool, bool]:
    """(reads target, fully overwrites target) for one real instruction."""
    if insn.flow in CLOBBERS:  # a call/interrupt -- conservative both ways
        return True, True
    group = GROUP[target]
    read = write = False
    for one in INFO.info(insn.insn).used_registers():
        if one.register not in group:
            continue
        if one.access in READS:
            read = True
        if one.access in WRITES and one.register in FULL_WIDTH:
            write = True
    return read, write


def _block_reads(block: Block, target: Register_) -> bool:
    written = False
    for insn in block.insns:
        r, w = _touches(insn, target)
        if r and not written:
            return True
        if w:
            written = True
    return False


def _block_writes(block: Block, target: Register_) -> bool:
    for insn in block.insns:
        _, w = _touches(insn, target)
        if w:
            return True
    return False


def live_in_reg(blocks: list[Block], target: Register_) -> dict[int, bool]:
    known = {b.at for b in blocks}
    live = {b.at: False for b in blocks}
    uses = {b.at: _block_reads(b, target) for b in blocks}
    defs = {b.at: _block_writes(b, target) for b in blocks}
    changing = True
    while changing:
        changing = False
        for block in reversed(blocks):
            out = block.leaves  # conservative: unmapped/returning edges are live
            for succ in block.succ:
                out = out or live[succ] if succ in known else True
            now = uses[block.at] or (out and not defs[block.at])
            if now != live[block.at]:
                live[block.at] = now
                changing = True
    return live


def live_after_reg(block: Block, offset: int, target: Register_, live: dict[int, bool]) -> bool:
    out = block.leaves
    for succ in block.succ:
        out = out or live.get(succ, True)
    written = False
    for insn in block.insns:
        if insn.at < offset:
            continue
        r, w = _touches(insn, target)
        if r and not written:
            return True
        if w:
            written = True
    return not written and out


def find_i(built: Built) -> list[Instance]:
    found: list[Instance] = []
    live = {
        Register.EDX: live_in_reg(built.blocks, Register.EDX),
        Register.EBX: live_in_reg(built.blocks, Register.EBX),
    }
    for node in built.nodes:
        if not isinstance(node, ir.Restore):
            continue
        target = Register.EDX if node.pair == 0 else Register.EBX
        block = blocks_mod.block_at(built.blocks, node.end)
        if block is None:
            continue  # restore not attributed to a decoded block -- skip, don't guess
        alive = live_after_reg(block, node.end, target, live[target])
        if not alive:
            found.append(Instance(node.at, node.end, node.end - node.at, f"pair={node.pair} dx/bx dead"))
    return found


# ---------------------------------------------------------------------------
# E -- refused regions naming their own byte growth.
#
# A "grows N bytes to M" refusal has two different real causes that look
# identical in rewrite --report's own output: genuine E (an interleaved,
# non-conflicting instruction breaks region contiguity, e.g. an unrelated
# array index computed between a load pair and the alu/store pair that reads
# it) and a plain consequence of D (the region already IS contiguous, but
# classify() cannot see the immediate-operand alu pair, so the region the
# planner considers is truncated to a lone load and a lone load plus its
# restore is bigger than leaving it alone). Both surface as the same refusal
# reason, so a region is only counted as E here if a D instance is NOT sitting
# immediately at its own end -- i.e. nothing contiguous was actually missed
# for D's own, already-counted reason.


def find_e(d_instances: list[Instance]) -> list[str]:
    base = BASE_OBJ.read_bytes()
    _data, regions = rewrite(base, dry_run=False)
    d_starts = {x.at for x in d_instances}
    out = []
    for r in regions:
        if r.taken or not r.reason or "grows" not in r.reason:
            continue
        cause = "D's own truncation (not counted here)" if r.end in d_starts else "genuine interleave"
        out.append(f"{r.id:3} {r.at:#06x}..{r.end:#06x}  {r.reason}  [{cause}]")
    return out


# ---------------------------------------------------------------------------


def main() -> int:
    built = build()

    b = find_b(built)
    d = find_d(built)
    f = find_f(built)
    i = find_i(built)
    e = find_e(d)

    print(f"\nB  restore/re-push round trip: {len(b)} instances, {sum(x.bytes_wasted for x in b)} bytes")
    for x in b:
        print(f"     {x.at:#06x}-{x.end:#06x}  {x.bytes_wasted:2}b  {x.detail}")

    d_bytes = sum(x.bytes_wasted for x in d)
    print(f"\nD  immediate-pair ALU missing: {len(d)} instances, {d_bytes} bytes (local pair only)")
    for x in d:
        print(f"     {x.at:#06x}-{x.end:#06x}  {x.bytes_wasted:2}b  {x.detail}")

    print(f"\nF  invisible sign-extension (cwd): {len(f)} instances")
    for x in f:
        print(f"     {x.at:#06x}-{x.end:#06x}  {x.detail}")

    print(f"\nI  dead lift.py restores: {len(i)} instances, {sum(x.bytes_wasted for x in i)} bytes")
    for x in i:
        print(f"     {x.at:#06x}-{x.end:#06x}  {x.bytes_wasted:2}b  {x.detail}")

    print(f"\nE  refused regions naming their own growth: {len(e)} instances")
    for line in e:
        print(f"     {line}")

    base_len = len(built.found.code)
    print(f"\ncode size: {base_len} bytes (this rewritten object)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
