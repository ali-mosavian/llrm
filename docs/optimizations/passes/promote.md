> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/promote.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/promote.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/promote.rs sha256:6f664426a9d29151e23f4bf615a84268da2ca7ec9bdeb958743b0c9c7e0b3b99 -->

# promote

Write-through promotion: a load of a cell that a store left its value in on
every path becomes that value, a phi where paths join. Every store stays;
`dse` removes the ones nothing reads. `Sroa` is the same rewrite for the
leaves of aggregates alone.

## How it works
A cell is a scalar memory location, named by a `Key`: an exact leaf of a
memory object (its object, byte range, `!tbaa` type and `restrict` roots)
or else its pointer decomposed (`MemRef`, any root). Accesses of one leaf
through distinct but equal pointers are one cell. `memoryssa::Accesses`
says what each instruction names and writes.

A cell is blocked, and stays in memory, when
- an access is volatile;
- its bytes partly overlap another leaf's (`_blocked`): a narrower store
  into a wider cell, the halves of one, a wider cell read in part;
- it is loaded as more than one type (an access of another type is only a
  write).

A cell is a candidate when it is accessed more than once. For `Sroa` it
must also be a leaf of an object that holds more than that leaf, and
`splitcopy` first turns a copy of an aggregate into its leaves' loads and
stores.

`_available` is a forward must-analysis over the reachable blocks: the set
of cells whose last write on every path is a store of the cell's own type.
A block's entry set is the intersection of its predecessors' exits (empty
at the entry). Walking a block, a store of a cell adds it; a write that may
reach the cell (`regions::overlapping`, calls by their effects) or a
definition of a value the cell's address is made from removes it; an
instruction with no known writes removes every cell. A load of a cell in
the set is planned. A load reached before any store keeps its memory value.

`rewrite` then places phis as LLVM's `PromoteMemToReg` does: at the
dominance frontier of each cell's stores, only where the cell is live on
entry, and renames down the dominator tree. Each planned load is replaced
by the stored operand or the phi. With `-g`, the debug record of the
variable a cell is gets the value, or a piece of it.

```text
 b1: store 7, %a   b2: store %d, %a       b1: store 7, %a   b2: store %d, %a
 b3: %x = load %a             =>          b3: %x = phi [7, b1], [%d, b2]
     %y = add %x, 1                           %y = add %x, 1
```

## Runs
In [`crate::pipeline`](pipeline.md)'s per-body fixed point: `Sroa` at the boundaries of
`Fixed` (start, after a peel, an unroll or a jam), before any pass asks
what is constant or invariant; `Promote` each round, after `gvn`. Both are
skipped on a body with an irreducible loop, which has no dominators. On at
every level above -O0 (`promote`, `-ftree-sra`).

## Needs
`memoryssa::Accesses`, `Held`, `regions`, [`crate::splitcopy`](splitcopy.md), the
dominator tree.

## Keeps
Nothing, where it changes a body.

## See also
LLVM's LICM scalar promotion (`promoteLoopAccessesToScalars`), function-wide
here, and SROA; gcc's `-ftree-sra`.
