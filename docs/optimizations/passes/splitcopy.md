> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/splitcopy.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/splitcopy.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/splitcopy.rs sha256:a1132d3b2dbc030cc0b0f8d7952e214e86081dd868c2f44af4b6672d75ea0078 -->

# splitcopy

A `memcpy` into a local nothing else can reach becomes the loads and
stores of the bytes the local is read as.

## How it works
The copy must be an `llvm.memcpy` of a constant, positive length, not
volatile, between two pointers that resolve to a known offset into
different objects, the destination an `alloca`.

The destination's users are walked through `gep` and `addrspacecast`. Each
one must be a non-volatile load, a store into it, a lifetime marker, the
copy itself, or another `memcpy` that writes it. Any other use (passed to a
call, stored as a value, compared, read as the source of another `memcpy`)
leaves the copy whole, because someone else may read bytes nobody named.

The loads inside the copied range are its leaves: an offset and a type
each. Leaves must not overlap unless equal, and no load may straddle the
range's edge. Each leaf becomes a load from the source and a store to the
destination in place of the copy; a byte nothing reads is not copied. A
destination with no leaf drops the copy.

```text
  memcpy(%to, %from, 8)            %v0 = load i16, %from
  %a = load i16, %to        ==>    store %v0, %to
  %b = load i16, %to+4             %v1 = load i16, %from+4
                                   store %v1, %to+4
                                   (the loads of %to stay)
```

The pass repeats until no copy splits. A destination another copy reads is
split once that later copy has been, so a copy of a copy is the loads of
the first source.

## Example
Before:
```mir
define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  %p = getelementptr i8, ptr %from, i16 4
  store i16 7, ptr %p
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
  %a = load i16, ptr %to
  %q = getelementptr i8, ptr %to, i16 4
  %b = load i16, ptr %q
  %r = add i16 %a, %b
  ret i16 %r
}
```
After:
```mir
define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  %p = getelementptr i8, ptr %from, i16 4
  store i16 7, ptr %p
  %1 = load i16, ptr %from
  store i16 %1, ptr %to
  %2 = getelementptr i8, ptr %from, i16 4
  %3 = load i16, ptr %2
  %4 = getelementptr i8, ptr %to, i16 4
  store i16 %3, ptr %4
  %a = load i16, ptr %to
  %q = getelementptr i8, ptr %to, i16 4
  %b = load i16, ptr %q
  %r = add i16 %a, %b
  ret i16 %r
}
```
`%to` is read as two words, at 0 and 4, so the 8-byte `memcpy` is a load and
a store of each; the bytes nothing reads are not copied. `Sroa` then
promotes the words.

From unit test
`test_a_memcpy_into_a_private_local_is_the_loads_of_what_is_read` in
`splitcopy_tests.rs`.

## Runs
Inside [`crate::promote`](promote.md)'s `Sroa`, before it scalarizes the aggregate's
leaves, so the new loads and stores are promoted by the same run. `Sroa`
is a structural-boundary pass: it runs at the start of the body's
pipeline and again after unroll and peel copy a loop. It is
off by `-fno-tree-sra` and at
-O0.

## See also
LLVM's SROA slicing an alloca's memcpys (`AllocaSlices`,
`visitMemTransferInst`).
