> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_generate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_generate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_generate.rs sha256:78bb160a9d9e2063e1be28768c58fcb306d00eb0da704e04b88a3a7fafeb3ed4 -->

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
DEVIATE: the pre-indexed addressing branches (`AMK_PreIndexed`; no target
of ours prefers it) are not ported; the post-indexed ones are.
Work per use is bounded by the formulae it has, which the caps bound.

DEVIATE LG1: `GenerateICmpZeroScales` asks `getExactSDiv` for no overflow
of the product, but for an odd factor `x == 0` and `x * c == 0` are the same
compare (multiplying by it is a bijection mod 2^bits), so it ignores the
significant bits there, for the counter alone compared to zero. The pin
keeps a loop's exit counter beside the scaled one it is a multiple of
(recsum: `dec; add -3; jne`); this exits on the scaled one (`add -3; jne`),
as the stand-in LSR did. Exit: none, a proof; if LLVM does the same, delete
the line.

DEVIATE LG2: for an even factor `c`, `getExactSDiv` fails where the compared
value `x` stays below 2^(bits - ctz(c)) under the loop's guards: `x * c == 0`
iff `x` is a multiple of that, so only 0 is. Without it a byte-stepped loop
kept a second counter beside the byte index (examples/dot m16: `dec cx`, a
32-bit spill). Proof: `the_even_factor_zero_rule_matches_the_wrapped_product_exhaustively`
(i8 all factors, i16 bounded). Exit: delete the check if the pin's SCEV proves
the same sext through `isAddRecSExtable`.

DEVIATE LG3: an address base that is an addrec of the loop, `R = r0 + r*i`, also
gets the form `(r0 - r*z0) + r * Z` for each exit counter `Z = {z0,+,z}` that
ends at zero when `r` divides `z` and `r` is a legal scale: the address shares
the exit counter, as gcc ivopts does (`ubase - ratio*cbase`). The pin has no
such formula and keeps a pointer walk beside the counter (examples/dot m16:
two address walks and a counter; 854 shares one). Exit: none, a gcc
translation; if LLVM gains it, delete `generate_end_biased`.
