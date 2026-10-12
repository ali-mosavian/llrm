> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/pointers.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/pointers.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/pointers.rs sha256:debd5965c9e92030b62e5044f8e600b4c1b55d73df4daec3ed12bcaf26f67198 -->

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
adds the other `(1 << shift) - 1` units. Nine operations in all, none of
which rebuilds the two halves. The caller supplies `fresh`, a source of new
value numbers. Pointer, displacement and result must be dword-wide, and a
cell stride must be a byte.

## Used by
Nothing outside the file's own tests: a grep finds no caller. Isel states
the carry itself (see `docs/machine/pointer-lowering.md`).
