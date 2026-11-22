> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mergeicmps.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mergeicmps.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mergeicmps.rs sha256:a401fc6de984e2f6edd546a0c4f5adbcbb1d7b8897c20e7ed33032139a9b8988 -->

# mergeicmps

A chain of `a.x == b.x && a.y == b.y` compares of adjacent fields is one
`memcmp` call, as LLVM's `MergeICmps`.

## How it works
A translation of LLVM
[`MergeICmps.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/MergeICmps.cpp)
at d1106deb71cc. Each phi that joins a chain of blocks, every one a
compare of two loads at a constant offset from a base that branches out to
the phi on a difference, is looked at. The compares are sorted by
(base, offset); those whose bytes touch become one `memcmp(a, b, n) == 0`
block, and the chain is rebuilt from the groups in their first-seen order.
The groups' bytes must be dereferenceable, as the loads may now run in
another order. Only where the target expands `memcmp` later
(`Tti::enable_mem_cmp_expansion`) and the compile states `memcmp`.

## Example
```text
b0: %x = load a.0; %y = load b.0; br (%x == %y), b1, done
b1: %u = load a.2; %v = load b.2; br done   ; done: phi [false, b0], [%u == %v, b1]
```
becomes one block calling `memcmp(a, b, 4)` and comparing it to zero.

DEVIATE 1 (exit 2026-12-31,
[runImpl](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L948)):
the pin updates the dominator tree with a `DomTreeUpdater`; here a change
invalidates it.

DEVIATE 2 (exit 2026-12-31,
[canSinkBCECmpInst](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L249)):
`AA.getModRefInfo` is the store against load `alias` answer; any other
writer (a call) is assumed to modify.

DEVIATE 3 (exit 2026-12-31,
[isDereferenceable](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L759)):
`valuetracking::dereferenceable` has no context instruction, so what the
pin proves from an assume or a free before it is not seen.

DEVIATE 4 (exit 2026-12-31,
[mergeComparisons](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/MergeICmps.cpp#L650)):
the merged location is the first compare's, and a merge of several blocks
has no `!prof` (`computeMergedBranchWeights` is untranslated).

MIR has no `blockaddress`, so the address-taken checks have nothing to
refuse.

## Parity slot
`mergeicmps@1671` (PassBuilderPipelines.cpp#L1671), between `divrempairs`
and `tailrec`. Not wired: `ExpandMemCmp` (L1676) is untranslated, and a
`memcmp` call left in place is worse than the compares.

## Keeps
Nothing: blocks and edges change.

## Used by
Named by `pipeline`.
