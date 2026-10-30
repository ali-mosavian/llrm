> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/knownpred.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/knownpred.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/knownpred.rs sha256:6e4e7229692f9794168cd4af860c6ffa50b1856beb38dd82a7d5d8e541a30410 -->

# knownpred

A branch on a value known in some predecessors, threaded: LLVM's
[`SimplifyCFGOpt::foldCondBranchOnValueKnownInPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3814)
and its `Impl`
([L3624](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L3624)).

A predecessor that gives the condition a constant (a phi input, or a
predecessor that branches on the condition itself) goes straight to the
arm that constant chooses: the predecessors are split off to a block that
holds a copy of what the branching block computes, folded on the constant,
and that block jumps to the arm.

## Example
```text
  b0: br i1 %c1, b1, b2                b0: br i1 %c1, b1, b3
  b1: %c = ...; br b2                  b1: %c = ...; br b2
  b2: %p = phi i1 [ false, b0 ],  ==>  b2: %p = phi i1 [ %c, b1 ]; br i1 %p, b3, b4
                  [ %c, b1 ]
      br i1 %p, b3, b4
```

Only blocks of at most 11 instructions are threaded through
(`simplifycfg-max-small-block-size`), and a value of the block live in more
than 24 blocks stops it (`max-jump-threading-live-blocks`).

| # | DEVIATE |
|---|---|
| D1 | Ephemeral values (feeding only `llvm.assume`) are counted as instructions. |
| D2 | `TTI.hasBranchDivergence` is not asked: no llrm target diverges. |
| D3 | The new block is not moved before the destination; it is merged into its predecessor when it can be, and otherwise stays last in the layout. |
| D4 | No indirect branches exist, so none is skipped. |
| D5 | `mayHaveSideEffects` of a folded copy reads no module globals: a call is kept. |

## Used by
Named by `simplifycfg`.
