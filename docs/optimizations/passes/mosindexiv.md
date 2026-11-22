> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mosindexiv.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mosindexiv.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mosindexiv.rs sha256:8026c79f897db740b88d4f7e2d3510e5246c4b873343609d36e5e56743cf50ea -->

# mosindexiv

A pointer recurrence whose step and offset fit a byte becomes a base plus a
byte counter. Translates llvm-mos's `MOSIndexIV`
([MOSIndexIV.cpp](https://github.com/llvm-mos/llvm-mos/blob/main/llvm/lib/Target/MOS/MOSIndexIV.cpp)).

## How it works
For each loop, each `getelementptr` whose SCEV is an add recurrence of the
loop in address space 0 (`Base + {0,+,Step}`) with the signed range of
`Step` and of `{0,+,Step}` both non-negative and below the index width is
rewritten to `gep i8 Base, zext(trunc {0,+,Step})`. The byte counter is a
dedicated phi; the high byte of the index is zero, so the backend can use
the byte-indexed addressing modes.

## Used by
[`crate::pipeline`](pipeline.md) runs it before the late `indvars`, where
`Tti::index_iv_width` is set.

## See also
[`crate::narrowiv`](narrowiv.md) (a counter, not an address).
