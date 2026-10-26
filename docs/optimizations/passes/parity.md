> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/parity.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/parity.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/parity.rs sha256:dda13c22db90307da78ef555938bae3ca828ade0c6bc4d1f00bc8eea0d1f9233 -->

# parity

The parity pipeline's slot registry: for each slot of LLVM's function
pipeline that `Options::llvm_order` builds, whether llrm's pass there is
translated from LLVM's, an older stand-in, or missing.

## How it works
`REGISTRY` has one `Entry` per slot, named as in `llvm_order` (the pass
and the `PassBuilderPipelines.cpp` line it stands at). `check_phase`
compares one built list with it: a `SlotKind::Translated` slot of that
phase that the list does not run, or a slot the list runs that has no entry,
is an error. `Fixed::linear` asserts it in debug builds, so a translation
cannot be marked joined and left out of the pipeline. The slots built with a
translation are told to [`llrm_mir::counts::joined`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/counts.rs), so `LLRM_DEBUG=counts`
prints, per analysis, how many times a joined slot made the manager compute
it and how many times an old one did (`callers` lines): an analysis with no
old caller can be deleted.

A translation joins when its oracle passes (output identical to `opt` at the
pin), not when it is on by default; see
the parity pipeline plan page (PR 2288).

## Example
`parity::check_phase(Phase::Simplify, &built)` is `Ok` when every Translated
Simplify slot is in `built` and every name in `built` has an entry.

## Used by
`pipeline` (`Fixed::linear`), the registry tests.
