> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/indvars.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/indvars.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/indvars.rs sha256:b2ea407f57d0110edcd11633eb8a5561b6a08890e4e945c62cc91438e68a26ed -->

# indvars

IndVarSimplify's exit rewriting reused across an outer loop: llrm-core's
`optimize/indvars.rs`, the port of `qbopt/optimize/indvars.py`, adapted
to the rich MIR. `widened` is `widenIV` for a counter that needs no sign
proof. `rewound` carries an exact inner recurrence's exit value
around the outer loop, less its distance, in place of its saved start.
Which counters a loop keeps, and which ends it, is `lsr`'s.
