> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchphi.rs sha256:176d24f8ec1253ebf94b97e5d35700f52fd6afcdf0bae3d2ee2ab3138e209770 -->

# switchphi

`forwardSwitchConditionToPHI`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
