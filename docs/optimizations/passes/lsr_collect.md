> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_collect.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_collect.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_collect.rs sha256:c94a75367c37c2a53dc4bf2da77c834fbebe47b9228fb9cac0ce90e50151a3bd -->

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
DEVIATE:
1. No IV chains (`CollectChains`, `isHighCostExpansion`'s chain users),
   `OptimizeShadowIV` and `OptimizeLoopTermCond` yet; the later stages of
   stage 3. No atomic accesses, funclet pads, scalable immediates or target
   memory intrinsics (MIR has none). `canSaveCmp` is false on x86. The swap
   of a compare's operands (`setOperand`) is recorded in `swapped_compares`
   for the caller to make: the analyses hold the function shared.
2. A callee is named from the module's globals the pass was handed, not the
   live module: a function declared during the compile (`runtime.nib` cut to
   a program, #3140) has no name there, so an `llvm.lifetime` call declared
   that way is not skipped by `collect_loop_invariant_fixups_and_formulae`
   and stays a use (worse code at most; the pin reads the callee's intrinsic
   id). Exit: the snapshot sees declarations added during the compile.
   Boundary: `a_call_to_a_global_the_outer_snapshot_lacks_is_no_intrinsic`.
