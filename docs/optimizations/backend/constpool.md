> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constpool.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constpool.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constpool.rs sha256:5427c4c7a6b18f604874b9378e3e5bd26a5dba4a7d2c69b2a35de5eea453ad08 -->

# constpool

Readonly constants the machine phases put in memory, as GCC's
`force_const_mem`: one datum per distinct bytes, which the driver names
and emits with the module's other readonly data.
