> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/equivalence.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/equivalence.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/equivalence.rs sha256:732a614548d2f8267b32b0b72a0b19e0242f5a425416a807f5dd19c261a33823 -->

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
