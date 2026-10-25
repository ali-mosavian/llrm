# Liveness reuse: one lane solve serves flags (2026-10-25)

TL;DR: built and dropped (2026-10-25): 0.50 M on `d_faces`, and 27 float objects change through other lane consumers. See Result. Spike: `FlagLive` (about 6.8 M inclusive on the board) re-solves what `LaneLive` already holds: the flag lanes are lanes of it. On `d_faces` the flags read off the lane answer equal `FlagLive`'s in 1148 of 1156 blocks at -O2 and 1170 of 1170 at -Os, never narrower. The 8 others come from one rule that differs. Reuse is feasible; it needs that rule aligned and an objsweep.

## Two solves of one problem
`liveness::lanewise` (`LaneLive`) and `peephole::_flags_live_out` (`FlagLive`) are both backward `incremental::Kept` solves over the same blocks. `LaneLive`'s universe already holds the flag lanes (`_flag_lanes(0xFFFF_FFFF)`). `FlagLive` is the same fixed point with three differences:
- exits and calls/returns: `_exit_flags()` (arithmetic flags without AF), not the whole universe;
- only `RegId::None` lanes are kept, and only the arithmetic mask;
- an instruction whose effect cannot be decoded: `LaneLive` reads and writes everything; `_flag_effect` goes by the mnemonic (`_reads_flags`), so `fldcw`, `fnstcw`, `fstp` read no flag.

## Spike
Throwaway under `LLRM_SPIKE_FLAGS` in `_flags_live_out`: per block, flags live after it from `lanewise(body).entering`, masked to the arithmetic lanes minus AF, compared with `FlagLive`'s.

| `d_faces` (m32) | blocks equal | derived wider | derived narrower |
|---|---|---|---|
| -O1 | 1044 | 8 | 0 |
| -O2 | 1148 | 8 | 0 |
| -Os | 1170 | 0 | 0 |

Unmasked, 236 to 289 blocks were wider: the non-arithmetic flag lanes. Wider blocks are those on a path through an undecodable instruction (`NOEFFECT` list: `fldcw` 24, `fnstcw` 8, `fstp` 8 on -O2). Checked by printing the effect-less instructions of the body; the difference is the third rule above, not a bug in either solve. One corpus file only; the other QCport files were not in the checkout I ran.

## Asks per pass
Every consumer of a lane or flag answer, by the call it makes (grep, not yet read one by one):
- `dead_at_exit` (lanes dead after a block): `exactaddress`, `fixupbw`, `fixupleas`, `machinecp`, `machinedce`, `parcopy`, `peep/walk`, `peephole`, `regthrash`, `sharedstores`, `upperzero`.
- `live_into` (lanes live on entry, with successors): `flagscopy`, `loopslots`, `postrasink`, `phielimination`.
- `_flags_live_out`: `peephole` (4), `peep/walk`, `sharedstores` (2).
- `ZeroRoots` (a separate `Kept`): `upperzero`, `exactaddress`, `peep/walk`, `peephole`.

## Design
1. Make `LaneLive`'s undecodable-instruction rule flag-aware: reads and writes every register lane, flags by `_flag_effect`'s mnemonic rule. This narrows flag lanes only; register lanes unchanged.
2. `_flags_live_out` returns the lane answer's `entering` masked to `_exit_flags` lanes, with the exit rule moved into `LaneLive::entering` for flag lanes. `FlagLive` and `FLAGS` are deleted.
3. `LLRM_CHECK_LIVENESS` compares against the old from-scratch flags solve until the objsweep is clean, then against nothing.

Saved: the `FlagLive` solve (3.94 M self, `_flag_effect` 1.57 M from it, the rest components and map inserts). Not saved: the per-instruction `_flag_effect` the consumers call. Cost added: a mask per block, and more work in the lane solve per undecodable instruction (none measured).

## Data still needed
- The same table on all 260 QCport modules, not one file.
- Pair + `objsweep` on the change; a wider block can only keep a flag live, so objects may grow, not break.
- `ZeroRoots` and `LaneLive` share no lattice (zero upper halves); not a candidate.
- w-1 (R45-8 withdrawn: facts already held once per body) and w-13 (intervals) on whether anything else wants the lane answer.

Exit: if the pair shows an object change that is not a gain, keep `FlagLive` and record why.

## Result: built, measured, dropped
Built as designed (`72ff0b8df`, not merged): 260-module run asserting flags-from-lanes narrower = 0 over the arithmetic lanes held; tests passed.
- `d_faces` m32 -O2: 813.805 M to 813.309 M, -0.50 M. The 6.8 M above was `FlagLive` inclusive on an older whole-compile profile; the lane solve now pays `_flag_effect` per instruction.
- objsweep: 27 of 568 objects change, all float programs, net -20 B, none grow. `LaneLive` reading x87 and `fldcw` flags by mnemonic changes what the other lane consumers (`machinedce` and the rest) see; the change cannot be separated from the flags'.
- Not worth 0.5 M. Reopen only if a lane consumer wants the mnemonic rule for its own sake.
