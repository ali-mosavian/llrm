---
architecture-index: the parity pipeline: plan
architecture-order: 80
---
# The parity pipeline

TL;DR: `LLRM_EXP_LLVM` becomes LLVM's -O1/-O2/-Os pipeline built from translated passes and analyses under one analysis manager; a translation joins when its oracle passes, not when it is default.

## Reference

The pinned pipeline per level, nested, with run counts: [slots](parity-pipeline-slots.md) (opt 24.0.0git `d1106deb71cc`, `-passes='default<O1|O2|O3>'`; -Os is the -O2 list on optsize IR). The llrm slot names and their `PassBuilderPipelines.cpp` lines are in `llvm_order` (`pipeline.rs`).

## What "joins" means in code

A registry `slot -> Translated | Old | Missing`. Each LLVM slot has one entry:

- `Translated(pass)`: the pass translated from the LLVM source at the pin. It runs in the slot under the parity switch.
- `Old(pass)`: llrm's stand-in runs; the entry names the translation that will replace it.
- `Missing`: LLVM runs something there, llrm nothing.

A translation joins (`Old` to `Translated`) when its oracle passes: output identical to `opt` at the pin on the corpus (llvm-difftest), plus the pass's own fail-first tests. Default-on is a separate decision, after the rows rule.

Each old analysis keeps a zero-callers counter (`LLRM_DEBUG=analyses`): when no joined slot asks for it, it is deleted.

## First joins

| slot | translation | oracle |
|---|---|---|
| early-cse (two slots) | `earlycse.rs` (#2164) | difftest vs `opt -passes=early-cse` |
| mem2reg | `mem2reg.rs` (#2130) | difftest vs `opt -passes=mem2reg` |
| sroa | `sroa.rs` (spike) | difftest vs `sroa<preserve-cfg>` |
| gvn | gvn-port | difftest vs `gvn`, `gvn<memoryssa>` |
| loop-rotate, unroll, peel, unswitch, deletion | loop-passes-port (#2234) | difftest per pass; acceptance rows in rotate-refusals (PR 2235) |

## Run counts per level

The run count per slot must match LLVM's. `parity::RUNS_PER_BODY` states it and `check_run_counts` checks a compile's `LLRM_DEBUG=runs` lines: `gvn@787` once per body. On d_faces -O2 under the order `gvn` runs 12 times over 11 functions (clang 11): the twelfth is the one body the interprocedural stage (`ipa-range`, after the function pipeline here, `ipsccp` before it in LLVM) sends back through the Simplify list. The old rounds ran it 35 times. At -O1 the order still runs `gvn` (11 times); clang's -O1 has none, a levels row. `loopsimplify`/`lcssa` run at three unconditional slots per body (x_hanoi2: ours 9/9/15/9 at -O1/-O2/-O3/-Os against LLVM's 4/10/10/10): stated in `parity::RUN_COUNTS_OPEN`, not yet checked.

## A/Bs against the old rounds

`LLRM_NO_LLVM_ORDER` also turns the translated greedy allocator off (`allocate.rs`), so an order-against-old A/B mixes the pass order with the allocator. `LLRM_NO_GREEDY` isolates the allocator: compare order with and without it before reading a regression as the order's.

## Recount of the registry

`LLRM_DEBUG=counts llrm-c -O2 f.c -S -o /dev/null 2>&1 | grep '^\[parity\]'` prints the registry's recount from `parity.rs` of the build it runs, the same at every level (it counts the registry, not the built list): all slots by state (Partial counted apart from Translated), each phase's slots, the analyses.

At `a94491462` (read from the binary and from `parity.rs`, which agree):

| | translated | partial | old | missing |
|---|---|---|---|---|
| all 88 slots | 23 | 6 | 51 | 8 |
| early | 2 | 0 | 2 | 0 |
| cleanup | 1 | 0 | 4 | 0 |
| simplify | 18 | 4 | 31 | 5 |
| late | 1 | 0 | 0 | 0 |
| module | 0 | 0 | 0 | 0 |
| backend | 1 | 2 | 14 | 3 |
| analyses | 2 | 0 | 0 | 0 |

After the analyses rows (every analysis the manager names has one) and the LSR row: 89 slots 23 / 9 / 49 / 8, analyses 7 / 1 / 37 / 0. `decide@1178` is Partial since the translated SimplifyCFG runs in it (#2907). `parity::check_phase` now fails when a slot the registry calls Old or Missing holds a translated pass (`FunctionPass::translation`), so a wiring PR updates its row in the same commit.

Since `d92949c1e` (22/6/52/8) one slot changed state: `gvn@787` Old to Translated. The Partial notes of `jumpthreading@668/@809` and `dse@820` moved their stage. Stage landings in slots the registry counts as one row (SimplifyCFG pieces under `decide@N`, `indvars@738`, LSR, jumps) do not move the count until the row's own criterion is met.

Off by default: `rotate@728` (`LLRM_ROTATE`). `jumpthreading@668/@809` run by default; `LLRM_NO_JUMPTHREADING` (set) turns them off. Every other Translated or Partial slot runs under the order; `LLRM_NO_LLVM_ORDER` turns the order off.

Callers of analyses (`callers-total`, joined / old / analyses with old callers): x_hanoi2 -O1 157 / 365 / 34, -O2 167 / 387 / 34, -O3 224 / 565 / 34; `d_faces` -O2 767 / 1701 / 37.

Every analysis the manager computes has a row in `parity::ANALYSES` and a `[parity] analysis <key> <kind> <type>` line. An Old row says what replaces it (`replaced_by`; "none" is a DEVIATE or delete candidate). `tools/old_analysis_callers.py` reads its list of old analyses from these lines, not from a file of its own.

## Scoreboard

What exists: `LLRM_DEBUG=runs` prints each step's own work with the body's `in=`/`out=` instruction counts and the function name, and `LLRM_DEBUG=counts` prints `callers <analysis> J joined O old`: how many times a joined (Translated) slot and an old one made the manager compute each analysis; an analysis with no old caller can be deleted. A per-slot instruction table is built by hand from the runs lines; there is no automatic table yet.

## Road to parity

Target: every bucket at most 1.5x clang's, `d_faces` -O2. Figures are M instructions, llrm / clang, from llvm-difftest's projection (callgrind, per-pass inclusive; per-pass llrm figures are upper bounds). Owner `?` is unassigned.

| bucket | llrm / clang M | translations | owner | blocker | target M |
|---|---|---|---|---|---|
| SimplifyCFG + JumpThreading | 343 / 20 | `SimplifyCFG.cpp`, `JumpThreading.cpp` | scalar-cleanup-port | DomTree and LVI joins; run counts | 30 |
| InstCombine | 243 / 42 | `InstCombine*.cpp` onto InstSimplify | fold-poc | InstSimplify join | 63 |
| regalloc | 276 / 42 | `RAGreedy`, `SplitKit`, region splitting | regalloc-core-poc | greedy not default | 63 |
| llrm-only analyses | 599 / 57 | BasicAA, SCEV, BPI/BFI, MemorySSA, LVI behind the managers | cgscc-poc, scev-port | BasicAA S3 and SCEV S3 migrations | 86 |
| glue | 390 / 99 | CGSCC FunctionAttrs walk, emission, C front end | cgscc-poc, backend-slopes, hir-branch-icmp-2211 | glue-plan steps | 149 |
| loop passes + LSR | 120+ / 10+ | rotate, unroll, peel, unswitch, `LoopStrengthReduce.cpp` | loop-passes-port, loop-products-slope | SCEV S3 to S5 | 15 |
| machine clean-up + FPS | 171 / 23 | MachineCopyPropagation, `X86FloatingPoint.cpp` | backend-slopes | greedy default | 35 |

Order:

1. Join the existing translations: EarlyCSE, SROA/mem2reg, GVN, InstSimplify, DomTree. Done when GVN's body is at most 1.2x clang's and the run counts are equal.
2. SimplifyCFG, InstCombine, LICM.
3. Migrations off BasicAA S3 and SCEV S3. Critical path: the loop, BPI/BFI and LSR rows wait on them.
4. Backend: greedy default, MachineCopyPropagation, FPS.
5. Glue: the CGSCC FunctionAttrs walk, emission, the front end.
6. In parallel: ilist block storage and use lists.

### Scoreboard table

llvm-difftest fills this every half hour; one row per bucket, instrument stated in the header.

| bucket | llrm M | clang M | ratio | target M | main |
|---|---|---|---|---|---|
| SimplifyCFG + JumpThreading | | | | 30 | |
| InstCombine | | | | 63 | |
| regalloc | | | | 63 | |
| llrm-only analyses | | | | 86 | |
| glue | | | | 149 | |
| loop passes + LSR | | | | 15 | |
| machine clean-up + FPS | | | | 35 | |
| total | | | | | |
