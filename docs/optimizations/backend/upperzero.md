> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/upperzero.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/upperzero.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/upperzero.rs sha256:b7280479791e67ef906826513df45f53ea96ba0debd5008e65bea1ec4cdf6c0d -->

# upperzero

Which 32-bit register roots hold zero in their upper half before each
allocated instruction.

A word or byte write leaves the upper half as it was; `movzx`, `xor r,r`
and a `mov` of a word-sized constant set it to zero; any other write of
the upper lanes, and anything whose effects are unknown, loses it. The
entry knows nothing. A 32-bit effective address names the same byte as
the 16-bit one only through registers this proves.
