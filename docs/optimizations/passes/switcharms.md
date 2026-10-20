> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switcharms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switcharms.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switcharms.rs sha256:fa4df4ac747327dc066db37380d85674277cf66ccdd83127f66a74207fc384ac -->

# switcharms

`simplifyDuplicateSwitchArms`, `simplifySwitchWhenUMin` and `simplifySwitchDefaultBranch`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
