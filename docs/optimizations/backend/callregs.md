> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/callregs.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/callregs.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/callregs.rs sha256:cdb8ffffbc28927b83f9a99e7d1a200fdcfabddc32bc338fafba8d36edcc0c16 -->

# callregs

Which registers a runtime call destroys or keeps, read from its contract.

## How it works
A runtime routine's contract names the registers it disturbs. Each
allocatable register is matched by its 16- and 32-bit spellings:
`call_clobbers` is the registers the contract disturbs plus the selectors
it cannot be trusted with, and `call_keeps` is the general registers it
does not disturb.

Selectors are the exception. A contract describes the 8086 and names no
FS or GS, so where it disturbs every register, or was written for the
386 (`contract.i386`), the selectors it does not name are clobbered
too (`unnamed_selectors_clobbered`). For an i386 contract
`call_clobbered_high` is the registers the call keeps only the 16-bit half
of: every general register it does not clobber whole.
`call_clobbered_high_keeping` removes those the callee's convention saves
in full.

## Used by
`isel.rs`, `isel/wide.rs`, `isel/unwind.rs` (the clobber sets of a call
they select) and `abi/qb.rs` (how many registers a call keeps).
