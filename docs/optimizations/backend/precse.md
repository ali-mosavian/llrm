> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/precse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/precse.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/precse.rs sha256:571e4340f3c3441347dab92058f8f988e65bf1e20eb9453ed78f355257dd43ad -->

# precse

The same pure computation made twice, made once: `MachineCSE` before PHI
elimination, where a value has one definition. Translated from
[MachineCSE.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp)
(`ProcessBlockCSE`, `isCSECandidate`, `isProfitableToCSE`).

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

DEVIATE E1 (no number): no `PerformSimplePRE` (hoisting an expression
computed in sibling blocks to their dominator) and no load of an invariant
cell (`isDereferenceableInvariantLoad`). Exit: both translated.

DEVIATE E2 (no number): the readers of a value are counted from the body as
it was, plus the readers merged in by earlier removals; the pin counts the
live use lists. Same decisions on a body where one removal does not change
another's readers.

DEVIATE E3 (no number): an operation with no register operand (a constant,
the address of a global or a frame cell) is not a candidate. The pin shares
it and rematerializes it where it is wanted; ours cannot once TwoAddress
and the coalescer write over it in place: compare_to_int64.c -Os 346 to 361
bytes, grep -O2 -m32 +21.9% instructions. Exit: the allocator remakes a
value with two defs.

## Example
`v3 := add v1, v2` in a block and `v7 := add v1, v2` in a block it
dominates: `v7` is gone and its readers read `v3`.

## Runs
A machine phase before `machinesink` (`flow::machine_with`), at every -O
level but -O0.
