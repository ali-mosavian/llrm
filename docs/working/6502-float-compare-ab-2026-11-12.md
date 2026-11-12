# 6502 hand-written float compares, A/B (2026-11-12)

Compiler dist-K for both sides; the library tree is the only change (`RT_DIR`).

| cell | -O2 bytes | -O2 clocks |
|---|---|---|
| fcmp_all, C compares | 36604 | 967900 |
| fcmp_all, `cmp64.s` + `cmp32.s` | 30796 | 769950 |

- The 79-cell table (O1, O2, Os) is unchanged: 0 cells differ, no table program compares floats.
- Checks: `run_cmp64.sh` (edge n=1521, random n=50000) and `run_cmp32.sh` (edge n=961, random n=50000): 0 fails; a NaN-result variant fails 476 edge records.
