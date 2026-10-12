> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/spillplacement.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/spillplacement.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/spillplacement.rs sha256:ec7de36bcfd96c90e0c0e97621d913647327c5cddd49931a6ff6887618adfa4c -->

# spillplacement

Where a split value should be in a register and where on the stack.

LLVM's `EdgeBundles` and `SpillPlacement`. Each block has an entry and an
exit node; an edge joins a block's exit to its successor's entry, and the
joined nodes form a bundle, the unit that is either all register or all
stack. Blocks bias the bundles at their borders; a block the value only
passes through links its two bundles. A Hopfield network settles each
bundle's sign.
