# Analyses gcc and LLVM do not compute

TL;DR: analyses are 1,278 M of d_faces -O1's 6,246 M (20%). Of that, about 300 M is recomputes that came out equal, and the idle passes' share is mostly the analyses they ask for first. gcc and LLVM do not keep most of these as whole-function results: they ask per query, or store the fact on the IR. That, not an update rule per analysis, is the next structural step.

Measured on `d_faces.c` -O1, default flags, main 736e47aab: `LLRM_DEBUG=time` (own M), `LLRM_WHY` (recomputes and how they came out), edit classes of each recompute (`meas/runs/whyk.txt`). "Equal M" is equal recomputes times the analysis's mean M per recompute, so a bound, not a measurement. The idle passes' work (796 M, hoist 197, indvars 102, decide 100, gvn 85) is these analyses computed for a pass that then found nothing: it is not added again here.

## Where it sits

| analysis | own M | recomputes | equal | equal M | what gcc / LLVM do instead |
|---|---|---|---|---|---|
| through-memory | 163 | 63 | 30 | 78 | No stored whole-function solve. gcc's FRE/CCP/DOM value-number inside the pass, walking virtual use-def chains with an alias-query budget (`sccvn-max-alias-queries-per-access`); LLVM's GVN asks MemorySSA or MemDep per load under a scan limit. Per pass run, per load. |
| points-to | 137 | 181 | 37 | 28 | gcc solves once per function (`pass_build_alias`, about 2 per function) and stores the set on each SSA name; an edit that makes no new pointer leaves it valid, a new name gets the conservative set. LLVM has no solver in the default pipeline: BasicAA is stateless per query. |
| globals-aa | 125 | 12 | - | - | LLVM's GlobalsAA is a module result, built once and kept while the functions' summaries hold. gcc's `ipa-pure-const` and modref summaries are computed in the IPA stage once. Ours builds 12 times. |
| call-effects | 122 | 113 | 48 | 52 | The effect of a call is read off the callee's attributes (LLVM `getModRefInfo(call)`; gcc `gimple_call_flags` and the modref summary): a lookup, no per-body analysis. |
| bounded | 117 | 51 | 0 | 0 | gcc's ranger and LLVM's LazyValueInfo are on demand with a per-block cache; nothing is computed for a value nobody asks about. (ranger's.) |
| pointer-values | 102 | 181 | 62 | 35 | The first phase of points-to: as above. |
| float-facts | 98 | 48 | 16 | 33 | Neither has the analysis. LLVM `computeKnownFPClass` runs on demand with a depth limit; gcc has frange in ranger. |
| annotated | 88 | 125 | 64 | 45 | The reference of a load or store is built when asked: LLVM `MemoryLocation::get(inst)`, gcc `ao_ref_init`. No table. |
| dominated-edges | 78 | 197 | 23 | 9 | Ranger's gori edge facts, on demand. (ranger's.) |
| registers | 66 | 207 | 10 | 3 | LLVM `computeKnownBits` is stateless, depth 6. gcc keeps nonzero bits and ranges on the SSA name. |
| counted | 47 | 192 | 52 | 12 | LLVM SCEV on demand, cached per loop and value; gcc `number_of_iterations_exit` per loop when a pass asks. (ranger's.) |
| exposed-frames | 20 | 260 | 106 | 8 | A flag on the declaration (gcc `TREE_ADDRESSABLE`); LLVM `PointerMayBeCaptured` per query, capped at 100 uses. |
| accesses (memory SSA) | 20 own, 339 with the analyses under it | 151 | 22 | 3 | gcc's virtual operands are part of the IR, always current; LLVM builds MemorySSA when a pass needs it and updates it with `MemorySSAUpdater`: 2 builds per function, here 9. |
| loops, dominators, shape | 34 | 532 | - | - | Maintained: gcc keeps dominance and the loop tree up to date through TODO flags; LLVM updates the tree with `DomTreeUpdater` and keeps `LoopInfo` preserved. |
| published, assumptions, followers | 22 | 316 | 213 | 14 | No counterpart; the assumption cache is LLVM's, kept up to date through handles. |

## What follows

1. A fact stored on the IR beats an analysis kept in step with it. gcc's points-to sets, nonzero bits and addressable flags live on the SSA name or declaration, are copied when a value is copied and are valid for a new value as the conservative set. That removes the equal recomputes by construction, with no per-edit proof. Candidates: points-to (and so pointer-values, call-effects' escape phase), exposed-frames, registers.
2. A table built for every access (annotated, through-memory, float-facts: 349 M own) is a per-query answer in both compilers. The passes that ask (gvn, dse, hoist, promote) ask of a few loads. This is the largest single item and a change in how the passes ask, not in how it is kept.
3. Ranger's three (bounded, dominated-edges, counted: 242 M) are whole-function tables; both compilers compute them per query. The owner's call.
4. The rule that an edit class proves a result unchanged is already in the tree (`Analysis::SKIPS`, `Depends`) for the pointer family. Applied to what still lacks it (assumptions, followers, published) it saves about 14 M.

Recommended next step: 1 for points-to, because it is both the largest recompute count (181, against gcc's 2) and the one gcc solves in a way that does not need proving. Sizing it needs the points-to owner's view of what an edit that makes no pointer does to the sets.
