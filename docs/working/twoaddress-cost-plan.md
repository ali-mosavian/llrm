# TwoAddress and LiveIntervals: compile cost against the pin (owner w-1)

d_faces `-m32 -mabi=sysv -march=i486 -O2`, dist build at parity e45901155 + #4120 (callgrind, inclusive Ir); the pin's numbers are `clang -O2` of the same file (the ledger's clang run, inclusive table).

| function | ours | pin |
|---|--:|--:|
| TwoAddress (`tied`) | 13.68 M | `TwoAddressInstructionImpl::run` 1.20 M |
| interval computation (`LiveIntervals::compute_virt_reg_interval`, all callers) | 20.78 M | `LiveIntervalCalc::calculate` 7.64 M |
| `LiveRangeCalc::extend` (all callers) | 22.39 M | 9.30 M |
| register coalescer (`copy_coalesce_work_list`) | 12.14 M | `RegisterCoalescer::run` 7.87 M |
| `LiveIntervals::new` in TwoAddress (7 functions) | 2.78 M | `SlotIndexes::analyze` 0.85 M |
| `handleMove` | not called yet (reschedules only) | 0.77 M |

Inside `tied` (13.68 M): `BodyFacts::replace` 3.43 M (149 calls: `into` and `first` recomputed for each tied pair), `LiveIntervals::new` 2.78 M, `is_killed` 1.58 M and `killed` 0.55 M (the lazy intervals of killed sources), `_reused` 0.95 M, `scan_uses` 0.67 M, hash growth 0.45 M, `_untied` 0.41 M.

Plan, one PR each, numbered rows:

1. **2026-10-30 — no recompute in `replace`**: the pin extends the destination's interval from the copy to the instruction and shrinks the source's end to the copy (`processTiedPairs`, LiveIntervals updates), where ours removes and computes both. Target: `replace` under 0.5 M; objects unchanged.
2. **2026-11-02 — `_reused` on the walk**: the update in place reads use counts from `LiveIntervals::operands` instead of its own passes over the body (0.95 M).
3. **2026-11-04 — the intervals are the coalescer's too**: count what TwoAddress computes lazily against what the coalescer then does not (the total, not the part, is the cost: #4009 read -0.06% on the ledger); drop `is_killed`'s copy-walk where the pin's `isKilled` does not make one.
4. **2026-11-06 — the shared structures**: `LiveIntervals::new` (maps by `HashMap<InstrKey>`, the CFG and the dominator tree built per function) against `SlotIndexes::analyze` and the pin's analyses it reads from the pass manager; and `LiveRangeCalc::extend`/`calculate` 2.4x the pin's, which every pass pays (splitkit's step E waits on it).

Each step: ledger d_faces Ir before and after, objects on the 568 sweep.
