> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/spares.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/spares.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/spares.rs sha256:68698a82a1fbdf37295009e8336a7a4a0f745ac23e4e1f9ebeb918c30df660ed -->

# spares

Keeps what the optimizer proved of a fixed-cell load for instruction
selection: each write that leaves the load's bytes as they were, as
`!llrm.spares` on the load, so the spiller can rematerialize the load past
those writes.

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

## Example
Before:
```mir
define ... @"_f@3"(ptr nocapture writeonly %0, i16 %1) ... {
  ; ...
b4:
  %10 = phi i16 [ %16, %b4 ], [ 0, %19 ]
  %11 = phi i16 [ %17, %b4 ], [ 0, %19 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ 0, %19 ]
  %12 = load i16, ptr @_g, !tbaa !9
  %13 = getelementptr i8, ptr %0, i16 %lsr.iv1
  store i16 %12, ptr %13, !tbaa !9
  %14 = getelementptr i8, ptr %2, i16 %lsr.iv1
  store i16 %11, ptr %14, !tbaa !9
  %15 = load i16, ptr @_g, !tbaa !9
  %16 = add nsw i16 %10, %15
  %17 = add nsw i16 %11, 1
  %lsr.iv.next = add i16 %lsr.iv1, 2
  %18 = icmp ne i16 %17, %1
  br i1 %18, label %b4, label %20
  ; ...
```
After:
```mir
define ... @"_f@3"(ptr nocapture writeonly %0, i16 %1) ... {
  ; ...
b4:
  %10 = phi i16 [ %16, %b4 ], [ 0, %19 ]
  %11 = phi i16 [ %17, %b4 ], [ 0, %19 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ 0, %19 ]
  %12 = load i16, ptr @_g, !tbaa !9, !llrm.spares !24
  %13 = getelementptr i8, ptr %0, i16 %lsr.iv1
  store i16 %12, ptr %13, !tbaa !9
  %14 = getelementptr i8, ptr %2, i16 %lsr.iv1
  store i16 %11, ptr %14, !tbaa !9
  %15 = load i16, ptr @_g, !tbaa !9, !llrm.spares !25
  %16 = add nsw i16 %10, %15
  %17 = add nsw i16 %11, 1
  %lsr.iv.next = add i16 %lsr.iv1, 2
  %18 = icmp ne i16 %17, %1
  br i1 %18, label %b4, label %20
  ; ...
```
Both loads of `@_g` gain `!llrm.spares`; the dump prints the metadata as
`!{i32 37}` on each, and does not say which instruction has that id.

From `--dump` of `examples/pass-docs/spares.c` (`p[i] = g; a[i] = i; s +=
g;` in a loop, `p` a parameter) at -O2 -m16, stages `cgpaddr` and
`spares`.

## Runs
Once, last in [`crate::pipeline`](pipeline.md) with `homes`, so the instruction ids it
names are the ones selection sees.

## Needs
`memoryssa::Accesses`, `cfg::Shape`, the program's alias data.

## See also
LLVM's `MachineMemOperand` keeps the IR value for the same use, alias
queries at the machine level (`MachineInstr::mayAlias`).
