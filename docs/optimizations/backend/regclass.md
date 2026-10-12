> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/regclass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/regclass.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/regclass.rs sha256:ae125588fc939c9c90fd88410ecc5ee479121fb2a32729b96ae5d7ca6a8363c7 -->

# regclass

The register class of each value: the one place that states which registers
a value may be in.

Owned here, read by the spiller (`ssaspill`) and the allocator (`allocate`),
`coalesce` and `constrain`: one answer (#441). The target's own sets
(`target::ADDRESSING`, `target::BYTE`, `Segments::selectors`) are the
vocabulary; what an operand needs of them comes from the instruction.
