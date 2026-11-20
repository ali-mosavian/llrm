> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/gepbyteindex.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/gepbyteindex.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/gepbyteindex.rs sha256:a78c90c147163d63c732e58e0fcac57b4e2777cc5488d0e70d0f56c59c2ff9b6 -->

# gepbyteindex

The index of an `inbounds` `gep` into a small global is a byte.

## How it works
`getelementptr inbounds [4 x i8], ptr @g, i16 (zext i8 %x)` is `@g + 4 * %x`
in a word. An `inbounds` access stays inside its object, so where `@g` is no
larger than the target's one-instruction index reach (`Tti::address_index_max`,
the 6502's `abs,x`: 255) the offset is a byte: `%x * 4` does not wrap in
`i8`, and a trip that would wrap is poison already. The `gep` becomes
`gep inbounds i8, ptr @g, i16 (zext (mul nuw i8 %x, 4))`, plus a `gep` of
the constant part of its indices, so isel finds `global + zext i8` and forms
`lda g,x`, not a 16-bit shift and add in a pointer pair.

Only a `gep` with one variable index, the zero extension of a byte, and
constant others; a scale of 1 is already a byte index. O(1) per `gep`.

DEVIATE GEP-BYTE-INDEX (no pin pass narrows by object extent: InstCombine's
`shl (zext X), C -> zext (shl X, C)` needs known-zero high bits of `X`
(`visitShl`, InstCombineShifts.cpp#L1175), and the DAG's `SimplifyDemandedBits`
cannot narrow an address index whose bits are all demanded; the extent is the
one `getBaseObjectSize` reads for `llvm.objectsize`; owner llrm-target-6502,
exit 2026-11-30: the 6502 backend narrows an address index from the object).

## Example
```text
 %z = zext i8 %x to i16                    %m = mul nuw i8 %x, 4
 %p = gep inbounds [4 x i8], @g, i16 %z    %z = zext i8 %m to i16
 %v = load i8, ptr %p                 ==>  %p = gep inbounds i8, @g, i16 %z
                                           %v = load i8, ptr %p
```

## Runs
In the strength slot after `gepoffset`; option `gepbyteindex`; only for a
target that sets `Tti::address_index_max`.
