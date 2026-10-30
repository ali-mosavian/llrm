> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/canonical.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/canonical.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/canonical.rs sha256:500da40d42f8e944f1b543bd5e93c663f13a04374e923e4c6f44cab2bf3a8d6b -->

# canonical

Canonical forms, as LLVM's InstCombine puts them: compares with the constant
on the right, no neutral terms, and a zero test at the width of what it
tests, so that every later pass matches one shape of each.

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

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  %a = add i16 0, %x
  %m = mul i16 1, %a
  %n = sub i16 0, %m
  ret i16 %n
}
```
After:
```mir
define i16 @f(i16 %x) {
b0:
  %n = sub i16 0, %x
  ret i16 %n
}
```
The neutral `add 0` and `mul 1` are their operands; `sub 0, x` is not
neutral and stays.

From unit test `neutral_terms_fold_on_either_side_but_zero_minus_x_stays` in
`canonical_tests.rs`.

## Runs
Not a step of the fixed point on its own: `fold` runs `compares` and
`identities` after folding, and `rotate` runs `identities` after rotating a
loop. Both are on at every level above -O0 (`fold` is off only with the pass
disabled by name).

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::algebraic`](algebraic.md), [`crate::fold`](fold.md); LLVM InstCombine.
