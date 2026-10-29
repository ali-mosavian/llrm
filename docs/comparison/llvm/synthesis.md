# LLVM compared: every cause of the gap, ranked

TL;DR: llrm compiles `d_faces` -O2 in 2,903 M instructions, LLVM in 415 M (6.8x; 4.8x to 6.4x in cycles, 13.9x in L1d misses). The measured causes, largest first: the optimizer's fact tower (651 M of analyses), allocation and copying (518 M, cutting across the rest), the register allocator (444 M against 39.5 M), passes whose cost grows faster than the body, runs that change nothing (224 M of their own work), the LIR clean-up (about 340 M against about 40 M), bookkeeping (about 128 M) and the front end (125 M against 33 M). Instruction selection is at parity. Of the rows below, four are landed or in flight; the unowned ones are marked.

> **Reference note.** The LLVM allocator figures on this page (greedy 39.5 M, 815 assignments, 78 evictions, 122, 13/22/33 K per decision) were taken from the distro `llc` 20.1.8, are not reproduced on the pinned or the distro `llc`, and are being re-taken on the pinned 24.0.0git `d1106deb71cc` by regalloc-core-poc. Do not quote them until its old-versus-new lands.

Shares are of the `d_faces` -O2 compile under callgrind (2,903 M; the flat profile uses 2,874 M), `-m32 -march=i486 -fno-inline-functions`, llrm main `21f5c9fc8`, LLVM clang 24 at `d1106deb71cc`. Rows overlap: allocation is inside the analyses and the passes, and a pass's inclusive cost holds the analyses it asks for. A row's share is therefore its own measurement, not a slice of a sum. Status: landed (PR merged), in flight (PR open or designed with an owner), not kept (measured, rejected), unowned.

## Ranked causes

| # | cause | measured on `d_faces` -O2 | LLVM mechanism that answers it | status |
|---:|---|---|---|---|
| 1 | The fact tower: analyses recomputed whole-function after changes, queries that re-walk | analyses 651 M of 2,830 M (23%); walker 133 M (2.5%), 65% of its queries repeat a key; `through-memory` quadratic (3.7x per doubling) | analysis manager with preserved sets ([analysis manager](analysis-manager.md)), MemorySSA `OptimizeUses` and walker caps ([memory SSA](memoryssa.md)), stateless BasicAA with depth limits ([alias](alias.md)) | in flight: lazy per-version facts (keep-analyses, design), walker tables (greedy-port); not kept: per-block write summaries (-0.4%), walker cache (ceiling 1.4%, not built) |
| 2 | Allocation and copying | 518 M (18.0%); 3,765,923 allocations against 62,881; 24% of D1 misses; about 100 instructions per allocation | inline `SmallVector`, operands co-allocated in the instruction, bump arenas ([allocation](allocation.md)) | landed: successors inline (#1956, -26 M of 258,958 allocations in a prototype), printed-body history gone (#1931); in flight: `env_set` once (#2007); unowned: inline operands, scratch reuse, presized vectors, B-trees to dense sets (51 M self) |
| 3 | Register allocator | `regalloc base` 444 M against greedy 39.5 M; 79 splits against 3 on the diamond shape, 419 stack operands against 5 | `trySplit` ladder, `SplitEditor` editing in place, interference cache ([register allocation](regalloc.md)); CodeGenPrepare sinks the repeated mask test | in flight: mask test at each branch (#2010, ssa-spill-m16); design: structural sharing of LIR rewrites, `Blocks::of` once per round; effect on `d_faces` unmeasured |
| 4 | Passes that grow faster than the body | `gvn` 249 M, `fold` 267 M, `hoist` 168 M, `lsr` 149 M, `decide` 135 M, `dse` 54 M (inclusive, with their analyses); exponents 1.2 to 1.4 for the optimizer, `dse` and `through-memory` 4.0x and 3.7x per doubling | per-pass scan limits and caps ([GVN](gvn.md), [LICM](licm.md), [SimplifyCFG](simplifycfg.md)) | in flight: `gvn` linear (#1999) and `lsr` linear (#2008), both about 0 on `d_faces`; `fold`: the float half is 45% of a run ([page](fold-hoist-decide.md)); the float run is a lazy-facts item (keep-analyses' design); the `FloatFacts` filter is not kept (6 of 20 solves skipped, +0.04%) and preserving analyses across the integer half is dropped (about 1% ceiling); `dse` sparse set (edit-rollback, PR coming); unowned: `hoist`, `decide` |
| 5 | Runs that change nothing | 637 of 722 itemised runs; own work 224 M (7.8%), 362 M with the analyses the next pass would ask anyway | last-run tracking, a fixed list run once per position ([idle runs](idle.md)) | measured, no cheap sound check: the edit digest nets +0.6%, a pass-level enabler table skips 0.6% and is not exact; the idle cost is the facts the pass asks for, keep-analyses' |
| 6 | LIR clean-up | `lir peephole` 130.7 M, `jumps` 65.4, `coalesce` 60.2, x87 70.9, `phielim` 14.4; LLVM about 40 M for the same work | DAG combine at selection (20.1 M), `PeepholeOptimizer`, block placement ([LIR clean-up](lir-cleanup.md)) | in flight: liveness owner and dirty blocks (ssa-spill-m16, roadmap step 6); not kept: shared lane and exit facts (at most 29 M, 1.0%, dropped as not worth a PR); unowned: gating the rule groups; ranks last: x87 live rows (25.7 M) |
| 7 | Bookkeeping | printed-body history 46 M, `Stamp` 49 M, verifiers 22 M, `env_set` 11 M (about 4.4%) | none; the pipeline is a fixed list | landed: digest (#1931), `env_set` read once (#2007), verifiers off in release (#1963: no `verify` frame in the profile); `Stamp` is real work (PostOrderFunctionAttrs, 108 M in three runs, 88 M of it `alias::initialized`): keep-analyses' lazy facts, skipping unchanged bodies saves 2 M |
| 8 | Front end and the rest | 81 M against 33 M (2.5x): the Open Watcom front end child 18, `translate` 42, `hir to mir` 15, `hir::unit` 6; the 30 M of HIR verification is off since #1963 | clang's front end | unowned |
| 9 | More rounds than a fixed list | the third round of -O2 is 211 M (6.3%); `gvn` ahead of `decide` -4.4% at -O2, +5.4% at -O3 | one fixed function pipeline ([GVN](gvn.md#proposed-change)) | not kept: fewer rounds (#1933, #1960, 35 cells worse), `gvn` first (rows lost), early scoped CSE (saves 3.1% to 3.4%, 4 cells worse), two rounds (not built) |
| 10 | Instruction selection | 54.6 M against 51.3 M | `SelectionDAGISel` | parity, nothing to do |

## What the cache adds

The 13.9x in L1d misses is not a tenth cause: it follows rows 1 to 3. Allocation and copying are 24% of the misses, the memory-fact tables 14%, register-allocation tables 9% and IR walks 7% ([flat profile](profile.md)). Live data at the peak is 23 MB against 5.5 MB in `cc1`, mostly the backend's copies of LIR; with mimalloc the resident set reaches 82 MB, 44 MB with the system allocator. No miss problem reaches the last level (0.67 M against 0.25 M LL misses).

## What is not a cause

- Caps: llrm's `gvn` walk stops at 100 writes as LLVM's memory dependence scans 100 instructions; the caps are alike ([GVN](gvn.md)).
- Fixed cost: an empty function costs 10 M in llrm and 15 M in clang ([index](readme.md#the-ladder)).
- Run count: 746 runs against 856; the cost of a run is the gap.
- `fold` against InstCombine per run on synthetic bodies: 36.8 and 29.1 M on the `mem` shape. On `d_faces` the per-run cost is 12x (5.4 M against 0.45 M) while the total is 4.4x, because llrm runs `fold` 36 times and LLVM runs InstCombine 88.

## Unowned, in the order of what they are worth

1. Allocation: inline operands, scratch reuse, presized vectors, dense sets instead of B-trees (row 2; bound 518 M less what #1956 took).
2. ~~The work check before analyses in idle runs~~ (row 5): closed, measured and not built; the idle run's cost is its facts (keep-analyses).
3. `hoist`, `fold`, `decide` per-run cost (row 4; 168, 267 and 135 M inclusive).
4. Front end (row 8; 92 M above LLVM's).
5. ~~`Stamp` and the verifiers~~ (row 7): the verifiers are gone from release, `Stamp` is keep-analyses' (`alias::initialized`).
6. Gating the peephole rule groups (row 6).

Open questions: [#1994](https://github.com/ali-mosavian/llrm/issues/1994).
