> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/fixednarrow.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/fixednarrow.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/fixednarrow.rs sha256:eff6eace5f06969ca7ebe948af582079f9a2efc94668b74c11a937d0847a4caf -->

# fixednarrow

A 32-bit fixed-point product whose operands' product fits 32 bits is a `mul`
and an `ashr`.

## How it works
`llvm.smul.fix(a, b, s)` is `(a * b) >> s` computed in 64 bits. isel lowers
it to the one-operand `imul` and `shrd`, which take EAX and EDX; the narrow
form `mul a, b; ashr s` is `imul r, r` and `sar`. The two are the same value
where the 64-bit product never leaves 32 bits.

The test is on the operands, from `ranges`' interval of each at the call's
block, not on the result: `smul.fix(65536, 65536, 16)` is 65536, but its
product is 2^32 and keeps the wide form. The product's bounds are
`ranges::product` of the intervals; it must lie in `[-2^31, 2^31)`. Only
32-bit products with a constant scale below 32 are taken.

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

## Runs
Once, after the loop-header copy and before `lsr`, which would hide such a
shift inside an induction variable. Not at -O0.

## Needs
`ranges` (scoped intervals), `Registers` and the CFG shape.

## Keeps
Blocks and edges are unchanged; dominators and loops are preserved.
