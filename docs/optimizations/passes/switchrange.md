> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchrange.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchrange.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchrange.rs sha256:7152df541ec2a9dad8bafbb5d617ffb11a3fa40e958aa8a6b9faa5bd74e254cc -->

# switchrange

`turnSwitchRangeIntoICmp`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
