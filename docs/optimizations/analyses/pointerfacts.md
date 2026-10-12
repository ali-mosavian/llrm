> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/pointerfacts.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/pointerfacts.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/pointerfacts.rs sha256:573a2b07180b882c052b20bfd224862793545c87136ab056aa6f03f92fd21c36 -->

# pointerfacts

Constant byte offsets between pointer values, and disjointness of two
accesses that share a base.

## How it works
`Offsets::relative(pointer)` walks `getelementptr inbounds` chains whose
indices are all integer constants, summing the byte offset with the data
layout (`collect_offset`), and returns the value it stopped at and the
offset from it. It stops at a value that is not such a `getelementptr` (a
parameter, an alloca, a global, a variable index, a step not `inbounds`).
`fixed` is the same through steps that need not be `inbounds`, for when the
address matters and not the object it stays in. A cycle is refused, and so
is an offset that does not fit the pointer's index width.

`inbounds` is the MIR's own proof that the result stays in its base's
allocation, so two pointers that reach one base lie in one allocation. Only
then does `comparable` return their two offsets, and `disjoint` and
`same_bytes` compare the byte ranges `[offset, offset + bytes)` of two
`Location`s. Two unrelated pointer values never prove disjointness.

```text
  %base ----+--- gep inbounds 0  -> %a   (relative: (%base, 0))
            |
            +--- gep inbounds 8  -> %b   (relative: (%base, 8))
  Location{%a, 8} and Location{%b, 4} are disjoint: 0 + 8 <= 8
```

Facts are read from the current SSA on each call, not stored on operands a
later transformation could retarget.

## Provides
`Offsets` (made by `offsets`), `Location`. Consumers: [`crate::memoryssa`](memoryssa.md),
[`crate::interprocedural`](interprocedural.md) (`cannot_fault`) and `argpromotion` in
llrm-transforms.

## Needs
`llrm_mir::datalayout::DataLayout` for offsets and index widths.
