"""
Put each bug back, and check something notices.

A test suite that has never been watched failing proves nothing about the bugs
it was written for. This applies a named one-line change to a copy of the tree,
runs the tests there, and reports which mutations survived. A mutation nothing
catches is a hole, and the run fails on one.

Confirming the mutation applied is half of it, and the half usually skipped:
without it a typo in the pattern reads as "the tests caught nothing", which
looks the same as a mutation that could not be made.

    uv run python tools/mutate.py
    uv run python tools/mutate.py --only checksum
"""

import sys
import shutil
import argparse
import subprocess
from pathlib import Path
from dataclasses import dataclass
from tempfile import TemporaryDirectory
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parents[1]
COPIED = ("qbopt", "tests", "tools", "fixtures", "suite", "pyproject.toml")


@dataclass(frozen=True, slots=True)
class Mutation:
    name: str
    file: str
    before: str
    after: str
    bug: str


MUTATIONS = (
    Mutation(
        "absorbed-divide-without-the-sign",
        "qbopt/transform.py",
        '        steps.append(made(ir.Operation.EXTEND, "cdq", (_wide(Register.EDX),), (_wide(RESULT),)))',
        "        pass  # no cdq",
        "`idiv` reads edx:eax and `cdq` is what puts the dividend's sign in edx -- "
        "without it every negative dividend divides as if it were huge and positive",
    ),
    Mutation(
        "absorbed-operands-the-wrong-way-round",
        "qbopt/transform.py",
        "INTO = (Register.EAX, Register.ECX)",
        "INTO = (Register.ECX, Register.EAX)",
        "the dividend goes in eax and the divisor in ecx -- swapped, every divide "
        "and every multiply-by-an-asymmetric-pair is a different answer, not a slower one",
    ),
    Mutation(
        "absorbed-over-a-live-flag",
        "qbopt/transform.py",
        "            if any(one.flags and one in read for one in op.defines):",
        "            if False:",
        "`imul` and `idiv` leave their own flags where the call left the runtime's, "
        "so a jcc after an absorbed site reads a different answer",
    ),
    Mutation(
        "an-unknown-call-trusted-for-its-arity",
        "qbopt/transform.py",
        "    return 2 if name is not None and name.upper() in ABSORB else None",
        "    return 2 if name is not None else None",
        "stack.frames() trusts a recognised call to have consumed exactly arity*4 "
        "bytes and disturbed nothing else -- claimed for a routine whose contract "
        "nothing here knows, every frame found after it in the block is shifted",
    ),
    Mutation(
        "widening-load-removed",
        "qbopt/forward.py",
        "    memory = MemorySizeExt.size(insn.insn.memory_size)",
        "    memory = 0x7FFFFFFF  # every write looks narrow",
        "`movsx eax,word [x]` reads two bytes and writes four, so a store of ax to "
        "[x] is not a provider for it -- deleting the load leaves the high half stale",
    ),
    Mutation(
        "written-operand-substituted",
        "qbopt/reencode.py",
        "    if any(one.access in WRITES for one in INFO.info(insn.insn).used_memory()):",
        "    if False:",
        "`add [x],1` writes its result back to memory and `add ax,1` does not -- "
        "substituting a written memory operand deletes the store, silently",
    ),
    Mutation(
        "accumulate-read-as-a-load",
        "qbopt/avail.py",
        "    reading = set(_real(op.uses)) - _addressing(op) - _preserved(op, defines[0], origin)",
        "    reading = set()",
        "`and cx,[x]` reads cx as data as well as defining it, so the bytes it "
        "leaves there are not the cell's -- taking it as a provider forwards the "
        "wrong value",
    ),
    Mutation(
        "checksum",
        "qbopt/omf.py",
        "return head + body + bytes([(-sum(head) - sum(body)) & 0xFF])",
        "return head + body + bytes([0])",
        "a zero checksum, which many tools write and BC does not",
    ),
    Mutation(
        "frame-thread-index",
        "qbopt/omf.py",
        "    if method < 3:",
        "    if method & 3 < 3:",
        "frame methods 4 and 5 read an index they do not carry",
    ),
    Mutation(
        "displacement-dropped",
        "qbopt/omf.py",
        '                disp, disp_pos = struct.unpack_from("<H", body, at)[0], at',
        "                disp, disp_pos = 0, at",
        "the target displacement, which holds every static address",
    ),
    Mutation(
        "ledata-first-write-wins",
        "qbopt/omf.py",
        "    for _record, index, offset, payload in ledata(records):",
        "    for _record, index, offset, payload in reversed(ledata(records)):",
        "BC's backpatch records overwritten by the earlier ones",
    ),
    Mutation(
        "shift-boundary",
        "qbopt/relocate.py",
        "            if offset >= edit.hi:",
        "            if offset > edit.hi:",
        "an offset exactly at a region's end is treated as inside it",
    ),
    Mutation(
        "branch-from-start",
        "qbopt/relocate.py",
        "    return shift.at(branch.target) - shift.before(branch.end)",
        "    return shift.at(branch.target) - shift.before(branch.at)",
        "a branch mapped from its own start rather than its end",
    ),
    Mutation(
        "rel8-truncated",
        "qbopt/relocate.py",
        "        if not reaches(branch, moved):",
        "        if False:",
        "a rel8 that no longer reaches, truncated instead of refused",
    ),
    Mutation(
        "target-not-shifted",
        "qbopt/relocate.py",
        "                disp = shift.at(fixup.disp) if into_code else None",
        "                disp = fixup.disp if into_code else None",
        "a fixup's offset moved but not what it points at",
    ),
    Mutation(
        "branch-end-not-before-insertion",
        "qbopt/relocate.py",
        "    return shift.at(branch.target) - shift.before(branch.end)",
        "    return shift.at(branch.target) - shift.at(branch.end)",
        "a branch ending exactly at a pure insertion shifted past it instead of left in front",
    ),
    Mutation(
        "divergence-gate",
        "qbopt/flags.py",
        "DIVERGENT = Flag.ZF | Flag.PF | Flag.AF",
        "DIVERGENT = Flag.NONE",
        "the flag gate opened",
    ),
    Mutation(
        "cleared-is-not-written",
        "qbopt/declen.py",
        "self.insn.rflags_written | self.insn.rflags_cleared",
        "self.insn.rflags_written",
        "a flag forced to zero counted as untouched -- what `and` does to CF",
    ),
    Mutation(
        "displacement-sign",
        "qbopt/declen.py",
        "return to_signed(self.insn.memory_displacement, BITNESS // 8) if self.disp_len else 0",
        "return to_signed(self.insn.memory_displacement, self.disp_len) if self.disp_len else 0",
        "a bp-relative displacement widened from its encoded size rather than the address size",
    ),
    Mutation(
        "leaves-not-conservative",
        "qbopt/flags.py",
        "            out = ALL if block.leaves else Flag.NONE",
        "            out = Flag.NONE",
        "flags assumed dead past an edge nothing can see",
    ),
    Mutation(
        "call-ends-a-block",
        "qbopt/blocks.py",
        "    FlowControl.CALL: Ends.FALLS_THROUGH,",
        "    FlowControl.CALL: Ends.RETURN,",
        "a call treated as the end of a block, which it is not",
    ),
    Mutation(
        "relocated-operand-not-zero",
        "qbopt/lift.py",
        "return MemoryOperand(base=base, displ=0, displ_size=2)",
        "return MemoryOperand(base=base, displ=1, displ_size=2)",
        "a relocated operand emitted with its address in the code, which LINK adds to",
    ),
    Mutation(
        "consume-slot-order-not-reversed",
        "qbopt/calls.py",
        "steps: list[Instruction] = [popped_into(target) for target in targets]",
        "steps: list[Instruction] = [popped_into(target) for target in reversed(targets)]",
        "a popped argument landing in the wrong register -- dividend and divisor swapped, not a crash",
    ),
    Mutation(
        "absorb-always-reloads-the-right-operand",
        "qbopt/calls.py",
        "same_address = left.kind is Kind.STATIC and right.kind is Kind.STATIC and left.addr == right.addr",
        "same_address = False",
        "x*x reloads the same address twice instead of loading it once -- correct, just wasteful",
    ),
    Mutation(
        "popped-into-wrong-width",
        "qbopt/calls.py",
        "return Instruction.create_reg(Code.POP_R32, target)",
        "return Instruction.create_reg(Code.POP_R16, target)",
        "a popped argument only recovers its low 16 bits, garbage in the rest of the register",
    ),
    Mutation(
        "segment-override-dropped",
        "qbopt/lift.py",
        "            return MemoryOperand(base=base, displ=disp, displ_size=2, seg=segment)",
        "            return MemoryOperand(base=base, displ=disp, displ_size=2)",
        "`es:[bx]` widened to `ds:[bx]` -- the override dropped, so the pair "
        "reads and writes the wrong segment",
    ),
    Mutation(
        "group-address-not-refused",
        "qbopt/lift.py",
        "            return None if resolved.space is Space.GROUP else resolved",
        "            return resolved",
        "a group-relative fixup treated as a real address instead of refused",
    ),
    Mutation(
        "ir-node-dropped-from-emit-order",
        "qbopt/ir.py",
        "    return tuple(nodes)",
        "    return tuple(nodes[1:])",
        "a node missing from decode_body's own output -- the byte-identical gate must notice",
    ),
    Mutation(
        "ir-root-register-narrowed",
        "qbopt/ir.py",
        "    Register.AL: Register.EAX,",
        "",
        "a sub-register no longer normalised to its root -- a node's own def/use set narrowed by one register",
    ),
    Mutation(
        "bodyedit-range-edge",
        "qbopt/bodyedit.py",
        "    if at == hi and at in owner:",
        "    if False:",
        "an insertion exactly on a body range's own edge accepted -- unreachable from a branch "
        "targeting the leading edge, ambiguous with whatever follows the trailing edge",
    ),
    Mutation(
        "bodyedit-table-not-skipped",
        "qbopt/bodyedit.py",
        "        if isinstance(node, ir.Data):",
        "        if False:",
        "an inline table's own start considered as a candidate insertion point instead of skipped",
    ),
    Mutation(
        "tail-seed-not-preserved",
        "qbopt/lift.py",
        "    live: dict[int, int | None] = {0: 0, 1: None}",
        "    live: dict[int, int | None] = {0: None, 1: None}",
        "a call's own result treated as fully clobbered instead of the value it always leaves in eax, "
        "walling off the same G/H shape this commit exists to fix",
    ),
    Mutation(
        "combined-call-keeps-its-restore",
        "qbopt/rewrite.py",
        "        emitted = absorb(site, after, restore=False)",
        "        emitted = absorb(site, after, restore=True)",
        "a widened region emitted without actually removing the now-redundant restore -- putting the "
        "high half back only to immediately re-derive it from eax",
    ),
    Mutation(
        "combined-call-need-not-forced",
        "qbopt/rewrite.py",
        "        need[0] = True  # the call's own bytes replace the deleted pushes/call outright, never optional\n",
        "",
        "a call's own bytes dropped as unneeded while the edit still deletes the original call -- silent "
        "stack corruption when the call's result is overwritten before anything reads it",
    ),
    Mutation(
        "immediate-combination-not-resigned",
        "qbopt/lift.py",
        "combined = to_signed((high.imm << 16) | (low.imm & 0xFFFF), 4)",
        "combined = (high.imm << 16) | (low.imm & 0xFFFF)",
        "a negative immediate pair left as an unsigned 32-bit pattern -- iced's own builders "
        "reject it outright, or silently encode a different value if they don't",
    ),
    Mutation(
        "conditional-write-counted-as-a-kill",
        "qbopt/registers.py",
        "KILLS = (OpAccess.WRITE, OpAccess.READ_WRITE)",
        "KILLS = (OpAccess.WRITE, OpAccess.READ_WRITE, OpAccess.COND_WRITE, OpAccess.READ_COND_WRITE)",
        "a conditional write that might not fire treated as always overwriting dx/bx -- "
        "a still-live restore judged dead",
    ),
    Mutation(
        "register-read-not-detected",
        "qbopt/registers.py",
        "        if one.access in READS:\n            read = True",
        "        if False:\n            read = True",
        "a real read of dx/bx never noticed -- every restore looks dead, including one something "
        "downstream still needs",
    ),
    Mutation(
        "round-trip-fires-on-a-targeted-gap",
        "qbopt/rewrite.py",
        "        if anchored_inside(found, mapped, a.edit.lo, b.edit.hi) is not None:\n            continue",
        "",
        "the restore/re-push fold applied even when a branch, public symbol, or line number "
        "targets the very bytes it removes",
    ),
    Mutation(
        "round-trip-matches-the-wrong-pair",
        "qbopt/rewrite.py",
        "        if found.code[gap_lo : gap_lo + 2] != PUSH_HI_LO[pair]:\n            continue",
        "",
        "a restore for one pair folded against the next call's pop of the OTHER pair's root -- "
        "the two registers never actually round-trip",
    ),
    Mutation(
        "bridge-crosses-control-flow",
        "qbopt/lift.py",
        "    if insn.flow != FlowControl.NEXT:\n        return False",
        "    if False:\n        return False",
        "a jump or branch bridged like an ordinary instruction -- fixtures/omf's own "
        "jumps-p-g2-zd.obj has one mid-statement, and what follows it is never actually "
        "reached the way a fall-through would be",
    ),
    Mutation(
        "bridge-continues-after-a-store",
        "qbopt/lift.py",
        "        if not committed and _bridges(insn):",
        "        if _bridges(insn):",
        "a gap bridged right after a value is already committed to memory -- suite/nots.bas "
        "pre-stages a call's own argument at a frame address this pass cannot tell apart from "
        "an ordinary local, and something landing between that store and the call corrupts it",
    ),
    Mutation(
        "commit-tracked-only-by-the-last-value",
        "qbopt/lift.py",
        "                stores.append(len(values) - 1)\n                committed = True",
        "                stores.append(len(values) - 1)",
        "a store's own commit forgotten the moment one more, unrelated value (a second pair's "
        "own fresh load) is added before the gap -- the same bug in a shape one value deeper",
    ),
    Mutation(
        "bridged-fixup-dropped",
        "qbopt/lift.py",
        "            fields = (field,) if field is not None else ()",
        "            fields = ()",
        "a relocated address inside a bridged instruction (BC's own `mov di,[array]`) spliced "
        "into a bigger region with no relocation to carry it -- the field is orphaned and reads "
        "as a bare zero after rewriting",
    ),
    Mutation(
        "sign-extend-not-adjacent",
        "qbopt/lift.py",
        "    if cwd.code != Code.CWD or cwd.at != first.end or first.register(0) != Register.AX:",
        "    if cwd.code != Code.CWD or first.register(0) != Register.AX:",
        "movsx claimed for a mov and a cwd that are not actually adjacent -- whatever real "
        "instruction sits between them is silently dropped from the program",
    ),
)


@dataclass(frozen=True, slots=True)
class Outcome:
    mutation: Mutation
    applied: bool
    caught: bool
    detail: str


def run_one(mutation: Mutation, into: Path, marker: str) -> Outcome:
    tree = into / mutation.name
    tree.mkdir(parents=True)
    for name in COPIED:
        source = ROOT / name
        if source.is_dir():
            shutil.copytree(source, tree / name, symlinks=True)
        else:
            shutil.copy(source, tree / name)

    target = tree / mutation.file
    text = target.read_text()
    if text.count(mutation.before) != 1:
        return Outcome(mutation, False, False, f"pattern appears {text.count(mutation.before)} times")
    target.write_text(text.replace(mutation.before, mutation.after))

    done = subprocess.run(
        [sys.executable, "-m", "pytest", "-x", "-q", "-p", "no:cacheprovider", "-m", marker],
        cwd=tree,
        capture_output=True,
        check=False,
    )
    tail = done.stdout.decode().strip().splitlines()
    return Outcome(mutation, True, done.returncode != 0, tail[-1] if tail else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="mutate")
    ap.add_argument("--only", action="append", choices=[m.name for m in MUTATIONS])
    ap.add_argument("--marker", default="not e2e", help="which tier to run against each mutation")
    ap.add_argument("--jobs", type=int, default=4)
    args = ap.parse_args(argv)

    wanted = [m for m in MUTATIONS if not args.only or m.name in args.only]
    with TemporaryDirectory() as temporary:
        into = Path(temporary)
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            outcomes = list(pool.map(lambda m: run_one(m, into, args.marker), wanted))

    survived = []
    for outcome in outcomes:
        if not outcome.applied:
            state = "NOT APPLIED"
        elif outcome.caught:
            state = "caught"
        else:
            state = "SURVIVED"
        print(f"  {state:12} {outcome.mutation.name:26} {outcome.mutation.bug}")
        if not outcome.applied or not outcome.caught:
            survived.append(outcome)

    print(f"\n{len(outcomes) - len(survived)} of {len(outcomes)} mutations caught")
    for outcome in survived:
        print(f"  {outcome.mutation.name}: {outcome.detail}")
    return 1 if survived else 0


if __name__ == "__main__":
    sys.exit(main())
