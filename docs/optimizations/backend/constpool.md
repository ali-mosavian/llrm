> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/constpool.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/constpool.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/constpool.rs sha256:e2522cc026a1ae88716e62f454c54a5173dceedb92ab737a0226de0ae1de6085 -->

# constpool

Readonly constants the machine phases put in memory, as GCC's
`force_const_mem`: one datum per distinct bytes, which the driver names
and emits with the module's other readonly data.
