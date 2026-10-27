> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/equivalence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/equivalence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/equivalence.rs sha256:61ff9d80a6cecc1d9b14672416f732b5b968ebea3a13fc3e5434fce7fe41af73 -->

# equivalence

`EquivalenceClasses`: disjoint sets of values with a stable member order.

## How it works
A translation of LLVM `d1106deb71cc` `llvm/ADT/EquivalenceClasses.h`. Each
value is a node with a leader and a next pointer. A leader heads a list of
its class; `union_sets` appends the second class's list to the end of the
first's, so the first leader stays the leader and `members` visits the
first class's members, then the second's, each in its own order.

## Example
After `union_sets(1, 2)`, `insert(4)` and `union_sets(5, 1)`, `members(1)`
gives `5, 1, 2` and `members(4)` gives `4`.

## Used by
Named by `runtimecheck`.
