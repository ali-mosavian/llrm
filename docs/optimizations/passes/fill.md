> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fill.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fill.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fill.rs sha256:79bfd04f9149194e4692baab4f52e3c6a0dee87bab3fae6a3d4ea9ccfc9db11b -->

# fill

A counted loop that stores one byte value into consecutive bytes is one
`llvm.memset`: LLVM's LoopIdiomRecognize. Adapted from llrm-core's
`optimize/fill.rs`, the port of `qbopt/optimize/fill.py`.

The loop keeps one trip, guarded by its header's test, which fills what
every trip stored; its counters leave with their exit values.

What changed with the IR:
- The fill is `llvm.memset`, of bytes and a byte count: a wider store fills
  only where each of its bytes is one number. A word fill of any other
  value, `rep stosw`, is isel's shape; the rich MIR has none.
- Where each trip stores is induction's `derived` address, a GEP off an
  invariant pointer; its bytes per trip must be the element's. The old
  `_offset` walked adds of a register base, and `_stepping` compared the
  counter's step itself.
- The fill's address is the store's own pointer, which the one trip left
  computes from the counters' starts. The old one rebuilt it from the cell's
  base, displacement, segment and storage class.
- Its count in bytes must not wrap the index: a byte's trips never do, and
  wider cells need `inbounds` GEPs or the proof's `maximum`. The counter
  must be the index's width.
- The memset is declared where the module has none, through the pass
  manager's `Declared`.
- `pure` is `memory::only_value` less loads and allocas, and less divisions,
  which trap.

llrm-mir has no idiom pass.

Straight-line stores of one repeated byte to adjacent bytes of one
object are one memset too: LLVM's MemCpyOpt, `tryMergingIntoMemset`.
