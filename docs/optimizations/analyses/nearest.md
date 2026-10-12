> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/nearest.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/nearest.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/nearest.rs sha256:a8f57c531cc48015086d107a15dae48a91cc57d5dbeec3f0d6f7a8bc322f074e -->

# nearest

The items placed in blocks that dominate a given block, nearest first.

## How it works
Loads look for a provider among the stores and loads that dominate them.
Scanning every item of the function per load is quadratic. `Nearest::of`
builds, per key, the blocks that hold an item of that key, sorted by their
preorder number in the dominator tree, and links each to the nearest block
above it in the tree that also holds one. `Nearest::dominating` binary
searches for the last block at or before the query block, walks the links up
until a block's subtree span contains the query, and yields the items from
there up. The work is the length of that chain, not of the function.

```text
 dominator tree           key K holds items in A, C, E
      entry                 sorted by preorder: A  C  E
       |                    up links:           -  A  C
       A (K)                query at F (under E): last block <= F is E
      / \                   chain E -> C -> A
     B   C (K)
         |
         E (K)
         |
         F  <- query
```

Items of several keys are merged by a cursor per key: the walk takes the
largest preorder number first, and within a block the last item first. An
item reached through two keys is returned once. A block the dominator tree
does not reach is left out of the chains and, as the query, yields nothing:
everything dominates it, so the caller decides.

## Provides
The `Nearest` index and its `Walk` iterator. Consumers: [`crate::avail`](avail.md) and
`loadjoins` in llrm-transforms.

## Needs
`llrm_mir::dominators::DominatorTree` (`span`, the preorder interval of a
block).
