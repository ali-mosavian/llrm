# z80 long shift by a constant: inline vs routine, 2026-11-10

llc 08ec83d6c88a (0db87355f6), `-S -O2` on `shl/lshr/ashr i32 %x, N`, sdasz80 listing bytes and T (straight line, exact). Routine call: `ld a,#N; call` 5 B, 24 + 59 + 40N T (shl; 45N for lshr/ashr), routine 15 B once.

| N | shl B/T | lshr B/T | ashr B/T |
|---|---|---|---|
| 1 | 10/60 | 22/93 | 22/93 |
| 3 | 33/158 | 40/165 | 40/165 |
| 5 | 43/212 | 56/229 | 56/229 |
| 9 | 43/177 | 24/150 | 46/190 |
| 15 | 23/95 | 23/95 | 22/93 |
| 16 | 4/20 | 4/20 | 6/30 |

call_keeps -O1 executed (sim): before #8911 1473 B/64413 T; after 1740 B/55322 T; heavy shifts as calls (env `LLRM_Z80_FS`) 1588 B/59636 T. Rates: 28 T/B for the shifts, 41 T/B for the rest, 34 T/B overall.
