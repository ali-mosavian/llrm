> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/splitstore.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/splitstore.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/splitstore.rs sha256:b02324b72b10089a00450861adaaf8be4d7d54820289255563a14eef7d1d3b8e -->

# splitstore

CodeGenPrepare's `splitMergedValStore`: a store of two values merged into
one wider integer is two stores
([CodeGenPrepare.cpp#L8608](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8608)).

## How it works
`std::make_pair(int, float)` passed by reference is, after SROA, a store of
`zext(bitcast f) | (zext(i) << 32)`. The target decides whether two stores
beat the merge (`isMultiStoresCheaperThanBitsMerge`; x86: a float and an
integer). The store is simple (not volatile), the value an integer of whole
bytes with halves of whole bytes, the `or` commutative, and the `zext`s and
the `shl` (by half the width) each read once. The low half goes to the
address and the high half half a width above (the other way round on a
big-endian target), at the alignment the offset allows.

```text
 %lz = zext i32 %b to i64                 store i32 %b, ptr %p, align 8
 %hz = zext i32 %i to i64                 %g = getelementptr i32, ptr %p, i32 1
 %hs = shl i64 %hz, 32            ==>     store i32 %i, ptr %g, align 4
 %o  = or i64 %lz, %hs
 store i64 %o, ptr %p, align 8
```

MIRROR: the whole function, in the pin's order of tests.

DEVIATE SPLIT1: the `-force-split-store` option (off by default) is not
read. Exit: none.

DEVIATE SPLIT2: the `or`, `shl` and `zext`s the store read go with it; the
pin leaves them for the dead code pass that follows.

## Example
`test_a_float_and_an_integer_merged_into_a_store_are_two_stores` in
`splitstore_tests.rs` is the sketch above.

## Runs
In `cgpaddr`'s run, before the address-mode sinking (the stores it makes
are sunk in turn).
