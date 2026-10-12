> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/nearcode.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/nearcode.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/nearcode.rs sha256:d7297828982a489f88294a3e1d6c6961a272317f5e06353c454d067cc63e0df4 -->

# nearcode

A function a frontend allows near (`nearcode`), only its own module calls,
and only directly, is entered by a near call: its callers share its code
segment, so a return address needs no segment. MIR states linkage and calls;
which instruction enters the code is chosen here, as a target fact.
