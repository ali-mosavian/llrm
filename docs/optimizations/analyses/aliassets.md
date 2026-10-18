> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/aliassets.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/aliassets.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/aliassets.rs sha256:663a049bf91e2ff67a83c7c8a9af14c1a05933a1702423c75cd786840d4a1a1e -->

# aliassets

`AliasSetTracker` (LLVM `AliasSetTracker.cpp`, `d1106deb71cc`): the
locations an analysis asks of, grouped into sets that may alias, with
whether each set's locations all must alias. Over the locations of
`mssa::alias::AliasQuery`.

DEVIATE: the reference counts and the intrusive list are not kept: a set
that forwards is skipped by every iteration, and one that does not is
referenced by the pointer map, so no set is ever removed. Unknown
instructions (`addUnknown`) are not tracked: LICM adds loads and stores
only.
