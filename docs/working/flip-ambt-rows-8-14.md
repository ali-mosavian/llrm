# flip-ambt rows 8-14

Method: tip `c76f9a6a5`, `llrm-c` md5 `8cc269d828497b719ed84ba27bcb5350` (dist build by `tools/pairs/pair_build.sh`), `-m32 -mabi=sysv -march=i486 -O2`, pin `llc -O2 -disable-lsr` with `PIN_TRIPLE=i486-unknown-none-elf` on the dag path's pre-isel IR (`30-switchconv.ll`), one harness and price table; script `ambt/run.py` (w12) pointed at this tree. Owner w11 (sdag-c), 2026-10-26.

Clocks (instructions, code bytes); all rows `ok`:

| program | old | dag | pin llc | dag/old clocks |
|---|---|---|---|---|
| x_isqrt | 411263 (281816, 219) | 483668 (282066, 171) | 479147 (292700, 187) | 1.176 |
| x_heapsort | 42318 (35787, 459) | 44415 (31486, 443) | 44351 (28987, 475) | 1.050 |
| x_dct | 115166 (105615, 920) | 120798 (102032, 920) | 117726 (98959, 904) | 1.049 |
| x_cordic | 54246 (38425, 187) | 57046 (37826, 171) | 54445 (35424, 155) | 1.052 |
| x_popcount | 309886 (223916, 251) | 323886 (187916, 187) | 351861 (217891, 187) | 1.045 |
| x_list | 59099 (26105, 171) | 59299 (25905, 155) | 59302 (25907, 155) | 1.003 |
| x_crc32 | 48797 (32383, 283) | 54198 (35582, 267) | 54200 (35583, 267) | 1.111 |

The dag object in ELF and in OMF has identical clocks on every row (same code, two containers).
