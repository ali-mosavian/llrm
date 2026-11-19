> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpphi.rs sha256:dd09fddfc0dfe943a126ff599bae89a20670b2c7dc8e55806db098f186187e70 -->

# slpphi

`SLPVectorizer`'s phi seeds: the phis of a block that take alike values are
one vector phi when the tree of the values they take pays.

## How it works
`vectorizeChainsInBlock` (l.35365) gathers the block's phis of a type the
vectors hold. Each is mapped to the values that reach it through chains of
phis (`PHIToOpcodes`) and they are sorted by type, size, and those values:
instructions first, by block and opcode, then constants, then the rest
(`PHICompare`). `tryToVectorizeSequence` (l.34805) cuts the sorted list
into runs of compatible phis (`AreCompatiblePHIs`, same type and alike
values), and gives each run, and the leftovers of runs too short to fill a
register, to `tryToVectorizeList` (l.30044): windows of the widest factor
down to the narrowest, a window whose tree costs less than the
`slp-threshold` of 0 being made. nbody's sixteen loop-carried doubles
become four `<2 x double>` phis.

DEVIATE SLP6 (owner arm64, exit 2026-12-01): `slptree` states the trees of
phis it takes; the phis' windows are not reordered (`reorderTopToBottom`).

## Runs
In `slpvectorize`, before the store chains (`vectorizeChainsInBlock`
looks at the phis first).

## Used by
`slpvectorize`.
