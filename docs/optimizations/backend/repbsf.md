> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/repbsf.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/repbsf.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/repbsf.rs sha256:71b428c4fd5982fc3265d8f8c2cf65e93c819a82e73b58d4c4f0ae6d63097515 -->

# repbsf

`bsf` gets the `rep` prefix (the TZCNT encoding, which processors without
`tzcnt` run as the same `bsf`) unless the function is optimised for size or
the instruction's flags are read.

LLVM's `X86MCInstLower` `BSF*` case sets `IP_HAS_REPEAT` when
`!hasOptSize() && EFLAGS is dead`
([L516](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86MCInstLower.cpp#L516)).
`Profile::size` stands for `hasOptSize`.
