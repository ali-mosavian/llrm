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

The run count per slot must match LLVM's. Today `loopsimplify`/`lcssa` run 3 times per body at every level against LLVM's 1 to 1.5 per function (d_faces -O2: 36 and 11 bodies vs 16). The fix is a row of this page.

## Scoreboard

Under the parity switch `LLRM_DEBUG=time` prints instructions per slot, and the rows report the per-slot totals next to LLVM's (`-time-passes`-equivalent). Not built yet.
