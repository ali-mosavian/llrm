# The 59 M of ledger residual outside the buckets

Same profile as [llrm-only-inventory](llrm-only-inventory.md): parity 0aea1e46a, d_faces -O2 callgrind, 936.9 M Ir. Inclusive Ir in M, rows nest and do not add. Excluded: the alias stack (w14) and the passes with a pin counterpart (listed in the inventory). Pin counterparts marked `check` were not read against the pin's source.

## The residual is understated

The ledger's per-bucket sums count some Ir in two buckets (~39 M); the union of the buckets is 731.8 M. The true residual is about 153 M, not 114 M. The 59 M is a floor.

## Items

| item | Ir | pin counterpart | verdict |
|---|---|---|---|
| `globalsmodref::Sites::of` | 8.6 | GlobalsAA's module-level `AnalyzeGlobals` and call-graph walk, built once | w14's S0 (#4734); not touched here |
| `InterferenceCache Cursor::move_to_block` | 11.3 | `InterferenceCache::Entry::revalidate` and `update` | at the pin's shape; no cut known |
| `SlotIndex::cmp_stale` (thread-local arena) | 7.4 | `SlotIndex::operator<`, a compare of two loads through the entry pointer | 0.8 M taken in #4748; the rest needs pointer entries (`!Send`) |
| `Function::successors`, `predecessors` | 7.9, 7.1 | `succ_begin` and `pred_begin`, pointer walks | check how many callers build a Vec; the pin allocates none |
| `DenseRows::sets` (allocation rows) | 7.2 | `LiveRegMatrix` and `LiveIntervalUnion` | llrm-only; translate first |
| `Vec<Loc>::clone` | 6.2 | none (machine operands are in place) | candidate: find the clone sites |
| `memory::Unit::defining` | 5.5 | `getUnderlyingObject` | part of the alias stack (w14) |
| `BlockIter::next` | 5.5 | `Function` iteration, a list walk | at the pin's shape |
| `masm::addressed_pass` | 3.8 | X86 address-mode folding at isel | check |
| `footprint::of` | 2.4 | `getModRefInfo` of a call | alias stack (w14) |
| `machinesched::graph` | 2.4 | `ScheduleDAGInstrs::buildSchedGraph` | at the pin's shape |

## Allocation and copying

`__rust_alloc` 28.2 M exclusive (1.15 M calls), `__rust_dealloc` 17.7, `memcpy` 23.2 (621 K calls). By caller, from the callgrind call tree:

- Vec growth is the mass: `finish_grow` 29.8 M inclusive (182 K allocs 4.8 M, 63 K reallocs 15.1 M). Of the reallocs, mimalloc's `realloc` copies 9.5 M in 2,442 calls (~3.9 K Ir a call, the copy of a grown Vec).
- No caller of `grow_one` or `do_reserve_and_handle` is above 0.3 M. The top for `Vec<u32>` is `frameescape::exposed_allocas` 0.29 M (3.2 K growths); for `Vec<usize>` `machinesched::graph` 0.32 M. 43.6 K `u32` and 27.4 K `usize` growths are spread over ~100 sites.
- Other allocators: `String::clone` 1.0 M (39 K), hashbrown `with_capacity` 0.9 M (20 K), `Vec<Loc>::clone` 0.8 M (34 K), `Vec<String>` collect 0.6 M.

Verdict: a long tail, no cut of 0.5 M at one site. Presizing the top sites is not worth a row each; a general fix is a size hint from the previous run of the same pass, which the pin does not do (SmallVector inline storage covers the small cases). The first `memcpy` table of this note's draft matched `MemCpyOpt` by name and was discarded.

## Order

1. `cmp_stale`: ~5 M left, a structure change.
2. `Sites::of`: w14.
3. `Vec<Loc>::clone` and the `successors`/`predecessors` Vecs: find the sites first.

`cmp_stale` priced at 7.4 M is 131 K compares at ~48 Ir; its ceiling is ~6 M (pointer entries, `!Send`), and a destructor-free arena took 0.8 M (#4748).
