# Allocation by caller (d_faces, -O2, m32 i486)

TL;DR: allocation plus its copying is 69.8 M of 740.8 M Ir (9.4%) and flat: the top 15
callers hold 32% of it. No single mechanical cut is worth a PR. `successors` and
`predecessors` allocate nothing (inline `BlockList`); their 6.8 M each is traversal.

Command: `llrm-cc -m32 -mabi=sysv -march=i486 -O2 -Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near=` with `-I` of `qcstub/src/{host,render,model,game,sound,ui,qgl}` and `qctc/inc`, on `render/d_faces.c`.
Method: `valgrind --tool=callgrind --separate-callers=4` on the line-tables release
`llrm-cc` (built `-p llrm-cc --bin llrm-cc`), then `allocby.py` sums `__rust_alloc`,
`__rust_dealloc`, `__rust_realloc`, `__rust_alloc_zeroed` and `memcpy` by the first
`llrm_` frame of the chain. Plain profile for inclusive numbers.
Bytes are not in a callgrind profile; sizes below are element sizes.

Total here is 740.8 M against w4's 936.9 M for the same file: the inputs differ
(command line not shared); shape agrees (finish_grow 24.3 M vs 29.8 M).

| # | Caller | Ir | Calls | Pin equivalent | Cut |
|---|---|---|---|---|---|
| 1 | `RawVec::finish_grow` memcpy (all growers) | 6.68 | 1735 | `SmallVector` inline, grow rare | reserve at the sites below |
| 2 | `llrm_c::hir::unit` | 1.51 | 37758 | no HIR | reuse buffers across units |
| 3 | `Function::create_instruction` | 1.29 | 174 | `BumpPtrAllocator` | `with_capacity` for the instruction arena |
| 4 | `Function::fork` | 1.19 | 47696 | `CloneFunction`, one pass | clone vectors by length, once |
| 5 | `peep::walk::windows` | 1.11 | 57640 | `MachineInstr` iterators, no copy | iterate in place |
| 6 | `llrm_c::stream::parse` | 1.00 | 5750 | lexer, no alloc | size the token vector from input length |
| 7 | drop glue (all) | 0.93 | 58477 | arena, no frees | follows the allocators above |
| 8 | `AggregateAA::add_aa_result` | 0.90 | 37388 | `AAResults` virtual chain, no vector | fold in place |
| 9 | `Function::log` | 0.89 | 45 | none | `with_capacity` |
| 10 | `simplify_instruction_with` | 0.83 | 48771 | `SimplifyQuery`, stack | small-vector worklist |
| 11 | `allocate::live_rows_numbered` | 0.77 | 15764 | `LiveIntervals`, per-vreg | keep rows across calls |
| 12 | `translate::Body::instruction` | 0.75 | 8458 | `IRBuilder`, arena | reserve operands |
| 13 | `incremental::Kept::ask` | 0.67 | 27352 | none | memo key without allocation |
| 14 | `Semantics::clone` | 0.57 | 23816 | `MachineInstr` operands in place | share through `Arc` |
| 15 | `Insn::try_renamed` / `_placed_for_rewrite` | 0.55 / 0.55 | 11079 / 11499 | operand rewrite in place | rewrite in place |

Ir in millions, callee cost only (alloc, free, copy), not the caller's own work.

`Function::successors` 6.85 M and `predecessors` 6.87 M: callers are `LazyValueInfo::solve`
(1.26 M, 6649 calls), `collect_transitive_predecessors`, `simplifycfg`, `mem2reg`. About
200 Ir per call walking the terminator and the use list. The cut is fewer calls
(cache per block in `LazyValueInfo`), not allocation.

`Vec<Loc>::clone` 5.3 M and `Semantics::clone` 5.7 M are inclusive clone cost, of which
alloc is 0.57 M: the rest is copying and refcounts.
