> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/nearcode.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/nearcode.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/nearcode.rs sha256:8f586fd2b341ee96467ab37923c0da1bc922b4bb1de059d3bcaed242c42a18c0 -->

# nearcode

A function a frontend allows near (`nearcode`), only its own module calls,
and only directly, is entered by a near call: its callers share its code
segment, so a return address needs no segment. MIR states linkage and calls;
which instruction enters the code is chosen here, as a target fact.
