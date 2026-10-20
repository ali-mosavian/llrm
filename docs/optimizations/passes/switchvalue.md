> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchvalue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchvalue.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchvalue.rs sha256:26e1f681e4a788635b7a21ff4d60a1b0f36830d8b99de38e3a7c0664f0b21891 -->

# switchvalue

A `switch` whose only predecessor compares the same value, simplified by
what that predecessor decided: LLVM's
[`simplifyEqualityComparisonWithOnlyPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L973),
the first step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)),
with
[`isValueEqualityComparison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L866),
[`getValueEqualityComparisonCases`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L899),
`eliminateBlockCases`
([L929](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L929))
and `valuesOverlap`
([L935](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L935)).

## How it works
A block with one predecessor that is a `switch` (or a compare of the value
with a constant) on the same value knows which constants reach it. Reached
by the predecessor's default, the cases the predecessor lists cannot
happen: they are removed from the `switch`. Reached by one case, the
constant is known: the `switch` becomes a `br` to the block that constant
picks. Dead edges lose their phi inputs.

## Example
```text
  b0: switch i16 %x, label %b1 [ 1, %ca  4, %ca ]       b0: switch i16 %x, label %b1 [ 1, %ca  4, %ca ]
  b1: switch i16 %x, label %cd [ 1, %cb  3, %cc ]   ==>  b1: switch i16 %x, label %cd [ 3, %cc ]
```

| # | DEVIATE |
|---|---|
| D1 | No branch weights, no `DomTreeUpdater`. |
| D2 | A pointer compared with `inttoptr` of a constant is not a value comparison (`getConstantInt`'s `inttoptr` arm); `null` is. |
| D3 | The calls from `simplifyCondBranch` ([L8963](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8963)) are not made: only a `switch` is rewritten, a conditional branch is only read as the predecessor. Exit: call them from `simplifycfg::block_simplified` (llrm-w-4, 2026-10-27). |

## Used by
`switchopt::simplified`, the first step.
