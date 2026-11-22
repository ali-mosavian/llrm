> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mosindexiv.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mosindexiv.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mosindexiv.rs sha256:fb73f9fd5c788ba72dc1293a1af472593188636238d251bb358b4be8b8981b1b -->

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

## Example
`sum += buf[i]` for `i` in `0..200` over a 300-byte global, 16-bit
pointers, `index_iv_width` 8. Before:

```mir
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %p = getelementptr inbounds i8, ptr @buf, i16 %i
  %b = load i8, ptr %p
  ...
  %next = add nuw nsw i16 %i, 1
  %c = icmp ult i16 %next, 200
```

After:

```mir
loop:
  %0 = phi i8 [ %3, %loop ], [ 0, %entry ]
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %1 = zext i8 %0 to i16
  %2 = getelementptr i8, ptr @buf, i16 %1
  %b = load i8, ptr %2
  ...
  %next = add nuw nsw i16 %i, 1
  %c = icmp ult i16 %next, 200
  %3 = add nuw i8 %0, 1
```

The address is the global plus a byte counter, so the backend indexes with
`lda buf,x`. A 300-trip walk reaches offset 299 and is left as it was.

From unit test `test_a_byte_range_walk_is_a_base_and_a_byte_index` in
`mosindexiv_tests.rs`.

## Used by
[`crate::pipeline`](pipeline.md) runs it before the late `indvars`, where
`Tti::index_iv_width` is set.

## See also
[`crate::narrowiv`](narrowiv.md) (a counter, not an address).
