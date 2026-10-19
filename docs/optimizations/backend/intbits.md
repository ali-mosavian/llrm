> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/intbits.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/intbits.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/intbits.rs sha256:bdf30fea22c072f0496166ee95854d56f21a9d8a505f452638e5854d741ec2b4 -->

# intbits

Integers of a given width held in an `i64`: the masks and wraps that stay
defined at eight bytes, where `1 << 64` is not.

## How it works
Each helper takes the width of the integer (bytes for a mask, bits for a
bound or a wrap) and returns the `i64` a shift by that width would, except
at the full width, where the result is the whole `i64`: no shift by 64 is
made.

## Example
```text
low_mask(4)                = 0x0000_0000_FFFF_FFFF
low_mask(8)                = -1            (1 << 64 would overflow)
signed_min(32)             = -2_147_483_648
signed_min(64)             = i64::MIN
wrap_to(0x1_0000_0001, 32) = 1
wrap_to(200, 8)            = -56
```

From unit test `eight_bytes_are_the_whole_integer`.

## Used by
`stepflags` (the constant of a compare against a step) and `peephole`
(known constants and address chains).
