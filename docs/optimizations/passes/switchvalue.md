> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchvalue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchvalue.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchvalue.rs sha256:dc4180b8a83a5d5c707a684b33a91dd9b30fd162bee013918a47d0e3510609ac -->

# switchvalue

A `switch` on a value its predecessors compare with constants too, folded
with them: LLVM's
[`simplifyEqualityComparisonWithOnlyPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L973)
and
[`foldValueComparisonIntoPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1475)
with
[`performValueComparisonIntoPredecessorFolding`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1251),
the first steps of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
and their helpers
[`isValueEqualityComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L866),
[`getValueEqualityComparisonCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L899),
`eliminateBlockCases` ([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L929))
and `valuesOverlap` ([L935](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L935)).

## How it works
A block with one predecessor that compares the same value (a `switch`, or
a branch on `x == C`) knows which constants reach it. Reached by the
predecessor's default, the cases the predecessor lists cannot happen: they
are removed. Reached by one case, the constant is known: the `switch`
becomes a `br` to the block that constant picks. Dead edges lose their phi
inputs.

A block of only the `switch` is folded into every predecessor that
compares the same value: the predecessor becomes one `switch` that sends
each constant where the two together did (a pointer is cast to an integer
first). A successor both reach whose phis disagree gets a block of its
own on the `switch`'s edge first (`SplitBlockPredecessors`); an edge back
to the folded block goes to a new block that loops.

## Example
```text
  b0: switch i16 %x, label %b1 [ 1, %ca ]          b0: switch i16 %x, label %cd [ 1, %ca
  b1: switch i16 %x, label %cd [ 3, %cc ]    ==>                                  3, %cc ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights, no `DomTreeUpdater`. |
| D2 | A pointer compared with `inttoptr` of a constant is not a value comparison (`getConstantInt`'s `inttoptr` arm); `null` is. |
| D3 | The calls from `simplifyCondBranch` ([L8963](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8963)) are not made: only a `switch` is rewritten, a conditional branch is only read as a predecessor. Exit: call them from `simplifycfg::block_simplified` (llrm-w-4, 2026-10-27). |

## Used by
`switchopt::simplified`, the first step.
