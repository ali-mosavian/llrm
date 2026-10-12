> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/slots.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/slots.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/slots.rs sha256:6b43e3246b41a070604999a56b8ccb6960046b6d955e07020c4af3bd12b09814 -->

# slots

The one rule for sharing a frame slot: two things share when they are never
live together. A slot is a home, the most bytes any holder needs, and what
its holders occupy over time; `spiller` shares spill slots by it and `isel`
the slots of block locals by their lifetimes.
