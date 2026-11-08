# m16 open rows vs the best external (2026-11-08)

Source: ~/scratch/handoff/matrix/x86-m16-O1-tip-2026-11-08.md (O1, 262 cells, 7 worse) and the O2 level of 25 programs (1 worse). O1 and Os are strict on bytes and clocks; O2 and O3 price 1 clock = 2 bytes.

| Row | Gap | Cause | Owner | Exit |
|---|---|---|---|---|
| M16-RECMANY-O1 | recmany c O1 242 B vs bcc 191 (clocks 232k vs 329k, won) | three const-modulo magic expansions (~66 B) and three spill slots; bcc uses inline `idiv ebx`; the pin also expands by magic at O1; keeping `idiv` costs about +60% clocks | m16 | 2026-11-24 |
| M16-SIEVE-RET | sieve c O1 +6 B, Os +2, O2 +1 | `remove_copy_by_commuting_def` needs the xor's second source to equal the copy destination; the pin has the same shape | m16 | 2026-11-20 |
| M16-HUGE-C-O1 | huge c O1 223 B vs bcc 219 (+4) | bcc's 219 B is call stubs; its map links F_LXMUL and H_PADD (>= 73 B of library) that ours inlines; bcc plus helpers >= 292 B. Library bytes are a separate column | m16 | 2026-11-24 |
| M16-HUGE-BQ16-O1 | huge bq16 O1 358 B vs bc 354..356 (+2..+4) | about 40 B of spill code around the inner loops (five slots stored at entry, reloaded at exit) | greedy-subreg | 2026-11-24 |
| M16-GREP-LSR | grep c O1 clocks +24.7%, bytes +2.5%; O2 clocks +18.6% | count-to-zero induction variable keeps the end pointer live; state and position spill every trip | w12 (LSR) | open |
| M16-RECCHOP-O1 | recchop c O1 184 B vs ow 179 (+5) | LSR-made invariants live across the recursive calls | w12 (LSR) | open |
