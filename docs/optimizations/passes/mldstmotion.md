> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mldstmotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mldstmotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mldstmotion.rs sha256:fbbab59bc9dffd0c46e2de7c817eacb31ba29c07ef66caea3a762b5eb2986576 -->

# mldstmotion

Stores to one address on both arms of a diamond become one store below it,
as LLVM's `MergedLoadStoreMotion` (`mldst-motion<no-split-footer-bb>`).

## How it works
A translation of LLVM
[`MergedLoadStoreMotion.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/MergedLoadStoreMotion.cpp)
at d1106deb71cc. A diamond head is a conditional branch whose two targets
each have the head as their only predecessor and the same single successor,
the tail. Walking the first arm from its end, a simple store with a store
on the second arm that must alias it, with nothing after either that may
throw, read or write the bytes, is paired with it. The first arm's store
moves to the top of the tail, taking a phi of the two values where they
differ; the second arm's goes. When the addresses are two identical
`getelementptr`s, each used only by its store and in its arm's block, the
address moves with the store. The walk then starts again from the end of the
first arm. At most 250 store-times-second-arm-size steps are tried
(`MagicCompileTimeControl`).

## Example
```text
 before                       after
 then: store i32 1, %p        then: ...
 else: store i32 2, %p        else: ...
 tail: ...                    tail: %v = phi [1, then], [2, else]
                                    store i32 %v, %p
```

The pipeline runs it without the footer split (`no-split-footer-bb`): a
tail with three predecessors or more is left alone.

DEVIATE 1: `CastInst::isBitOrNoopPointerCastable` stands for "the two stored
types are the same type, or both pointers" (MIR has no `bitcast` to insert
for the other pairs).
DEVIATE 2: debug locations and `DIAssignID` are not carried; MIR keeps none.

## Parity slot
`mldst-motion@783` in the simplify list between `sroa@776` and `gvn@787`
(PassBuilderPipelines.cpp#L783).

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Used by
Named by `pipeline`.
