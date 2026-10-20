> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchvalue.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchvalue.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchvalue.rs sha256:3e15c1caf2e78805f56fa5a53318b8e5abadbf3dca5b4e11fe1ef6278807fae0 -->

# switchvalue

The value-comparison folds of `simplifySwitch`: `simplifyEqualityComparisonWithOnlyPredecessor`, `simplifySwitchOnSelect` and `foldValueComparisonIntoPredecessors`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. It will hold, from the pin
(`SimplifyCFG.cpp` at d1106deb71cc): `simplifyEqualityComparisonWithOnlyPredecessor`
(L973), `foldValueComparisonIntoPredecessors` (L1475), `simplifySwitchOnSelect`
(L5192) with `simplifySwitchOnSelectRemap` (L5083), and their helpers
(`valuesOverlap`, `eliminateBlockCases`, `performValueComparisonIntoPredecessorFolding`).
Owed by llrm-w-4, 2026-10-23; the registry row is in `parity.rs`. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
