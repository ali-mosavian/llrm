> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/timing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/timing.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/timing.rs sha256:ba8a5c2aa14ba65c53bab15a9c1d2373f416ef0b68d7a224939d071a46382824 -->

# timing

Core-clock bounds of multiplies and divides, read from the CPU profile.

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
middle of the range.

## Used by
[`crate::backend::division`](division.md) (multiply by reciprocal against a divide) and
[`crate::backend::arithmetic`](arithmetic.md) (constant multiply chains against `imul`).
