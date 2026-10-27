> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/avail.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/avail.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/avail.rs sha256:d4dc798d2f378de90df5dfb7555c0c374fed8c895a8b78f4c2265440ec097665 -->

# avail

The cells loads and stores name: what a load reads and lands in, what a
store writes.

## How it works
The memory model is `MemRef` cells and [`crate::memoryssa`](memoryssa.md)'s `Accesses`.
Load forwarding is `llrm-transforms`' `gvn` (MemoryDependence), not here;
dead stores are `deadstore` there (LLVM's DSE).

## Example
A load of a cell, and the value it lands in:

```mir
%v = load i16, ptr %p   ; `loaded_into`: the cell of %p, and %v
store i16 %v, ptr %q    ; `stored_from`: the cell of %q, and %v
```

## Provides
The accessors `loaded_into`, `stored_from`, `stored_cell`.

## Needs
[`crate::memoryssa`](memoryssa.md) (`Accesses`).

## See also
LLVM `MemoryDependenceAnalysis`, GVN `processLoad`.
