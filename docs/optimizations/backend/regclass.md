> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/regclass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/regclass.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/regclass.rs sha256:16227ece5a6ecae1a109e69e422a5b9e12031dd453b524da6a0f6afa4e5fd7f7 -->

# regclass

The register class of each value: the one place that states which registers
a value may be in.

Owned here, read by the spiller (`ssaspill`) and the allocator (`allocate`),
`coalesce` and `constrain`: one answer (#441). The target's own sets
(`target::ADDRESSING`, `target::BYTE`, `Segments::selectors`) are the
vocabulary; what an operand needs of them comes from the instruction.
