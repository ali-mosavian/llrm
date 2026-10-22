> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/precse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/precse.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/precse.rs sha256:a23ac6624b49344969db14aba819b6e248fc1bcd84b8f2251ab70dbd12eec4f2 -->

# precse

The same pure computation made twice, made once: `MachineCSE` before PHI
elimination, where a value has one definition. Translated from
[MachineCSE.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp)
(`ProcessBlockCSE`, `isCSECandidate`, `isProfitableToCSE`,
`PerformSimplePRE`).

## How it works
The dominator tree is walked with a scoped table from an instruction's
expression (operation, destination width and sources) to the value it
defines: a block sees the entries of the blocks that dominate it and drops
its own when its subtree is done (`ScopedHashTable`, `EnterScope`,
`ExitScope`). An instruction that `candidate` admits (no store, call,
terminator, volatile access, fixed register or load) and finds its
expression in the table is removed, and every use of its value reads the
table's, when `profitable` agrees:

- an operation as cheap as a move is shared only inside its block or with a
  block that dominates it by an edge (heuristic 1), so a constant does not
  stay live across a function;
- a value is shared only if every reader of the new one also reads the old
  one, or the old one is read in the block (heuristics 2 and 3).

Before the table is asked, an operand made by a plain copy names the copy's
source (`PerformTrivialCopyPropagation`), and a commutative operation is
asked with its sources swapped if the first form is not there.

An operation that sets flags is shared only where nothing after it in its
block reads them (the pin's `hasLivePhysRegDefUses`: EFLAGS).

An invariant load (a load of a constant global the module defines,
`isDereferenceableInvariantLoad`; selection marks it `Insn::invariant`) is
a candidate like any other, and a removed instruction no longer counts as a
reader of its operands.

`PerformSimplePRE` runs first: an expression computed in two blocks, where
neither dominates the other and one reaches the other, is also computed
before the terminator of their nearest common dominator (when that block
runs no more often than the two together); the walk above then removes the
two. A copy no one reads is dropped again (the pin leaves it to
`DeadMachineInstructionElim`).

DEVIATE CSE1 (owner funcattrs-port, exit 2026-11-13):
`isProfitableToHoistInto` reads the block frequencies selection estimated
(`LirBody::frequencies`, kept through rewrites), not
`MachineBlockFrequencyInfo` of the current CFG; a body with no table gets no
PRE. No `hasMinSize` shortcut (the selector has no minsize flag, isel.rs I2)
and no `isLegalToHoistInto` test (no block says it is a landing pad).

DEVIATE CSE2 (owner funcattrs-port, exit 2026-11-13): an operation with no
register operand is not a candidate, a load of a constant global with a
fixed address among them (the pin shares and rematerializes it; ours cannot
once TwoAddress and the coalescer write over it in place: compare_to_int64.c
-Os 346 to 361 bytes, grep -O2 -m32 +21.9% instructions). Exit: the
allocator remakes a value with two defs. (This was E3.)

DEVIATE CSE3 (owner funcattrs-port, exit 2026-11-13): only a load selected
by a pattern from a constant global is invariant. The pin also takes
`!invariant.load`, GOT and constant-pool loads and the immutable fixed stack
objects (incoming arguments); those loads are kept.

DEVIATE CSE4 (owner funcattrs-port, exit 2026-11-13):
`PerformTrivialCopyPropagation` renames an operand in the table's key
without moving the instruction's read to the copy's source, so the readers
of a copy's source are counted from the body as it was. Heuristics 2 and 3
see the pin's lists except for that.

## Example
`v3 := add v1, v2` in a block and `v7 := add v1, v2` in a block it
dominates: `v7` is gone and its readers read `v3`.

## Runs
A machine phase before `machinesink` (`flow::machine_with`), at every -O
level but -O0.
