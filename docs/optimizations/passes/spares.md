> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spares.rs sha256:674b644d3087de087024d4239e6c22fceea28e943acb856e8518fd74fb454b14 -->

# spares

What the optimizer proved of a fixed-cell load, kept for instruction
selection: each write that leaves the load's bytes as they were, as
`!llrm.spares` on the load.

## How it works
In each function with a loop, for each non-volatile load whose address is
an object's own bytes at a constant offset (no base register: the same cell
on every trip), the pass asks of every instruction that writes or may write
memory whether it leaves the cell alone (`memoryssa::spares`: alias proves
its writes disjoint from the load's bytes). The ids of those writes are
attached to the load as metadata `!llrm.spares`, operands that are the
instruction ids.

```text
  %v = load i16, ptr @g, !llrm.spares !{i32 7, i32 12}
  ...                        ; %7 and %12 write memory, never @g
```

Selection turns each (load, write) pair into the pair of the selected
instructions' positions (`Body::spares`). The spiller reads them
(`_keeps`): a spilled load is rematerialized past a write that spares its
cell, where the LIR cannot place the write's address itself.

## Runs
Once, last in [`crate::pipeline`](pipeline.md) with `homes`, so the instruction ids it
names are the ones selection sees.

## Needs
`memoryssa::Accesses`, `cfg::Shape`, the program's alias data.

## See also
LLVM's `MachineMemOperand` keeps the IR value for the same use, alias
queries at the machine level (`MachineInstr::mayAlias`).
