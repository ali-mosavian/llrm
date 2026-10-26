> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/timing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/timing.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/timing.rs sha256:0aa383e260ee6dae41ee29f3cfd7dc4fcf27269bd8a7773d32d632f6b8a3691a -->

# timing

Core-clock bounds of multiplies and divides, read from the CPU profile, so
the compiler can pick a multiply or divide form by cost.

## How it works
A CPU description states a price per key (`smul_min_w4`, `smul_max_w2`,
`sdiv_w4`, `udiv_w2`, ...). This module reads them and returns `Clocks`, a
`minimum` and `maximum`, or none where the profile has no such key or the
width is not 2 or 4 bytes. A CPU the description does not price yields none,
and the caller does not use the form that needed it.

A multiply that ends early on a short multiplier (the 486's `10 + max(bits,
3)`) is priced by linear interpolation from the minimum at 3 bits to the
maximum at the full width. A negative multiplier takes the profile's
`smul_negative_bits` as the floor. An unknown multiplier is priced at the
middle of the range here; a division by a constant prices its unknown
dividend at the minimum, as gcc does (`division`).

## Example
The bounds `signed_multiply` returns for a full product, in core clocks:

```text
cpu   width   minimum   maximum
386   2       9         22
386   4       9         38
486   2       13        26
486   4       13        42
P5    2       11        11
P5    4       10        10
```

A 486 dword multiply may take 42 clocks, so a reciprocal sequence for a
dividend known to need 32 bits is compared against 42.

From unit tests `test_full_product_bounds_follow_operand_width` and
`test_486_reciprocal_is_priced_by_the_dividend_s_length_and_by_the_minimum_where_unknown`
in `timing.rs`.

## Used by
[`crate::backend::division`](division.md) (multiply by reciprocal against a divide) and
[`crate::backend::arithmetic`](arithmetic.md) (constant multiply chains against `imul`).
