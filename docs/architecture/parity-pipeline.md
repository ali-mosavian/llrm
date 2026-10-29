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


## Recount of the registry

The count of record is the registry's own recount: `LLRM_DEBUG=counts llrm-cc -O2 f.c -S -o /dev/null 2>&1 | grep '^\[parity\]'` prints it from `parity.rs` of the build it runs, the same at every level (it counts the registry, not the built list): all slots by state (Partial counted apart from Translated), each phase's slots, the analyses. No page or test pins a number: `parity_tests.rs` checks that the tally and the printed lines are the recount of the rows by kind, so a PR that moves a slot edits its row and nothing else.

`parity::check_phase` fails when a slot the registry calls Old or Missing holds a translated pass (`FunctionPass::translation`), and when a Translated or Partial slot is not run, so a wiring PR updates its row in the same commit. A Partial row says what is done and what is left (`criterion:` and `exit:`); stage landings inside a row the registry counts as one slot (SimplifyCFG pieces under `decide@N`, LSR, jumps) do not move the count until the row's own criterion is met.

Every analysis the manager computes has a row in `parity::ANALYSES` and a `[parity] analysis <key> <kind> <type>` line. An Old row says what replaces it (`replaced_by`; "none" is a DEVIATE or delete candidate). `tools/old_analysis_callers.py` reads its list of old analyses from these lines, not from a file of its own.

`rotate@728` is on (off only where unroll-and-jam runs). `jumpthreading@668/@809` run by default; `LLRM_NO_JUMPTHREADING` (set) turns them off. Every other Translated or Partial slot runs under the order; `LLRM_NO_LLVM_ORDER` turns the order off.

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

## Backend DEVIATEs

| id | where | deviation | owner, date | exit |
|----|-------|-----------|-------------|------|
| D-SB1 | `isel/wide.rs` `narrow_divisor`, the idiv test; `isel.rs` `significant_bits` | the divisions count sign bits with the old `valuetracking::sign_bits`, not the translated `compute_num_sign_bits`: with it, x_ll_div ran +8% and switch +10% in clocks (two dword `idiv`s for the helper; `x % 65537` as `idiv` for the reciprocal) | w-1, 2026-10-20 | price two dword divisions against the helper, and the reciprocal against `idiv`, from the target description's table; then use the translated count |
