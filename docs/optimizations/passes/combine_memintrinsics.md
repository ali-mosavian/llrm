> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_memintrinsics.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_memintrinsics.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_memintrinsics.rs sha256:74270aae892e19282a05a441d51cbb02368d31272a20a21c93f12d5699862834 -->

# combine_memintrinsics

InstCombine's memory intrinsics: a `memcpy` or `memmove` of 1, 2, 4 or 8
bytes is one integer load and one store. Translated from
`InstCombinerImpl::SimplifyAnyMemTransfer`
([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp)).

## How it works
A transfer of a constant length of 1, 2, 4 or 8 bytes becomes `load iN`
from the source and `store iN` to the destination, each at the alignment
known of its pointer, volatile as the transfer was; a single load and store
is right for an overlap, so a `memmove` goes the same way. A transfer from
an undef or poison source that is not volatile is erased. The C front end's
aggregate assignment is a `memcpy` (#3621); this is what lets SROA and
promotion see a copy of a small struct as a value.

## Example
```text
call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 4, i1 false)
  ==>  %v = load i32, ptr %s            ; store i32 %v, ptr %d
```

MIRROR: the size test (at most 8 and a power of two), the integer type, the
volatile bit carried over.

DEVIATE 1: MIR gives a transfer's pointers no `align` attribute, so the
alignments are `pointer_alignment` of each side
(`Value::getPointerAlignment`), not `getKnownAlignment` with the assumption
cache and the dominator tree, and the pin's step that first raises the
transfer's own alignments is not made. DEVIATE 2: no
`isModSet(AA->getModRefInfoMask(Dest))` constant-memory test, and no atomic
transfers (MIR has none); no TBAA or loop metadata to carry (MIR transfers
have none).

## Used by
`combine` (its `visit`, the `Call` opcode).
