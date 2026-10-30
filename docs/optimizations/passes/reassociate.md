> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/reassociate.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/reassociate.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/reassociate.rs sha256:9530ad2772aef8f05184224b0339cc73ae63953df7b1654ab872db7904f28617 -->

# reassociate

Reassociate: put commutative expressions in an order that promotes
constant folding, CSE, LICM and PRE.

Translation of LLVM's
[`Reassociate.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Reassociate.cpp)
at d1106deb71cc, `ReassociatePass::runImpl` (:2737) and `OptimizeInst`
(:2292), `ReassociateExpression` (:2442).

MIRROR: blocks in reverse post-order; each instruction is optimized once
(a dead one erased), the instructions the pass queued are then visited
again until none is left; a tree of one associative operation is
linearized, its leaves sorted by rank and optimized as a whole, and written
back into the nodes it came from.

DEVIATE 1: no uniformity analysis. LLVM boosts the rank of divergent
leaves; a target without divergence (every llrm target) has none.

DEVIATE 2: no vectors, and no debug locations (MIR carries none on an
instruction); a value's name is not carried to the instruction replacing it.

DEVIATE 3: a value id is never reused, so the pair map has no stale-key
check (`PairMapValue::isValid`).

DEVIATE 4: MIR has no constant `sub`: `NegateValue` of a constant
expression is a `sub 0, C` instruction (see `reassociate_negate`). Exit: a
constant `sub` in MIR.

DEVIATE 5: a constant has no use list, so the negates of one are kept in
`constant_negates`, to find the one `NegateValue` reuses.

DEVIATE 6: `reassociate` reports a change only if the body differs, not
`MadeChange` (which is also true for an add swapped and swapped back). The
pipeline repeats its body until no pass changes it, so a pass that reports
a no-op never settles it. Exit: the pipeline's rounds compare body
fingerprints themselves (as its cycle history already does), and
`reassociate` returns `MadeChange`.

## How it works
The files are split by job: `reassociate_rank` (ranks),
`reassociate_tree` (linearize and rewrite a tree), `reassociate_optimize`
(the identities of one operation), `reassociate_negate` (subtract, negate,
shift and distribute into forms that reassociate), `reassociate_pairs`
(which pair to form first for CSE). This file is the driver and the shared
helpers; each of the others is an `impl` block of `Reassociator`.

## Used by
The pipeline's `reassociate` slot.

## Example
`(x + 5) + 4` is `x + 9`; `(a * b) + (a * c)` is `a * (b + c)`.
