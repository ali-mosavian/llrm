> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/algebraic.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/algebraic.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/algebraic.rs sha256:a3fcde5922df6ec94c3b3e6fdd2f1c50d8dae1da4562384bb111321b926735c9 -->

# algebraic

Algebraic identities over integer values, as LLVM's InstCombine and, for a
loop-carried operand, Reassociate.

## How it works
`simplified` repeats a round of rules until none fires. A round runs, in
order: the rules that need an analysis (`_whole_fixed`,
`_unsigned_divisions`, `_divisions`, `_redundant_masks`, `_unread_bits`),
then the first matching local rule on each instruction (`_rewritten`), then
the loop-carried reassociation (`_reassociated_recurrences`) and
`_shared_shifts`. A repeated addend (`_doubled_addend`) is tried last, only
when nothing else fired, since the other rules may cancel it (`x - (x + 5 +
x)`).

Every rewrite holds modulo 2^width, so a rewritten instruction drops `nsw`,
`nuw`, `exact` and `disjoint`: a reassociated sum may overflow where the
original did not. The rules fall in groups.

- Constants and chains: neutral terms (`x + 0`, `x * 1`, `x & -1`), `x * 0`,
  `x % 1`, decided self-compares, a `select` on a constant; two single-use
  shifts, scales, offsets or bitwise constants become one; `(x + a) * k + b`
  distributes; `(x + a) + a` is `x + (a << 1)`; `x - (x + y)` is `-y`.
- Casts: `trunc(ext x)`, `ext(ext x)`, a mask under an extension, a mask
  over a `sext` that reads only the low bits (`and (sext (trunc v)), M` is
  `and v, M`; `and (sext x), 2^N-1` over an N-bit `x` is `zext x`),
  `inttoptr (add x, y)` of a flat pointer as `gep i8, (inttoptr x), y` (a
  constant `y` not under `size`), `sext` of a value whose sign bit is clear,
  logic or a phi of like extensions done at the narrow type,
  `ptrtoint(inttoptr x)`, a byte offset from a constant near address as the
  constant address.
- Compares: `icmp P (x + C1), C2` is `icmp P x, C2 - C1` (an ordered test
  needs `nsw` or `nuw` for its signedness); a compare of extensions is a
  compare of the sources; `xor (icmp P a b), true` is the inverse compare;
  `icmp ne (sext i1 x), 0` is `x`.
- Divisions: `udiv` and `urem` by a power of two are a shift and a mask;
  `sdiv` and `srem` of a dividend proved non-negative (`ranges`, or a clear
  sign bit) are unsigned, by a power of two always, by another constant at
  any width and target but under `size`, where the signed form is a byte
  shorter (selection prices the reciprocal, prefixes included); other `sdiv`
  and `srem` by a power of two are an `ashr`, a bias and a shift; a
  fixed-point product or quotient by a whole number cancels its scale.
- Masks and demanded bits: `and x, C` where `C` covers every bit `x` may set
  is `x`; a mask, or an `or`, that touches only bits nothing reads is its
  operand.
- Phis: a phi repeating an earlier phi of its block from the same
  predecessors is that phi.

A counter carried round a loop is put at the root of its `add` tree, so the
inner sum can be invariant and leave the loop:

```text
 loop:  %i = phi [ %i0 ], [ %n, latch ]        %i = phi [ %i0 ], [ %n, .. ]
        %t = add %i, %a                  ==>   %s = add %a, %b   (invariant)
        %n = add %t, %b                        %n = add %i, %s
```

A mask whose operand advances by a constant each trip moves after its scale,
`(x & m) * 2^k` as `(x * 2^k) & (m * 2^k)`, so the scale can fold into the
loop's step.

## Example
Before:

```mir
define i16 @f(i16 %x, i16 %a) {
b0:
  %m = add i16 %x, %a
  %r = add i16 %m, %a
  ret i16 %r
}
```

After:

```mir
define i16 @f(i16 %x, i16 %a) {
b0:
  %0 = shl i16 %a, 1
  %1 = add i16 %x, %0
  ret i16 %1
}
```

A value added twice is added once, doubled.

From unit test `test_a_value_added_twice_is_added_once_doubled` in
`algebraic_tests.rs`.

## Runs
`algebraic` runs once in each pass of the fixed point, after `indvars` and
before `dead`; it has no flag of its own, so it is on whenever the optimizer
is. It runs again after the loop-header copy (`-ftree-ch`, not at -Os). The
`size` option is set at -Os and -Oz.

`Differences` is a smaller pass of this file, run after `lsr`: it applies
only the rules `x - (x + y)` is `-y` and a negated difference, to clean
bounds subtracted from counters LSR rebased, without folding the scales LSR
just chose back together (LLVM's LSR cleans with InstSimplify for the same
reason). It is skipped with `-fno-strength-reduce`.

## Needs
`ranges` (non-negative dividends, redundant masks), `DemandedBits` (unread
bits), `Registers` and `consts` (divisors known through phis), `induction`
(recurrences, over the manager's `Shape` and `Registers`, which a round that
edited the body invalidates).

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved; other
analyses are invalidated when anything changed.

## See also
LLVM InstCombine (`visitSub`, `foldICmpAddConstant`, `visitTrunc`),
Reassociate, CorrelatedValuePropagation `processAnd`, SimplifyDemandedBits;
gcc `fold_comparison`.
