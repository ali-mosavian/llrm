> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/indvars.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/indvars.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/indvars.rs sha256:016c385504a6e48bdcb7c83dd3d2411ef4f626c2be8c37252856b89f1880aa44 -->

# indvars

IndVarSimplify's exit rewriting reused across an outer loop: llrm-core's
`optimize/indvars.rs`, the port of `qbopt/optimize/indvars.py`, adapted
to the rich MIR. `widened` is `widenIV` for a counter that needs no sign
proof. `rewound` carries an exact inner recurrence's exit value
around the outer loop, less its distance, in place of its saved start.
Which counters a loop keeps, and which ends it, is `lsr`'s.
