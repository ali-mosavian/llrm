> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fixednarrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fixednarrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fixednarrow.rs sha256:7aeb1cb1d4081ab444770bbcce05409734d7473a65c9b9263cde99b2c123f774 -->

# fixednarrow

A 32-bit fixed-point product whose operands' product fits 32 bits is a `mul`
and an `ashr`.

## How it works
`llvm.smul.fix(a, b, s)` is `(a * b) >> s` computed in 64 bits. isel lowers
it to the one-operand `imul` and `shrd`, which take EAX and EDX; the narrow
form `mul a, b; ashr s` is `imul r, r` and `sar`. The two are the same value
where the 64-bit product never leaves 32 bits.

The test is on the operands, from LazyValueInfo's `getConstantRange` of each
at the call, not on the result: `smul.fix(65536, 65536, 16)` is 65536, but
its product is 2^32 and keeps the wide form. The product's bounds are
`ConstantRange::multiply` of the two ranges taken in 64 bits; it must lie
in `[-2^31, 2^31)`. Only 32-bit products with a constant scale below 32 are
taken.

DEVIATE 1: not an LLVM pass. LazyValueInfo answers from definitions and
the branches on the edges to the call, not from trip counts or from a
test on a derived value, so a factor bounded only by a loop's escape test
(`examples/mandel`: `zr^2 + zi^2 > 4`) would keep its wide form, where the
`ranges` engine this replaced narrowed it. `Query` adds that engine's
reach for Q-format squares only: a square is not negative and, where the
branch to the call bounds a sum of two squares, at most the bound; a
square at most `M` bounds its factor by `isqrt(((M + 1) << s) - 1)`; and
sums, differences, shifts, products, sign extensions and phis of bounded
values are bounded, a counter `start, start + 1, ...` that leaves on
`next == end` by `[start, end - 1]`.

A factor `v << k` or `v + v` whose interval keeps it from wrapping leaves
the product for the scale: `(v * 2^k * b) >> s` is `(v * b) >> (s - k)`, so
`x * 2 * y` in Q8 is one `sar` by 7, and a fixed-point integer made by `<<
s` and scaled back costs nothing (the scale reaches 0 and the `ashr` is
dropped).

```text
 %x2 = shl %x, 1                          %m = mul %x, %y
 %p = call smul.fix(%x2, %y, 8)     ==>   %p = ashr %m, 7
 (x and y small enough that x*y fits)
```

DEVIATE LP-3 (no pin pass: SelectionDAG expands `smul.fix`
(`TargetLowering::expandFixedPointMul`); exit: isel does it and this is
deleted; owner w-6; review 2026-11-20).

## Example
Before:

```mir
define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
b0:
  %t = add i32 %a, %a
  %p = call i32 @llvm.smul.fix.i32(i32 %t, i32 %b, i32 8)
  ret i32 %p
}
```

After:

```mir
define i32 @twice(i32 range(i32 -1000, 1001) %a, i32 range(i32 -1000, 1001) %b) {
b0:
  %t = add i32 %a, %a
  %0 = mul i32 %a, %b
  %1 = ashr i32 %0, 7
  ret i32 %1
}
```

The doubling leaves the product and the scale drops from 8 to 7; `%t` is
left for dead-code removal.

From unit test `a_doubled_factor_leaves_the_product_for_the_scale` in
`fixednarrow_tests.rs` (fixture `SHIFTED`; the printed text was taken from a
run).

## Runs
Once, after the loop-header copy and before `lsr`, which would hide such a
shift inside an induction variable. Not at -O0.

## Needs
LazyValueInfo (`LviCache`).

## Keeps
Blocks and edges are unchanged; dominators and loops are preserved.
