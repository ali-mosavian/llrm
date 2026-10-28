> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/postings.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/postings.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/postings.rs sha256:7d5d2714ca4f15931ae32ec04379812386b21568fc64508bb4fa105b96f246f5 -->

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

Occurrences are kept by slot, not by position: each block numbers its
instructions with ascending slots, `STRIDE` apart, and an entry names
`(block, slot)`. A rewrite that keeps most of a block (`aligned_insns` finds
the kept instructions by pointer) leaves their slots and entries alone, cuts
the entries of the instructions it removed and puts those of the ones it
added at slots between their neighbours', as LLVM's `SlotIndexes` numbers an
instruction put between two (and renumbers a block only when the room is
used up). The cost of an edit is the entries of the instructions it changed,
not those of every instruction after it. A position, where a reader wants
one, is where the slot is in the block's list of slots.

A block the rewrite made over (little of it is the same, or the kept
instructions changed their order) is made again: the entries of the values
it names are cut out and the new ones spliced in.

`LLRM_CHECK_POSTINGS` asserts after each update that the followed postings
equal ones worked out whole. `redone()` counts the blocks worked again, for
a test that a rewrite redoes only the blocks it changed.

## Example
None: the index is plumbing that changes no instruction, so there is no LIR
to show before and after. Read `Postings::of` for what it holds.
## Used by
[`crate::backend::allocate`](allocate.md), `crate::coalescer::pass`,
`crate::backend::splitkit`, `crate::backend::spiller` and
[`crate::backend::storedhomes`](storedhomes.md).
