> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_collect.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_collect.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_collect.rs sha256:40a9177b83974e2b6a6f4fa9ddd0a0c1991791ad9fb4bc0f53b4dc1027e3d15f -->

# lsr_collect

What LLVM's loop strength reduction collects before it generates formulae:
the uses and their fixups, the interesting factors and types, and each use's
first formula (`LSRInstance::getUse`, `CollectInterestingTypesAndFactors`,
`CollectFixupsAndInitialFormulae`, `CollectLoopInvariantFixupsAndFormulae`
and the helpers they call; LoopStrengthReduce.cpp at `d1106deb71cc`,
l.795-1125 and l.2817-3960).

## How it works
Each `IvStrideUse` becomes a fixup of an `lsr_use::Use`, found or made by
the use's expression (split into a base and an immediate offset) and its
kind: an address, an equality compare with its other side folded in
(`i == n` as `n - i == 0`), or a plain value. Fixups of one expression share
a use; their offsets widen its range while an address mode still holds
them. A use's first formula is the expression split into its loop-variant
and invariant registers. Invariants that other code in the loop also uses
are added as uses, so the solver sees what they pin in registers.

## Example
`p[i] = 0` with the exit `i + 1 == n`: an address use of `{%p,+,4}` and a
compare use of `{(-1 + %n),+,-1}`; factors 4, -4 and -1.

## Runs
Nothing runs it yet: stage 3 of the translation of LSR.

## Limits
DEVIATE: no IV chains (`CollectChains`, `isHighCostExpansion`'s chain
users), `OptimizeShadowIV` and `OptimizeLoopTermCond` yet; the later stages
of stage 3. No atomic accesses, funclet pads, scalable immediates or target
memory intrinsics (MIR has none). `canSaveCmp` is false on x86. The swap
of a compare's operands (`setOperand`) is recorded in `swapped_compares`
for the caller to make: the analyses hold the function shared.
