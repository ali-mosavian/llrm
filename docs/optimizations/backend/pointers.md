> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/pointers.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/pointers.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/pointers.rs sha256:f95a15358ee4bae30f4a0fc8c62533c0ee27f6af376062b0dfe59fb947ae6784 -->

# pointers

Advances a packed huge pointer by a displacement as arithmetic on its 32-bit
form.

## How it works
A huge pointer is selector:offset in one dword, and an offset that carries
moves the selector by the ABI's stride (`HugeShift`: a constant 0..16, or a
byte cell the runtime sets). `Model::offset` returns the operations that
compute `result = pointer + displacement` with that carry corrected:

```text
 pages = ((pointer & 0xffff) + displacement) >> 16
 high  = ((pages << shift) - pages) << 16
 result = (pointer + displacement) + high
```

The plain dword sum already adds one selector unit per page crossed; `high`
adds the other `(1 << shift) - 1` units. Eight operations for a constant
stride, nine with a cell (one more to load it), none of which rebuilds the
two halves. The caller supplies `fresh`, a source of new value numbers.
Pointer, displacement and result must be dword-wide, and a cell stride must
be a byte.

## Example
Advancing `v1` by `v2` into `v3` with a constant stride of 12 (fresh values
count from `v11`):

```lir
v11 := and v1, 0xffff
v12 := add v11, v2
v13 := shr v12, 16
v14 := shl v13, 12
v15 := sub v14, v13
v16 := shl v15, 16
v17 := add v1, v2
v3  := add v17, v16
```

With `v1 = 0x2000fffe` and `v2 = 2` the offset crosses a page, and `v3` is
`0x30000000`: the selector moves by `1 << 12`.

From unit test `test_huge_pointer_advance_does_not_rebuild_both_halves` in
`pointers.rs`, which asserts the result and at most eight operations.

## Used by
Nothing outside the file's own tests: a grep finds no caller. Isel states
the carry itself (see `docs/machine/pointer-lowering.md`).
