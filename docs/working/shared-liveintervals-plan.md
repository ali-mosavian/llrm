# TwoAddress on a shared LiveIntervals (row 62, row 62b) — owner w-1, due 2026-11-20

Why: TwoAddress costs 7.84 M Ir on d_faces -O2 m32 against the pin's 1.07 M. The pin builds one `LiveIntervals` per function, TwoAddress edits it, the Coalescer reads it; ours builds `BodyFacts` in TwoAddress (DEVIATE TA-6) and a fresh `LiveIntervals` in the Coalescer (`coalescer/env.rs::joined`).

Pieces, each its own PR:

1. **2026-11-06 — `LiveIntervals::handleMove`** (LiveIntervals.cpp#L1623; `HMEditor::updateAllRanges`, `updateRange`, `handleMoveDown`, `handleMoveUp`, `findLastUseBefore`, not `updateRegMaskSlots` (no regmask slots), not `handleMoveIntoNewBundle` (no bundles)) as methods in `liveness/liveintervals.rs`. Test: after a move, the intervals equal those built afresh (`assert_intervals_equal_afresh`), over a corpus of moves up and down across defs, uses and kills. Not wired.
2. **2026-11-13 — TwoAddress on `LiveIntervals`**: `BodyFacts` replaced by one `LiveIntervals::new` per function; kills read off the intervals as the pin (`isPlainlyKilled`, `isKilled`); `rescheduleMIBelowKill`/`AboveMI` and the 3-address conversion edit it (`handleMove`, `insert_instr_*`, `rewrite_instr`). Test: `live_rows` walks in TwoAddress = 0, `LiveIntervals::new` once per function (`LiveIntervals::BUILT`). Exits TA-6; `_reused` folds into the same edits.
3. **2026-11-20 — Handoff to the Coalescer**: the edited `LiveIntervals` is passed with the body, and `joined` takes it when the body is the one it was built for (identity of the block `Arc`s), else builds. Test: `LiveIntervals::new` once for TwoAddress + Coalescer.

Cost, each piece: d_faces `-m32 -mabi=sysv -march=i486 -O2` (ledger command), base vs head, Ir by phase. Objects must not change (568 sweep); a change is a numbered row.
