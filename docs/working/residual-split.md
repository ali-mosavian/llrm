# The 59 M of ledger residual outside the buckets

Same profile as [llrm-only-inventory](llrm-only-inventory.md): parity 0aea1e46a, d_faces -O2 callgrind, 936.9 M Ir. Inclusive Ir in M, rows nest and do not add. Excluded: the alias stack (w14) and the passes with a pin counterpart (listed in the inventory). Pin counterparts marked `check` were not read against the pin's source.

## The residual is understated

The ledger's per-bucket sums count some Ir in two buckets (~39 M); the union of the buckets is 731.8 M. The true residual is about 153 M, not 114 M. The 59 M is a floor.

## Items

| item | Ir | pin counterpart | verdict |
|---|---|---|---|
| `globalsmodref::Sites::of` | 8.6 | GlobalsAA's module-level `AnalyzeGlobals` and call-graph walk, built once | check whether ours rebuilds per query; if so cache it |
| `InterferenceCache Cursor::move_to_block` | 11.3 | `InterferenceCache::Entry::revalidate` and `update` | at the pin's shape; no cut known |
| `SlotIndex::cmp_stale` (thread-local arena) | 7.4 | `SlotIndex::operator<`, a compare of two inline integers | translate first: hold the index inline, drop the arena lookup |
| `Function::successors`, `predecessors` | 7.9, 7.1 | `succ_begin` and `pred_begin`, pointer walks | check how many callers build a Vec; the pin allocates none |
| `DenseRows::sets` (allocation rows) | 7.2 | `LiveRegMatrix` and `LiveIntervalUnion` | llrm-only; translate first |
| `Vec<Loc>::clone` | 6.2 | none (machine operands are in place) | candidate: find the clone sites |
| `memory::Unit::defining` | 5.5 | `getUnderlyingObject` | part of the alias stack (w14) |
| `BlockIter::next` | 5.5 | `Function` iteration, a list walk | at the pin's shape |
| `masm::addressed_pass` | 3.8 | X86 address-mode folding at isel | check |
| `footprint::of` | 2.4 | `getModRefInfo` of a call | alias stack (w14) |
| `machinesched::graph` | 2.4 | `ScheduleDAGInstrs::buildSchedGraph` | at the pin's shape |

## Allocation and copying

`__rust_alloc` 28.2 M exclusive, `__rust_dealloc` 17.7, `finish_grow` 29.8 inclusive, `memcpy` 23.2. The pin's cost for the same work is not measured here, so no row above takes a share of these. Next step: attribute them by caller with `--tree=caller` and take the top three callers.

## Order

1. `cmp_stale`: 7.4 M, one structure change, pin shape known.
2. `Sites::of`: 8.6 M if it is rebuilt per query.
3. `Vec<Loc>::clone` and the `successors`/`predecessors` Vecs: find the sites first.
