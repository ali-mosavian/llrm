> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/peel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/peel.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/peel.rs sha256:c3f446819a3fa00b50b19edd645efe3602a2e1d28dd005da3378dd15180875df -->

# peel

Exact CFG loop peeling, priced before anything is cloned: llrm-core's
`optimize/peel.rs`, the port of `qbopt/optimize/peel.py`, adapted to the
rich MIR.
LLVM: LoopPeel (`peelLoop`) peeling every trip, priced by LoopUnrollPass's
full-unroll cost model.

Peeling is the CFG counterpart of full straight-line unrolling: clone
every block of a proven exact loop once per trip, keep the residual loop
as a correctness fallback, and let Fold, Decide and Dead prove it
unreachable.

The copy budgets are peelsize's `Limits`, which count instructions. The
one machine number is the target's costs: a function with an instruction
the target does not price is left alone (`profit::priced`).

What changed with the IR: the old stage records and `watch` hook are the
pass manager's dump and change log. Dropped:
`MAX_CONDITIONAL_FLOAT_OPERATIONS`, a limit on the x87 strict-FP fixed
point a floating branch crossed; a float is an ordinary value here.
