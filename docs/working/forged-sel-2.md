# FORGED-SEL-2: what a forged selector reaches

Owner: funcattrs-port. Written 2026-10-22; due 10-26. Evidence: `tools/pairs` builds of 188fd3905f^ and 188fd3905f, `cargo test -p llrm-qb --lib -- test_a_poke test_a_def_seg_known_only`.

## The rule now

`ForgedSelectors` (forging.rs): an `inttoptr` into the target's selector space from an integer that is not a plain number (VARSEG, arithmetic, a PEEK result, a loaded `b$seg`) may name the data segment, so GlobalsAA tracks no global of the module (188fd3905f, #3450). A numeric selector (`&HB800`) is exempt. The rule is sound and coarse: it treats every non-numeric selector as able to reach any DGROUP byte.

## What it costs

Four tests go red exactly at 188fd3905f and are green at its parent; a fifth (`test_a_def_seg_known_only_to_promotion_is_not_rebuilt_per_poke`) is red before it and is a separate row:
- `test_a_poke_takes_its_segment_from_the_def_seg_before_it` ([true] where [false] was expected), `test_a_poke_after_a_call_that_never_runs_def_seg_keeps_its_segment` ([false,true,true] vs [false,false,false]), `test_a_poke_to_video_memory_leaves_invariant_globals_hoisted`, `test_a_poke_through_a_counted_selector_leaves_invariant_globals_hoisted`.
All four are the conservative form, not wrong code: a POKE reloads `b$seg` (its selector is a loaded word) and loop-invariant globals stay in memory. Their old comment gives the reason they held: the runtime never hands out `b$seg`'s address.
Cost, `ROW_LOOP`'s inner loop (`DEF SEG = &HA000 + yp`, `POKE x, cd(dn)`, m16 -O2, `llrm-qb` at the two commits): 20 -> 22 instructions per trip, 9 -> 9 memory operands (the loop gives up the hoisted `yy`/`xp` sums and reloads `b$seg`, and gains fewer spills). A POKE under a constant selector (`DEF SEG = &HB800`): identical listings, no cost.

## Exits (each a sound fact, none a pattern)

1. Runtime-owned cells are not reachable by a forged selector. The runtime's cells (`b$seg`, the module cells named by `!llrm.named`) are never given to the program; writing one through a computed selector corrupts the runtime, which the language contract excludes (the old tests' comment: the runtime never hands the address out). GlobalsAA keeps tracking them in a forged module. Soundness: the contract, stated once in the runtime description (`!llrm.named` is the existing trust boundary: a global named there is outside a source program's reach). Violation: a program that scans memory (far6) and happens to hit a runtime cell: undefined in BC too.
2. A selector whose value range lies at or above the end of conventional memory cannot name DS. DOS loads the program below 0xA000 (real mode, 640 KB); `DEF SEG = &HA000 + yp` with yp in [0, 3980] is in [0xA000, 0xAF8C]. A target-description field (`conventional_end = 0xA000`) and the selector's value range (ValueTracking's ranges; known bits) are all the proof needs; it clears ROW_LOOP and every video-memory POKE with a computed offset, not the numeric ones (already exempt).
3. A descriptor-based far pointer reaches only its array (the bounds rule). `inttoptr(load of an array descriptor's segment cell)` + an offset that stays inside `[0, extent)` addresses the array's bytes: a dynamic array has a segment of its own, a static one sits at layout offsets disjoint from the scalars. Needs a language contract in the frontend (the descriptor load is tagged with its array, `!llrm.array`), and states that out-of-bounds indexing without /D is undefined. The bintree and jpeg loops need this (the far store to an element must not make the counter reload).

Order: 1 first (smallest, restores the four tests), 2 next (a target fact plus ranges), 3 last (a frontend contract: the most general, the most to prove).

## Tests that exit each

1: the four tests above, as they are (green again), plus a test that a forged selector (`DEF SEG = VARSEG(x)`, far3/far6) still untracks a program global. 2: ROW_LOOP green, with a selector range below 0xA000 still forged. 3: a bintree loop test (the counter is not reloaded after an array element store) and an out-of-range selector test.
