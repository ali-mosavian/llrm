> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpvectorize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpvectorize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpvectorize.rs sha256:d6254cfe6e3f45a75002989108a0e9312290152dfd5db365c2358f4da9710dec -->

# slpvectorize

`SLPVectorizer`'s store chains: consecutive stores of one scalar type
become vector stores when the cost model says the vector is cheaper.

## How it works
`vectorizeStores` (l.29748): the stores of a block are grouped by the
object they address and the type they store, each group sorted by offset,
and cut into runs of adjacent cells. Over a run the vectorization factor
falls from the widest power of two the vector register and the run hold to
the target's `getStoreMinimumVF`; at each factor the run is tried in
windows of that many stores, a window that vectorizes is not tried again.
A window is a tree of the stores and the vector of their values; its cost
is the vector store less the scalar ones, the values' own (a vector of
constants costs nothing), and it vectorizes when that is below the
`slp-threshold` of 0 (`getTreeCost` l.19795, rejection l.19079). The
vector store stands at the last store of the window, with the address and
alignment of the first.

A window whose values are not all constants is a tree of the loads,
arithmetic, casts, compares, selects and `llvm.fmuladd` that make them
(`slptree`); the first such window that pays is made and the function is
looked at again, so no plan names a scalar another has gone from.

DEVIATE SLP1 (owner arm64, exit 2026-11-24): the narrower tree
`slptree` states.

## Runs
At the pin's `slp-vectorizer` slot (`PassBuilderPipelines.cpp` l.1455).

## Used by
`pipeline` (Late).

## Example
Eighty adjacent `store i16 1824` become ten `store <8 x i16>` of the
constant (the const_stores test).

```text
store i16 1824, ptr %p0 ... store i16 1824, ptr %p79
        =>
store <8 x i16> <i16 1824 x 8>, ptr %p0 ... ptr %p72 (ten stores)
```
