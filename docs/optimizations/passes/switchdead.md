> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchdead.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchdead.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchdead.rs sha256:c5b68235a50d2ba71fb4c6547be2498ee5d3c2d47be220947d83867adaa2e01e -->

# switchdead

`eliminateDeadSwitchCases` and `simplifySwitchOfCmpIntrinsic`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
