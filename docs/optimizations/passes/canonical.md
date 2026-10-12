> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/canonical.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/canonical.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/canonical.rs sha256:106a832eb78e4195fac43e83b3d83a3f520d4e31dfbc5c61016a1bb5f655f759 -->

# canonical

Canonical forms, as LLVM's InstCombine puts them: compares with the constant
on the right, no neutral terms, and a zero test at the width of what it
tests.

## How it works
Two steps, run together by `Canonical` and, as functions, by `fold` and
`rotate`.

`compares` swaps every `icmp C, v` (an integer constant on the left, a
non-constant on the right) to `icmp v, C` with the predicate mirrored. No
machine compares an immediate against a register in that order, and folding
makes this shape whenever a compare's left operand becomes known. A compare
of two constants is left to folding, and one of two values stays.

`identities` then visits each instruction once:

- `algebraic::identity`: neutral terms (`x + 0`, `x * 1`, `x & -1`, ...) are
  their operand. Rewrites that state a result from a proof in full (a
  rotated loop's trip count is `bound - start + inclusive` for any start)
  leave the neutral terms to this step, so none folds its own.
- a zero test of an extension tests what was extended: `x == 0` and `x != 0`
  of `zext`/`sext` of `y` are the same test of `y`, and of an `i1`, `y != 0`
  is `y` and `y == 0` is `not y`.
- `x <=u 0` is `x == 0` and `x >u 0` is `x != 0`: nothing is below zero.

```text
 %e = sext i1 %go to i8                  br i1 %go, ...
 %c = icmp ne i8 %e, 0         ==>
 br i1 %c, ...                           (the compare induction reads)
```

## Runs
Not a step of the fixed point on its own: `fold` runs `compares` and
`identities` after folding, and `rotate` runs `identities` after rotating a
loop. Both are on at every level above -O0 (`fold` is off only with the pass
disabled by name).

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::algebraic`](algebraic.md), [`crate::fold`](fold.md); LLVM InstCombine.
