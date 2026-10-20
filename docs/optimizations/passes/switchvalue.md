> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchvalue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchvalue.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchvalue.rs sha256:0f5d55d8964f87e2270aca2fcfa9d228554a758e3b3b656cd878ef04528c74ef -->

# switchvalue

The value-comparison folds of `simplifySwitch`: `simplifyEqualityComparisonWithOnlyPredecessor`, `simplifySwitchOnSelect` and `foldValueComparisonIntoPredecessors`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
