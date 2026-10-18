# Block storage: the instruction list as LLVM's

Status: plan. A block holds its instructions as a `Vec<InstId>`
(`Block::instructions`, `llrm-mir/src/module.rs`). Inserting or erasing at a
known instruction costs O(block): a position scan and a shift. LLVM's
`BasicBlock` holds an intrusive list ([`simple_ilist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/ADT/simple_ilist.h#L78)):
insert, erase and splice at an iterator are O(1). A pass that does one
mid-block edit per instruction is linear in LLVM and quadratic here, so the
fix is the storage, not a batch in each pass.

## Evidence
SROA translated from `SROA.cpp` (#2317) on single-block ladders, work per
doubling: mem400 to mem800 x3.76, loop x3.70, diamond x2.5; LLVM's `opt` x2.05.
Profile on mem800: `Function::detach` 32%, `attach` 18%, `erase` 12%. Batching
the erases (`erase_all`, #2064) fixes the erase share only; the inserts stay
O(block) (#2316).

## What exists
- Writers are already positional: `insert`, `move_to`, `move_run`, `erase`,
  `erase_all` take a `Position` (`Before(inst)`, `End(block)`), 226 uses of
  `Position::` and 147 of `.erase(`. Their bodies are the O(block) part.
- Readers take the slice: 337 uses of `.instructions()` in 114 files
  (82 in `crates/opt`, 21 `crates/ir`, 8 `crates/backend`, 2 target, 1
  frontend), by shape: about 140 `for x in b.instructions()`, 32 indexing
  `[i]`, 22 `.iter()`, 21 `.len()`, 8 `.last()` (the terminator), the rest
  snapshots (`.to_vec()`) before editing. Largest users: `jumpthread` 14,
  `lsr` 11, `isel` 11, `mem2reg` 10, `floatloop` 10, `guarantee` 9, `ehprepare` 9.
- `Function::positions()` and indexing give "which comes first" in O(1) today
  because the Vec is dense; a list does not.

## The API (LLVM's names)
All on `Function`/`Block`, mirroring `BasicBlock`/`Instruction`:

| LLVM | here | cost |
|---|---|---|
| `BasicBlock::begin/end`, `iterator` | `Block::iter()` (double-ended), `Function::next_in_block/prev_in_block(inst)` | O(1) step |
| `getTerminator`, `front`, `back` | `Block::terminator()`, `first()`, `last()` | O(1) |
| `size`, `empty` | `Block::len()` (kept count), `is_empty()` | O(1) |
| `insertBefore/After(iterator)` | `insert(inst, Position::Before/After(x))` | O(1) |
| `eraseFromParent` | `erase(inst)` | O(1) |
| `BasicBlock::splice(Dest, Src, First, Last)` | `splice(run, Position)` | O(run) |
| `moveBefore/After` | `move_to` | O(1) |
| [`Instruction::comesBefore`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/Instruction.cpp#L335) with [`renumberInstructions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/IR/BasicBlock.cpp#L636) | `comes_before(a, b)` on a lazily renumbered order cache, invalidated by an insert, rebuilt on the next ask | amortised O(1) |

Indexing (`[i]`, `position`, `windows`) goes away: a reader that needs "the
instruction before this one" asks `prev_in_block`; "the n-th" has no LLVM
counterpart and is replaced by a walk or by `comes_before`.

## Stages
**A. The API over today's Vec** (draft PR, then readers pass by pass). Add the
table above; `splice` and a batched insert make a run of edits one rewrite of
the block (linear). Readers move from slices to `iter()`/cursors in PRs of
one crate or pass each; every PR is output-identical (identical.py full
corpus, every `LLRM_CHECK_*` on) and removes slice uses until `instructions()`
is private. Cost: about 140 reader sites in 114 files, mechanical; the 32
indexing sites and the 8 `last()` need a reading each. Slope test: SROA and
`dead` on the ladder, `edit::instructions_scanned` doubling ratio <= 2.3.

**B. The swap.** Behind the API only: prev/next per `InstId` in an arena
(`Vec<(InstId, InstId)>` indexed by id, the block holds head, tail, length),
`parent` kept as today. Insert, erase, move are O(1); `comes_before` uses
order keys renumbered lazily per block as LLVM does. Cost: the writers' bodies
(`edit.rs`), `Block`, `Function::positions`, the clone and `delete_body`
paths, the print/parse/verify readers, the change log (`Change::Inserted {
next }` already names a following instruction, so the log is list-shaped).
No pass changes if A is complete.

## Price the swap first (POC)
Before B: SROA alone, same sources, on an index-linked list in an arena
(prev/next per instruction, head/tail per block), against the Vec version on
mem400/800, loop400/800, diamond400/800 (replay inputs, `sroa-poc sroa-unit`).
B is approved only if the ratio per doubling comes near LLVM's 2.05.

Bound, before building it (perf CPU samples of `sroa-poc sroa-unit`, 25 kHz;
list = `detach`, `attach`, `erase`, memmove; rest = everything else, parse
included, so it is a floor on linearity). Per doubling, 400 to 800:

| ladder | total | list ops | rest |
|---|---|---|---|
| mem | x2.75 | x3.56 | x2.13 |
| loop | x2.74 | x4.1 | x1.75 |
| diamond | x3.1 | x4.8 (4 to 19 samples) | x2.98 |

With the list ops taken to O(1) the rest bounds the ratio: mem about 2.1, loop
about 1.8, so the swap can bring SROA near LLVM's x2.05 on those. Diamond's
rest is x3.0 by itself: a second super-linear term that is not the list
(unlocated; its own issue).

POC result (branch `alim/poc/block-list`, `sroa-poc sroa-unit`, same method;
output byte-identical to the Vec version on the five ladders checked): the
arena list as truth (prev/next per `InstId`, head/tail/len per block) with the
`Vec` kept as a lazy view rebuilt on read, SROA's own readers unchanged.
CPU samples, 400 to 800: mem 123 to 244 (x1.98, was x2.75), loop 130 to 284
(x2.18, was x2.74), diamond 51 to 79 (x1.55, was x3.1); mem800 is 3.4 times
faster than the Vec (244 against 832 samples). That is LLVM's x2.05. Two
lessons: any read of `instructions()` after an edit rebuilds the view (the
first run, with the erased-block check going through `block()`, was 77% in
`attach`), so stage A's reader migration is part of the cost, not optional;
and `Block::instructions()` cannot stay a slice once the list is the truth.

## DEVIATE
None planned: the list is LLVM's. A deviation is recorded here only if the
POC leads to a storage that is not an ilist.
