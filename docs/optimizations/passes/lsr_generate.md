> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_generate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_generate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_generate.rs sha256:d947197f68608b1b17ebd9efaa96032ae174b65cda3ed9e10d102bba3f488022 -->

# lsr_generate

The formulae LLVM's loop strength reduction generates for each use
(`GenerateReassociations`, `GenerateCombinations`,
`GenerateSymbolicOffsets`, `GenerateConstantOffsets`,
`GenerateICmpZeroScales`, `GenerateScales`, `GenerateTruncates`,
`GenerateCrossUseConstantOffsets`, `GenerateAllReuseFormulae` and
`FilterOutUndesirableDedicatedRegisters`; LoopStrengthReduce.cpp at
`d1106deb71cc`, l.3880-5000).

## How it works
From a use's first formula, each generator makes the others the target could
take: an add split into several registers or several combined into one, a
global moved into the address field, an offset moved between the immediate
and a register, a factor of a stride folded into the scale, a wider
induction variable truncated, and a register a constant apart from another
use's. Each formula is offered to `Collection::insert_formula`, which
keeps one per set of registers. The recursion is capped (depth 3, and
sub-expression depth 3), as LLVM's is.

## Example
For the compare `{(-1 + %n),+,-1}`: `%n` with `{-1,+,-1}` scaled, and `(-1 +
%n)` with `{0,+,-1}` scaled. For the store, `%p - 4 + 4*{1,+,1}`, sharing
the compare's register.

## Runs
Nothing runs it yet: stage 4 of the translation of LSR.

## Limits
DEVIATE: the pre- and post-indexed addressing branches
(`AMK_PreIndexed`, `AMK_PostIndexed`; x86 prefers none) are not ported.
Work per use is bounded by the formulae it has, which the caps bound.
