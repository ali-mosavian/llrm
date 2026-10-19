> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_formula.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_formula.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_formula.rs sha256:bf7695b968f00c1a9b273ddb4096f0bd0fe111c1cd53f8a4e9d69e9066fdf4c4 -->

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
[`crate::lsr`](lsr.md) is gcc's ivopts and stays the default).

## Limits
DEVIATE: `Immediate` is fixed. LLVM's also holds a scalable quantity
(`vscale * n`) for SVE and RVV; no target here has scalable vectors, and
`ScalarEvolution` has no `vscale` node.
