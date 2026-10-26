> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memcpyoptimizer.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memcpyoptimizer.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memcpyoptimizer.rs sha256:90aaea15569a4f6673a8c62f4b816ac2025acef67b90fc4e075de26a47a55167 -->

# memcpyoptimizer

Adjacent stores of one repeated byte become a `llvm.memset`: LLVM's
`MemCpyOptPass`, stage 1 (`runImpl`, `iterateOnFunction`, `processStore`,
`processMemSet`, `tryMergingIntoMemset`, `MemsetRanges`).

## How it works
Each block is walked in order. A simple store whose value is a repeated byte
(`isBytewiseValue`), or a `llvm.memset` of a constant length, starts a scan
of the instructions after it, up to the terminator. The scan keeps the
stores and memsets of the same byte at a constant offset from the start
pointer in `MemsetRanges`, the sorted byte ranges they cover, and stops at
the first instruction that touches memory otherwise (a store of another
byte, a load, a call). Every range of at least two instructions that is
worth it (`isProfitableToUseMemset`: four stores or sixteen bytes; fewer
only where the memset needs fewer stores of the widest native integer, and
always where a memset is among them) becomes one memset placed where the
scan stopped, and its stores go. The walk goes on from the last memset made.
The function is walked again until a walk changes nothing.

```text
  store i8 0, ptr %p                call memset(%p, 0, 4)
  store i8 0, ptr %p+1              (three stores gone)
  store i8 0, ptr %p+2        ==>
  store i8 0, ptr %p+3
  <first instruction that is not one of them>
```

MIRROR: `MemsetRanges::addRange`, `isProfitableToUseMemset`,
`tryMergingIntoMemset`, `processStore`, `processMemSet`,
`iterateOnFunction`, `runImpl`.

DEVIATE 1: `getPointerOffsetFrom` is `MemRef`'s root and displacement: two
pointers of one root with no variable index, no segment. Exit: none needed.
DEVIATE 2: a call that only accesses inaccessible memory ends the scan
(LLVM steps over it); `memory::of` has no such effect. Exit: model the
effect.
DEVIATE 3: only integer stores are bytewise (LLVM's `isBytewiseValue` also
reads floats, null and aggregates), and `processStore`'s promotion of an
aggregate store to a memset is absent. Exit: S2, with the float constants.
DEVIATE 4: unreachable blocks are walked. Exit: none needed.
DEVIATE 5: `processStoreOfLoad`, `processMemCpy` and the rest are stages 2
and 3.

## Example
Four byte stores:

```mir
store i8 0, ptr %p
%q1 = getelementptr i8, ptr %p, i16 1
store i8 0, ptr %q1
...
```

become `call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 4, i1 false)`.

## Runs
In the slot of `merge@818`, where `loopidioms::Merge` ran.

## Keeps
The blocks and edges: only instructions change.

## See also
LLVM
[`MemCpyOptimizer.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MemCpyOptimizer.cpp#L348).
