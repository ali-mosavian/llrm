> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spillplacement.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spillplacement.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spillplacement.rs sha256:dc6657d1a98a4c80a806f8a5fc4caaa9b0940b78535839acfca129cb63876b2b -->

# spillplacement

Where a split value should be in a register and where on the stack.

LLVM's `EdgeBundles` and `SpillPlacement`. Each block has an entry and an
exit node; an edge joins a block's exit to its successor's entry, and the
joined nodes form a bundle, the unit that is either all register or all
stack. Blocks bias the bundles at their borders; a block the value only
passes through links its two bundles. A Hopfield network settles each
bundle's sign.
