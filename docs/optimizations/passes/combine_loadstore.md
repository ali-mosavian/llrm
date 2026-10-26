> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/combine_loadstore.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/combine_loadstore.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/combine_loadstore.rs sha256:7d6d83cef9f1d4d692cd939d3b7b397bf22f3631b3a4ef241e43ec3f6eff4b3d -->

# combine_loadstore

InstCombine's stores and allocas: the store that cannot matter is erased,
the one that cannot execute stores poison; an alloca's count is an array
type and a zero-size alloca is one at the top of the entry block. Translated
from `visitAllocaInst`
([:509](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L509)),
`simplifyAllocaArraySize` (:182) and `visitStoreInst`
([InstCombineLoadStoreAlloca.cpp:1473](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineLoadStoreAlloca.cpp#L1473))
with `equivalentAddressValues` (:1452) and `canSimplifyNullStoreOrGEP`
(:1022).

## How it works
A volatile store is left alone. Otherwise the store is erased when it writes
an alloca nothing else reads (directly or through a one-use GEP), when a
store of the same type to the same address stands within the 6 instructions
before it with nothing that touches memory between (that earlier store is
erased instead), when it writes back the value just loaded from that
address, or when it stores `undef`. A store to null (or a GEP of null) in an
address space where null is not defined stores poison.

```text
store i16 1, ptr %p      ; erased
%x = add i16 %a, 1
store i16 %x, ptr %p
```

MIRROR: every check and its order, the scan of 6 and what ends it.

DEVIATE 1: `combineStoreToValueType`, `unpackStoreToAggregate` and
`replaceGEPIdxWithZero` are the next slices of this file, not yet
translated.

DEVIATE 2: a store to constant memory (`!isModSet(AA->getModRefInfoMask)`)
is not erased: the combine holds no alias analysis.

DEVIATE 3: a store to an `undef` pointer is not a marker that removes what
follows (`handleUnreachableFrom` is a CFG edit this pass does not make, see
the module doc of `combine`); the store stays.

DEVIATE 4: `simplifyNonNullOperand` on the pointer and the byte and
protected-field rewrites are not made (MIR has no byte type or
`llvm.protected.field.ptr`; the non-null rewrite is a later slice).

DEVIATE 5: what ends the scan of 6 is a load, a store, a call, an invoke or
a resume: MIR has no other instruction that reads memory, writes it or
throws.

DEVIATE 6: an alloca only copied from constant memory
(`isOnlyCopiedFromConstantMemory`, `PointerReplacer`) is not translated
here; `visitAllocSite` is, with DEVIATE 7 on `alloc_site_removable`. MIR's
scalar alloca has no count operand, so a count of 1 is dropping the operand.

## Used by
`combine` (its `visit`).

## Example
The first store is overwritten before anything reads it:
```mir
store i16 1, ptr %p
store i16 2, ptr %p
```
After, one store: `store i16 2, ptr %p`.
