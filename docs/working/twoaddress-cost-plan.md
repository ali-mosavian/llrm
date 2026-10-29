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

## Status at the tip (2026-10-24) and TA-R1
`tied` is 9.14 M Ir on d_faces -O2 (11.73 M at #4121), against the pin's `TwoAddressInstructionImpl::run` 1.20 M plus its `LiveVariables::analyze` 6.0 M, a separate pass: 1.27x. `BodyFacts::of` is 4.50 M of it (the dataflow 3.4 M, LV3, exit 2026-12-11). Done: `replace` -0.35 M (#4312), `_reused` -0.74, `process_copy` -0.76, the maps -0.22, in-place intervals -2.5 (#4188, since replaced by LV3).

TA-R1 (numbered row, owner w-1, exit 2026-11-14): what is left, each under 0.4 M and mostly inherent: `_untied` 0.53 (the 140 tied instructions build their copies), `finish` 0.35 (`body_like` clones 7 bodies), `_reused` 0.39 (its index for 679 candidate blocks), `three_address`, `try_instruction_commute`, the two reschedules and `scan_uses` 0.12-0.15 each. Re-profile at the tip on the exit date; a callee that still shows over 0.3 M gets a PR, the rest is recorded as inherent.

## m16 `add` to `lea` (textfill; owner w-1 with w-2, plan 2026-11-06)
`llrm-cc -m16 -O2 -march=i386 -S bench/textfill/textfill.c` loops with `add ax,2; cmp bx,3997; mov bx,ax; jb`. The pin at m32 (and BC at m16, with a changed constant) has no extra `mov`: `cmp eax,3997; lea eax,[eax+2]; jb`.

Cause: `three_address` returns None for anything but dwords (`into.width != 4`), so the word add stays two-address and its result needs the copy. `lea ax,[bx+2]` is flag-free in 16-bit addressing, but only `bx`, `si`, `di`, `bp` can be the base, so the conversion needs a register-class constraint on the source before allocation.

Plan: (1) fail-first test on the m16 listing: no `mov` between `cmp` and `jb` in the textfill loop; (2) word `add`/`sub` with an immediate converts when the target's 16-bit address form takes the source's class, with the constraint recorded on the new `lea`; (3) w-2: row 65 keeps the `cmp` before the `lea`.

## TA-R1 re-profile (2026-10-25, symbolized dist build at the tip, d_faces -O2)
`TwoAddress` is 6.43 M Ir inclusive (9.14 M on 10-24), against the pin's `TwoAddressInstructionImpl::run` 1.20 M plus its `LiveVariables::analyze` 6.0 M: 0.89x of the pin. No function in `twoaddr.rs`, `twoaddress.rs` or `livevariables.rs` is over 0.11 M exclusive (`from_dataflow` 0.106 M, `tied` 0.094 M, `_untied` 0.045 M); the rest is in shared callees the fat-LTO build folds together, so the per-callee split is not readable. Closed: the exit condition (a callee over 0.3 M gets a PR) finds none; LV3's dataflow stays the one DEVIATE (exit 2026-12-11).

What each side counts: ours is `TwoAddress::transform_raising` inclusive, which holds `tied` and so `BodyFacts::of` (the only caller of `LiveVariables::from_dataflow`, `twoaddress.rs`) and every other call `tied` makes: `twoaddress.rs`, `twoaddr.rs`, `liveness/livevariables.rs`, and the shared callees they call. The pin side is `TwoAddressInstructionImpl::run` 1.20 M plus `LiveVariables::analyze` 6.0 M, its separate pass, read from the 10-24 profile and not re-measured here. `LiveIntervals` and `SlotIndexes` are in neither number (ours: the coalescer's; the pin: its own passes). Both sides are therefore TwoAddress plus the liveness it asks for. The call structure is read from the source, not from the LTO profile, which folds callees.

