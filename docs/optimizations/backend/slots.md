> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/slots.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/slots.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/slots.rs sha256:341873a530acb2852e156c7bb18a85c7e9921dcb93436931d24400b5d0a34e3b -->

# slots

The one rule for sharing a frame slot: two things share when they are never
live together. A slot is a home, the most bytes any holder needs, and what
its holders occupy over time; `spiller` shares spill slots by it and `isel`
the slots of block locals by their lifetimes.
