> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inlinemetadata.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inlinemetadata.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inlinemetadata.rs sha256:0bbec84aaaae20eb3ed2c9b962a0920135f6759d942e200b10cf44c43826ba7c -->

# inlinemetadata

`PropagateCallSiteMetadata`
([L998](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L998)):
a call site's `!llvm.mem.parallel_loop_access`, `!llvm.access.group`,
`!alias.scope` and `!noalias` go to every memory-accessing instruction of the
inlined copy, joined with what the instruction has.

`ScopedAliasMetadataDeepCloner`
([L1108](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1108)):
a callee that carries scope lists is inlined with its own copy of them, so
two copies of one body are not in one scope.

`AddAlignmentAssumptions` (L1785) is `preserve-alignment-assumptions-during-inlining`,
hidden and off in the pin: nothing to translate.

## Deviations
- A node made here is not uniqued against the module's: two equal lists may
  have two ids. Nothing compares ids of non-distinct lists.

## Used by
Named by `inline`, `inlinealias`, `noaliasscopes`.
