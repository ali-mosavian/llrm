> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/precse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/precse.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/precse.rs sha256:711d45c651e91bce47130f772aaca691e762a703ced818004179fe73caefa220 -->

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

When the table has no entry, an operand made by a plain copy is moved to the
copy's source and a copy nothing else reads is erased
(`PerformTrivialCopyPropagation`); a commutative operation is then asked
with its sources swapped if the first form is still not there.

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

DEVIATE CSE1 (owner w-2, exit with isel.rs I2): no `hasMinSize` shortcut
(the machine phases have no minsize flag; it is -Oz only, which no sweep
covers). `isLegalToHoistInto` is translated: a return block is no target;
the pin's other two conditions cannot hold on this LIR (see
`legal_to_hoist_into`).

DEVIATE CSE2 (owner funcattrs-port, exit 2026-11-13): an operation with no
register operand is not a candidate, a load of a constant global with a
fixed address among them (the pin shares and rematerializes it; ours cannot
once TwoAddress and the coalescer write over it in place: compare_to_int64.c
-Os 346 to 361 bytes, grep -O2 -m32 +21.9% instructions). Exit: the
allocator remakes a value with two defs. (This was E3.)

DEVIATE CSE3 (owner w-1, exit 2026-11-08): both selectors mark a load
invariant when it reads a constant global or says so (`!invariant.load`, a
`noalias readonly` argument), and the DAG selector the load of an incoming
stack argument (an immutable fixed object). The pin also takes GOT and
constant-pool loads, which ours lacks; the old selector reads an incoming
argument as a memory operand of its reader, not a load of its own.

Row CSE5 (was DEVIATE CSE5, retired 2026-10-25): `machinecse.rs`, the same
elimination after allocation, was ours alone (the pin runs MachineCSE once,
before allocation). Deleted: d_faces -3.60 M Ir (dist). 7 of 568 objects grow
and none shrinks (x_mergesort +34 B at m32 -Os and m16 -O1 to -Os, shellsort
m16 +22 at -O1/-O2), QCport (tools/rows/qcport.py) +0.030% -O1, +0.080% -O2,
+0.116% -O3, +0.025% -Os, x_mergesort -Os +0.43% clocks. Exit: CSE2 and
rematerialization of multi-def values in RA (w13/w14).

## Example
`v3 := add v1, v2` in a block and `v7 := add v1, v2` in a block it
dominates: `v7` is gone and its readers read `v3`.

## Runs
A machine phase before `machinesink` (`flow::machine_with`), at every -O
level but -O0.
