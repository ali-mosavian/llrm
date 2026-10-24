# Intervals once (splitkit, w-1; 2026-10-24)

TL;DR: the pin builds live intervals once, 9.1 M Ir on d_faces -O2 m32; llrm builds them in three stages, about 24 M. One design: TwoAddress stops owning intervals, one set is built and handed down.

## Measured (ledger command; ours at 255a0f2c35, pin = clang profile)
| stage | llrm | pin |
|---|--:|--:|
| TwoAddress's own intervals (`LiveIntervals::new` 7 calls, lazy `compute_virt_reg_interval`) | 2.97 M + part of 13.7 M `tied` | none: `TwoAddressInstructionImpl::run` 1.20 M total, on `LiveVariables` (6.0 M `analyze`, a separate pass) |
| Coalescer (`interval_rc`, 1380 calls) | 3.6 M | uses the shared set |
| RegAlloc/greedy (`interval_rc`, 800 calls) | 13.0 M | uses the shared set |
| the one build | - | `LiveIntervals::analyze` 8.24 M + `SlotIndexes::analyze` 0.85 M = **9.1 M** |

Target: all interval building (every stage) at 9.1 M. TwoAddress's kill/def facts are separate (the pin's `LiveVariables`, 6.0 M); they must not be intervals.

## Design (one chain, owners and dates)
1. **greedy takes the coalescer's set** (splitkit, 11-09). `decide_with_split` builds `LiveIntervals::new(body)` again; the coalescer's `LiveIntervals`, updated in place by its joins, is handed to RegAlloc through the existing `liveintervals::hand_over` / `take_handed_over` (coalescer/env.rs, #4009). Saves the greedy recompute (13.0 M, minus what the constrain step must add: it creates the pinned copies, so `rewrite_instr`/`insert_instr` keep the set exact).
2. **TwoAddress stops owning intervals** (w-1, 11-13). It reads a kill/def table (`LiveVariables`' job: `getVarInfo`, kills per instruction) built in one pass over the body. `BodyFacts`' lazy intervals and `LiveIntervals::new` go (2.97 M + the lazy part of `tied`).
3. **one eager build before the Coalescer** (splitkit, 11-16): `computeVirtRegs` as the pin does, all vregs once, `SlotIndexes` once. The coalescer and greedy start from it; no stage computes an interval lazily (1380 + 800 calls become 0).
4. **per-call cost** of what is left (`LiveRangeCalc::extend` 5.8 K vs 2.7 K Ir per call, `LiveRangeUpdater::add` 205 vs 117): plan rows 1a-1c, 11-06/11-09.

Order matters: 1 first (largest, 13 M, needs only the existing handoff), then 2 and 3. Each step: ledger Ir before/after, objects unchanged on the 568 sweep. Owners tell the other when a step lands; w-1's TwoAddress callees (`replace` 3.43 M, `is_killed` 1.6 M, `_reused` 0.95 M) are its own and independent of this chain.

Not measured: what the constrain step's new vregs cost the shared set (step 1 prices it); the pin's `LiveVariables` against ours as a kill table.
