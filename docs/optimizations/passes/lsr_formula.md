> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_formula.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_formula.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_formula.rs sha256:693b00331ac49936ec9ffbedd513fa8e7a6d8130858b609fea4749ba9d2f66d8 -->

# lsr_formula

The data of LLVM's loop strength reduction: `Immediate`, `RegUseTracker`
and `Formula` (LoopStrengthReduce.cpp at `d1106deb71cc`, l.212-793).

## How it works
A `Formula` is one way to compute an address or a value for a use:
`BaseGV + BaseOffset + sum(BaseRegs) + Scale * ScaledReg + UnfoldedOffset`,
each register a SCEV. `canonicalize` keeps it in the shape the generators
and the solver expect. `RegUseTracker` says which uses reference which
register.

## Example
`p[i + 2]` with 4-byte elements: base register `p`, scaled register `i` with
scale 4, base offset 8.

## Runs
Nothing runs it yet: stage 1 of the translation of LSR (the old pass in
[`crate::lsr_pass`](lsr_pass.md) is the pass that runs them).

## Limits
DEVIATE: `Immediate` is fixed. LLVM's also holds a scalable quantity
(`vscale * n`) for SVE and RVV; no target here has scalable vectors, and
`ScalarEvolution` has no `vscale` node.
