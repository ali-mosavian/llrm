---
architecture-index: Retiring the old Shape analysis
architecture-order: 277
---
# Retiring the old `Shape` analysis: survey and plan

TL;DR: `cfg::Shape` is three parts, and two are already the translated stack: `dominance` is the lazy `DomTree` handle, `loops` is `natural()` over `LoopInfo`. Only `reaching` (one reader) has no translation yet. Retire it as: `isPotentiallyReachable` (R1), the readers move to `Dominators`/`Loops` directly in three groups (R2 analyses, R3 loop transforms, R4 the rest), then delete `Shape` and the manager's accessors (R5).

Counts are textual (greps of `Shape::of`, `.shape()`, `get::<Shape>`, `.dominance`, `.loops`, `.reaching` per file), taken at the head of `feat/parity`; llrm-codegen's `analysis/graph.rs` `Shape` is another type (LIR labels and successors) and out of scope. The reader PR re-counts with the compiler by deleting the field.

## Parts and replacements

| part | what it is now | replacement | readers |
|---|---|---|---|
| `dominance` | `cfg::Dominance`, lazy `DomTree` handle (`tree`, `dominates`, `immediate`, `*_resolved`) | `Dominators` analysis result / the handle held by the unit; no copy in `Shape` | see table |
| `loops` | `Vec<cfg::Loop>` (header, latches, body) from `natural(LoopInfo)` | the `Loops` analysis (`LoopInfo`, generic over the CFG in #2228) and its `Loop` handle | see table |
| `reaching` | per-target closure of predecessors, memoised | `isPotentiallyReachable` ([CFG.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CFG.cpp#L131)): dominator-tree and loop shortcuts, one bounded search per query (`DefaultMaxBBsToExplore` = 32) | `alias.rs` (`shape.reaching.to`) |

## Readers

Group A analyses (R2), B loop transforms (R3), C the rest (R4). `of` = `Shape::of` calls, `get` = manager/unit accessors.

| group | file | of | get | reads |
|---|---|---|---|---|
| A | `llrm-analysis/alias.rs` | 0 | 2 | dominance, loops, reaching |
| A | `llrm-analysis/assumptions.rs` | 0 | 0 | dominance |
| A | `llrm-analysis/bin/shadow_mssa.rs` | 0 | 2 | dominance |
| A | `llrm-analysis/blockfreq.rs` | 0 | 1 | - |
| A | `llrm-analysis/branchprob.rs` | 0 | 0 | loops |
| A | `llrm-analysis/capturecheck.rs` | 1 | 0 | - |
| A | `llrm-analysis/consts.rs` | 0 | 2 | dominance, loops |
| A | `llrm-analysis/floatfacts.rs` | 0 | 1 | loops |
| A | `llrm-analysis/functionattrs.rs` | 1 | 0 | loops |
| A | `llrm-analysis/globalsaa.rs` | 1 | 1 | - |
| A | `llrm-analysis/guards.rs` | 0 | 3 | dominance, loops |
| A | `llrm-analysis/induction.rs` | 0 | 6 | dominance, loops |
| A | `llrm-analysis/manager.rs` | 1 | 18 | dominance, loops |
| A | `llrm-analysis/memory.rs` | 2 | 0 | - |
| A | `llrm-analysis/mssa/shadow.rs` | 0 | 2 | dominance |
| A | `llrm-analysis/parameter_ranges.rs` | 1 | 0 | - |
| A | `llrm-analysis/ranger.rs` | 0 | 4 | dominance |
| A | `llrm-analysis/ranges.rs` | 0 | 2 | dominance, loops |
| A | `llrm-analysis/regions.rs` | 0 | 3 | - |
| A | `llrm-analysis/slotstores.rs` | 0 | 1 | dominance |
| C | `llrm-transforms/addresssink.rs` | 1 | 0 | dominance |
| C | `llrm-transforms/algebraic.rs` | 2 | 1 | loops |
| C | `llrm-transforms/candidate.rs` | 2 | 0 | loops |
| C | `llrm-transforms/deadstore.rs` | 0 | 1 | dominance, loops |
| C | `llrm-transforms/decide.rs` | 2 | 0 | loops |
| C | `llrm-transforms/earlycse.rs` | 0 | 1 | dominance |
| B | `llrm-transforms/exitfold.rs` | 0 | 6 | dominance, loops |
| B | `llrm-transforms/exitsink.rs` | 0 | 0 | loops |
| B | `llrm-transforms/floatloop.rs` | 1 | 1 | loops |
| C | `llrm-transforms/gvn.rs` | 0 | 0 | dominance |
| B | `llrm-transforms/indvars.rs` | 0 | 1 | dominance, loops |
| C | `llrm-transforms/interprocedural.rs` | 0 | 1 | - |
| C | `llrm-transforms/ipacp.rs` | 1 | 0 | loops |
| C | `llrm-transforms/jumpthread.rs` | 2 | 1 | dominance, loops |
| B | `llrm-transforms/lcssa.rs` | 2 | 0 | loops |
| B | `llrm-transforms/licm/mod.rs` | 0 | 2 | dominance, loops |
| B | `llrm-transforms/licm/promote.rs` | 0 | 0 | dominance |
| B | `llrm-transforms/licm/safety.rs` | 0 | 0 | dominance |
| B | `llrm-transforms/licm/sink.rs` | 0 | 0 | dominance |
| B | `llrm-transforms/loopdistribute.rs` | 2 | 0 | loops |
| B | `llrm-transforms/loopfuse.rs` | 1 | 0 | dominance, loops |
| B | `llrm-transforms/loopidioms.rs` | 1 | 0 | loops |
| B | `llrm-transforms/loopinterchange.rs` | 1 | 0 | loops |
| B | `llrm-transforms/loopsimplify.rs` | 2 | 0 | dominance, loops |
| B | `llrm-transforms/loopsplit.rs` | 2 | 0 | dominance, loops |
| B | `llrm-transforms/loopversion.rs` | 1 | 0 | loops |
| B | `llrm-transforms/lsr.rs` | 1 | 2 | dominance, loops |
| C | `llrm-transforms/memcpyopt.rs` | 0 | 1 | - |
| C | `llrm-transforms/mirror.rs` | 0 | 1 | loops |
| B | `llrm-transforms/peel.rs` | 1 | 0 | loops |
| C | `llrm-transforms/pipeline.rs` | 1 | 0 | dominance, loops |
| C | `llrm-transforms/profit.rs` | 3 | 1 | loops |
| B | `llrm-transforms/rotate.rs` | 1 | 0 | loops |
| C | `llrm-transforms/spares.rs` | 0 | 1 | loops |
| B | `llrm-transforms/trivialunswitch.rs` | 1 | 0 | loops |
| B | `llrm-transforms/unroll.rs` | 1 | 0 | loops |
| B | `llrm-transforms/unroll_jam.rs` | 1 | 0 | loops |
| B | `llrm-transforms/unswitch.rs` | 1 | 0 | - |
| C | `llrm-transforms/window.rs` | 0 | 0 | loops |

## Stages

| stage | what | date |
|---|---|---|
| R1 | `isPotentiallyReachable` over `DomTree` + `LoopInfo`; `alias.rs` reads it; fail-first: a query the closure answered by a full scan now answers by the shortcut with the same result, with a work counter | 10-23 |
| R2 | group A reads `Dominators`/`Loops` from the manager, not `Shape` | 10-25 |
| R3 | group B (loop passes) reads `Loops`; `cfg::Loop` users move to `LoopInfo`'s `Loop` | 10-27 to 10-30, in two PRs |
| R4 | group C | 10-31 |
| R5 | delete `Shape`, `Reaching`, `natural`, the manager's `Shape` accessors; registry row closed | 11-01 |

Each reader PR: no behaviour change (the same output on the corpus), every `Shape::of` removed in its files, counts re-run.
