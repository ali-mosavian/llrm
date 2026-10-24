# Pre-sizing growing vectors (compile time)

Owner: funcattrs-port (2026-10-24). Base feat/parity 9f3810d98, ledger command (qbq-llrm d_faces, -m32 -O2), dist build: 922.56 M Ir. `RawVecInner::finish_grow` is 31.34 M inclusive: 17.51 M of it in 74,928 `realloc` calls (a vector doubling), the rest first allocations (`Vec::new` then `push`), which a capacity does not save.

## Sites, by growth cost

Ir is the caller's inclusive cost into `grow_one`, `do_reserve_and_handle` or the map's `insert_full`.

| # | Site | Element | Calls | Growth Ir | Capacity, and why it is known |
|---|---|---|--:|--:|---|
| 1 | `llrm-analysis/src/alias.rs:3752` `annotated_with`, `llrm-analysis/src/memoryssa.rs:511` `Accesses::plain` | `IndexMap<InstId, MemRef>` | 167 | 1.57 (1.27 grow + 0.30 rehash) | the function's loads and stores: each one has a reference; a call with one (a fill) grows once more |
| 2 | `llrm-mir/src/module.rs:640` `Function::iter_block`, collected | `Vec<InstId>` | 8,278 | 1.18 | the block's `len`, kept by every edit: the iterator states it as its size hint |
| 3 | `llrm-mir/src/facts.rs:534` `Facts::of` | `Vec<Fact>` | 20,727 | 1.47 | at most one fact per attribute; mostly a first allocation, so expected small |
| 4 | `llrm-mir/src/domtree.rs:1973` `calculate_from_scratch` | `Vec<Option<BlockId>>` (`num_to_node`) | 3,104 | 0.53 | a full walk numbers at most every block, and the virtual root (LLVM's `SmallVector<NodePtr, 64>`) |
| 5 | `llrm-core/src/greedy/matrix.rs:174` `LiveRegMatrix::new` | `Vec<LiveIntervalUnion>`, `Vec<Query>` | 36 | 0.43 | the target's register units (LLVM's `Matrix.init(TRI->getNumRegUnits())`) |

## Not pre-sized

| Site | Growth Ir | Why |
|---|--:|---|
| `Function::log`, the change journal | 0.94 | grows for the function's life; no final size |
| `Function::create_instruction`, the instruction and block arenas | 1.28 | the count of instructions a pass will make is not known |
| `LiveIntervals::push_operand` | 0.74 | one small vector per value: first allocations |
| `Mssa::create_new_access` | 0.54 | bounded only by the instruction count, which over-allocates |
| the C front end (`selected_checking`: HIR operands, `Cow<str>`, `hir::unit`) | 1.73 | inlined into one symbol under fat LTO; site not identified |
| `String::write_str` under `format!` | 0.93 | formatting, not a sized collection |

## Results

Filled per commit: `I refs` before and after, d_faces object identical, objsweep 0 objects changed on the series head.
