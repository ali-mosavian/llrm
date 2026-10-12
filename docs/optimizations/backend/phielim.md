> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/phielim.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/phielim.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/phielim.rs sha256:488ab470cbae5610889ae343e3355f92f38b90c8977c8bc3772d11e58f67aab5 -->

# phielim

Port of `qbopt/backend/phielim.py`: the pass that takes LIR out of SSA.

Each phi becomes a copy at the end of every predecessor it names. A copy
on a critical edge goes in a block of its own, labelled above physical
offsets in a namespace per body entry. LLVM's `PHIElimination`.
