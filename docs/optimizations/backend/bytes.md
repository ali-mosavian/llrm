> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/bytes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/bytes.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/bytes.rs sha256:2155c04ef6e5f25414c09cda27a09011fec0d617627bef16b0c1e9d6d392b617 -->

# bytes

Expands `llvm.bswap` into shifts, masks and ors in MIR before instruction
selection, for the targets whose description lists `bswap` under `expand`.

## How it works
Byte `i` of an `n`-byte integer moves to byte `n - 1 - i`: a left shift by
`(n - 1 - 2i)` bytes when that is positive, a right shift by its negation
otherwise, then a mask of the byte's new place. The bytes are or-ed. Two
shifts and masks, an or, per pair of bytes: linear in the width, and no
multiply.

```text
 %r = call i16 @llvm.bswap.i16(i16 %x)    ; (x << 8) | (x >> 8)
```

## Example
`bswap` of an `i16`:

```mir
define i16 @f(i16 %x) {
  %r = call i16 @llvm.bswap.i16(i16 %x)
  ret i16 %r
}
```

After, in the order the instructions are made:

```mir
define i16 @f(i16 %x) {
  %1 = shl i16 %x, 8
  %2 = and i16 %1, -256
  %3 = lshr i16 %x, 8
  %4 = and i16 %3, 255
  %5 = or i16 %2, %4
  ret i16 %5
}
```

From unit test `a_byte_swap_is_shifts_masks_and_an_or` in `bytes.rs`.

## Runs
Once per module in `driver::optimized`, with the bit counts, for a target
that lists `bswap` under `expand` (m16 and m32 x86: no `bswap`/`xchg` forms
are selected for it yet). A call on a target that does not list it reaches
selection, which refuses it by name.

## See also
LLVM's `TargetLowering::expandBSWAP`
([TargetLowering.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp)),
which does the same with fixed forms for `i16`, `i32` and `i64`.
