> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/executed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/executed.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/executed.rs sha256:9683dc6469ae9a74fb3cf14b29ed7aab0a6e9a977f7a1def10c707f59f7885ae -->

# executed

A finished body's executed work, estimated without a profile.

Each block's expected executions per call (`analysis::frequency`: the
branch heuristics, a loop's proven trips) weigh its instructions and memory
operands, so spill code the MIR estimate cannot see is counted. On the
placed body, each edge's odds say how often a branch is taken.
