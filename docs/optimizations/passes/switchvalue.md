> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchvalue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchvalue.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchvalue.rs sha256:1e4a529a73cd325abdcd5cb6825b4eb50b8170f457fa5005dda43b37dc0fd408 -->

# switchvalue

The value-comparison steps of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)):
a `switch` on a value its predecessors compare with constants too, folded
with them, and a `switch` on a `select`. LLVM's
[`simplifyEqualityComparisonWithOnlyPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L973),
[`foldValueComparisonIntoPredecessors`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1475)
with
[`performValueComparisonIntoPredecessorFolding`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L1251),
and
[`simplifySwitchOnSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5192)
with
[`simplifySwitchOnSelectRemap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L5083)
and
[`simplifyTerminatorOnSelect`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4998),
with the helpers
[`isValueEqualityComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L866),
[`getValueEqualityComparisonCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L899),
`eliminateBlockCases`
([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L929))
and `valuesOverlap`
([L935](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L935)).

## How it works
A block with one predecessor that compares the same value (a `switch`, or
a branch on `x == C`) knows which constants reach it. Reached by the
predecessor's default, the cases the predecessor lists cannot happen: they
are removed. Reached by one case, the constant is known: the `switch`
becomes a `br` to the block that constant picks. Dead edges lose their phi
inputs.

A `switch` on `select (x == C), K, x` switches on `x`, `C` going where `K`
goes. A `switch` on a `select` of two constants is a branch on the select's
condition to the blocks the constants go to.

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

## Used by
`switchopt::simplified`, the first step, and `simplifycfg::block_simplified`
for a conditional branch (`simplifyCondBranch`'s value comparisons).
