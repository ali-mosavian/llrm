> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_use.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_use.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_use.rs sha256:809b13699145578317e13cad03ece3d8d68ff88aa7c166c4b2535fed0cdab67f -->

# lsr_use

The uses and the price of LLVM's loop strength reduction: `LSRFixup`,
`LSRUse`, `Cost` and the addressing-mode checks they rate by
(LoopStrengthReduce.cpp at `d1106deb71cc`, l.1070-2130).

## How it works
An `LsrUse` is one thing the loop computes (an address, a compare against
zero, a plain value) with the `Formula`s that could compute it. `Cost` rates
a formula against the registers already chosen for the other uses: registers
first, then add recurrences, IV multiplies, base adds, scale, immediates and
the setup in the preheader. The target answers what an address mode folds
(`Tti`).

## Example
`p[i] = 0` in a loop is an address use with one fixup at offset 0; its
formula `{%p,+,4}` costs one register and one recurrence (1 instruction).

## Runs
Nothing runs it yet: stage 2 of the translation of LSR (the old pass in
[`crate::lsr_pass`](lsr_pass.md) is the pass that runs them).

## Limits
DEVIATE: the target hooks have no access type or address space on the
legality call, and no per-instruction query (`LSRWithInstrQueries`); x86 has
neither. `getPreferredAddressingMode` is none on x86, so the pre- and
post-indexed branches of `RateRegister` never apply and are not ported. A
scalable `Immediate` does not exist (see [`crate::lsr_formula`](lsr_formula.md)).
