> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/intbits.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/intbits.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/intbits.rs sha256:eaa7727e2e70f7f27b25733f1c82977aa13dd9c5e15166ac6e41ecb2c99d7bbe -->

# intbits

Integers of a given width held in an `i64`: the masks and wraps that stay
defined at eight bytes, where `1 << 64` is not.

## Example
`low_mask(4)` is `0xffff_ffff`, `low_mask(8)` is `-1`, and
`wrap_to(200, 8)` is `-56`.
