> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/postings.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/postings.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/postings.rs sha256:9b79e3b4f39a6bd423a7013792b1d4a6c675f603c10b0a791dbedba7aed746e4 -->

# postings

An index of where each value is defined, read and required in a body, kept
current as the body is rewritten.

## How it works
A spill or split rewrites only the instructions that name its values and
keeps every other `Arc<Insn>`. The spiller's scans look for a few values, so
they read an index of their occurrences instead of walking the body.
`Postings` holds, per value, the positions `(block, instruction)` of its
definitions (`defs`), reads (`uses`) and register requirements (`needs`), in
body order, and per block the positions of instructions with a frame cell
operand.

`following(body, read)` hands `read` the postings of `body`. One copy lives
per thread, with the instruction lists it was made of. Asked of a body, it
compares each block's list with the held one by `Arc` identity. If the
blocks are the same (same positions, same instruction pointers) it answers
at once. If the body has another shape, as a different function does, it
makes the postings whole (`Postings::of`). Otherwise it redoes only the
blocks that differ (`replaced`):

```text
 block before: [a b c d]           block after: [a b' x d]
 identity compare:  = ≠ . =        changed instructions: b -> b', c -> x
 values named by a b c d b' x are "touched"; for each, the block's entries
 are cut out of its list and the new ones spliced in, in position order.
```

A block of more than 48 instructions is patched instead of remade: the kept
instructions are found by pointer (`intervals::aligned_insns`) and their
entries moved to their new positions, the removed ones' dropped, the added
ones' put in, so the cost is the block's entries and not a hash insert each.

`LLRM_CHECK_POSTINGS` asserts after each update that the followed postings
equal ones worked out whole. `redone()` counts the blocks worked again, for
a test that a rewrite redoes only the blocks it changed.

## Example
None: the index is plumbing that changes no instruction, so there is no LIR
to show before and after. Read `Postings::of` for what it holds.
## Used by
[`crate::backend::allocate`](allocate.md), [`crate::backend::coalesce`](coalesce.md),
[`crate::backend::splitkit`](splitkit.md), [`crate::backend::spiller`](spiller.md) and
[`crate::backend::storedhomes`](storedhomes.md).
