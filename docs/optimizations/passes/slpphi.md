> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpphi.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpphi.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpphi.rs sha256:349bcb41609abfa2188bd83139a28af4b298ca2c18f490f6bead5d36662382fa -->

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

## Example
Sixteen loop-carried doubles in a loop header become `<2 x double>` phis
when the trees of their values pay (the nbody test).

```text
%a = phi double [ 0.0, %entry ], [ %a1, %loop ]
%b = phi double [ 0.0, %entry ], [ %b1, %loop ]
        =>
%v = phi <2 x double> [ zeroinitializer, %entry ], [ %v1, %loop ]
```
