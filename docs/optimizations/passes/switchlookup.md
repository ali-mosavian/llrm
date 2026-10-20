> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/switchlookup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/switchlookup.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/switchlookup.rs sha256:94fd322157d815cfbabbcdc4c267abd28f6407540c6f6cb49578bbe7e61a8415 -->

# switchlookup

`simplifySwitchLookup`: `switchToLookupTable` and `convertSwitchToArithmetic`, a step of `simplifySwitch`
([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).

Not translated yet: `folded` finds nothing. [`crate::switchopt`](switchopt.md) calls it
where LLVM does.
