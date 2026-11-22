> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/slpinsert.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/slpinsert.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/slpinsert.rs sha256:b7a1e5c786fccc542ff5f6366c8f8aeed12ea666d21424f3e28cf0ff6d8d53ee -->

# slpinsert

`SLPVectorizer`'s buildvector seeds: a chain of `insertelement`s of
scalars is one vector when the tree of the scalars pays.

## How it works
`vectorizeInserts` (l.35325) walks the inserts `vectorizeChainsInBlock`
collected since the last instruction with no user, last first. For each,
`findBuildAggregate` (l.34348) follows the chain through the first operand
(single-use inserts at constant indices) and `vectorizeInsertElementInst`
(l.34781) gives the inserted scalars, by element, to `tryToVectorizeList`
(l.30044): the root is the inserts, priced as the shuffles and inserts the
vector replaces (`getEntryCost`, l.16556) and made as the vector shuffled
into the chain's first operand (`vectorizeTree`, l.23698).

DEVIATE SLP10 (owner arm64, exit 2026-12-01): the insert of a subvector
(`InsertSubvector`, a vector wider than a register with inserts in more than
one register) is not priced, so such a chain stays; the pass that tries
reductions rooted at an insert (`vectorizeHorReduction`) is not run here.

## Runs
In `slpvectorize`, after the phis and before the store chains.

## Used by
`slpvectorize`, `slptree`.

## Example
A chain of `insertelement`s of scalars whose tree of operations pays becomes
one vector operation.

```text
%x0 = add i32 %a0, %b0
%x1 = add i32 %a1, %b1
%i0 = insertelement <2 x i32> poison, i32 %x0, i64 0
%i1 = insertelement <2 x i32> %i0, i32 %x1, i64 1
=> %i1 = add <2 x i32> %va, %vb
```
