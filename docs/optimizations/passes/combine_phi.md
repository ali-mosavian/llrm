> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_phi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_phi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_phi.rs sha256:b0a7fb93beb1f5b8e8757c1bf72006a295441ae8d1606ee225c1664fd40c1787 -->

# combine_phi

`foldOpIntoPhi`: an instruction that reads a phi becomes a phi of the
instruction applied to each incoming value.

Translation of
[`InstCombinerImpl::foldOpIntoPhi`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L1943)
and `simplifyInstructionWithPHI` (:1859) at d1106deb71cc.

MIRROR: the phi must have one user, or only users identical to the
instruction; every other operand must be phi-translatable; each incoming
value either simplifies with the operation applied, or (for at most one, on
an unconditional, reachable, non-back edge) gets a copy of the operation at
the end of its block; the implied-condition constant of an `icmp` on a
branch's edge.

DEVIATE PHICONST (owner x87, exit 2026-11-14): a phi of constants that
has several different users is folded into all of them at once when every
user is a binary operator or cast that simplifies to a constant on every
incoming. The pin refuses (the one-user rule above), so each user
reads the phi and runs at run time; gcc folds them (x_cordic -O3 at m32:
`ashr (phi [-39797, 39797]), 1` and `add (phi ..), 19898` ran 1746
instructions more than gcc's per-arm constants). N users become N phis of
constants and no instruction is copied; the old phi has no user left. Work
is O(users x incoming). DEVIATE PHICONST-2 (same owner and exit): an
incoming that is itself a phi of constants of its own block, read by this
phi alone (a diamond that picks the constant), folds the same way, the user
applied to each of its constants in a phi there; and in a loop the users
that fold fold while the others stay on the phi (x_cordic -O3: `add %24,
%27` beside the `ashr %24, 2` that starts the chain, 28,326 -> 27,526
clocks, same bytes); outside a loop a user left on the phi leaves all of
them (the shared softfloat routines grew 6 to 8 B for no clock). Only where
bytes are priced (-O2, -O3): a phi of constants is a move per edge, and at
-O1/-Os the corpus grew by 2 to 12 B (m16 float_in_place_update -O1 +12, m32
-Os +2) with no cell smaller.

DEVIATE 1: the `icmp` of a `ucmp`/`scmp` intrinsic with a constant (the
first `WillFold` case) is not made: MIR has no `ucmp`/`scmp`.

DEVIATE 2: the implied-condition test takes the compare's predicate without
`samesign` (MIR compares do not carry it), so it can only find less.

DEVIATE 3: no debug-use rewrite (`replaceAllDbgUsesWith`): `replace_value`
moves debug records of the phi with its uses.

`foldBinopWithPhiOperands` (:2215) and `foldBinopWithRecurrence` (:2100):
a binary operator of two phis of one block.

MIRROR: two simple recurrences of one operation, with constant starts and
steps, become one recurrence (`reduced.phi`); an identity constant on one
side of every incoming pair leaves the other side; a pair of constants for
one predecessor folds there and the operation moves to the other
predecessor, which must jump straight here.

DEVIATE 4: `IRBuilder` here simplifies what it makes (`create_binop`), not
just constant-folds, as everywhere in `combine_build`.

## Used by
The `visit*` of the families that fold an operation into a phi
(`foldBinOpIntoSelectOrPhi`, `visitICmpInst`, `visitCastInst`...).

## Example
```text
%p = phi i16 [ 1, %a ], [ 2, %b ]          %q = phi i16 [ 6, %a ], [ 7, %b ]
%r = add i16 %p, 5                   ==>   (uses of %r read %q)
```
