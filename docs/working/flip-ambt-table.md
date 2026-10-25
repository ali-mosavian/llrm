# AM-BT flip table, rows 1-15 by dag/old (w12, 2026-10-25)

Clocks: vsgcc 486 price table, m32 -O2 -march=i486, one binary, `--isel=old|dag`. Source: `~/scratch/coordwork/amtable-2026-10-25.md` (parity 3c36dac99 + #4668, all 30 ok).
**dag/pin here is dag / pinned clang -O2** (same clock model), not `llc` on our IR; not yet re-run on the final tip. Causes are the first-divergence letters of `first-divergence-m32.md` (A..O). Rows 16+ : w13, w10.

| # | prog | dag/old | dag/pin | verdict | cause | owner |
|---|---|---|---|---|---|---|
| 1 | x_iir | 2.38 | 1.01 | PIN-FAITHFUL | `imul` by constant (old chose shift/lea; the pin keeps `imul`, 13 clk on i486); B prologue; tuning row after parity | w12 |
| 2 | x_switch_ranges | 1.83 | 0.99 | PIN-FAITHFUL | old beat the pin (0.54); B saved-register set | w13 (CSR #4729) |
| 3 | x_fir | 1.83 | 0.68 | PIN-FAITHFUL | `imul` by constant as row 1; O loop structure; dag is faster than the pin | w12 |
| 4 | x_hash | 1.75 | 1.01 | PIN-FAITHFUL | old beat the pin (0.57); N call-argument lowering (size only) | w12 |
| 5 | x_switch_dense | 1.61 | 1.00 | PIN-FAITHFUL | old beat the pin (0.62); B saved-register set | w13 (CSR #4729) |
| 6 | switch | 1.36 | 0.99 | PIN-FAITHFUL | old beat the pin (0.73); B | w13 (CSR #4729) |
| 7 | x_binsearch | 1.26 | 1.02 | PIN-FAITHFUL | old beat the pin (0.81); C entry-argument compare | w12 (#4745) |
| 8 | x_isqrt | 1.18 | 1.01 | PIN-FAITHFUL | old beat the pin (0.86); C | w12 (#4745) |
| 9 | x_heapsort | 1.15 | 0.96 | PIN-FAITHFUL | H combine (`sar;shr` for `/2`); dag faster than the pin | w2 |
| 10 | x_list | 1.13 | 1.12 | DEFECT | A constant before the first compare (`xor r,r`) | w10 |
| 11 | x_crc32 | 1.11 | 1.40 | DEFECT | C is the first divergence; 1.40 is larger than C explains (old 1.26 also off, code 1.89x): not traced | w13 |
| 12 | x_dct | 1.07 | 1.06 | DEFECT | C, row C #4745 re-measure pending | w12 / w13 |
| 13 | x_cordic | 1.05 | 1.05 | DEFECT | C | w12 (#4745) |
| 14 | x_popcount | 1.05 | 0.94 | PIN-FAITHFUL | I copy coalescing; dag faster than the pin | w13 |
| 15 | x_bubble | 1.04 | 1.00 | PIN-FAITHFUL | A constant sharing | w10 |

Reading: 11 of 15 sit at the pin (0.96-1.02) where old was below it. The old selector's clocks came from choices the pin does not make; they are not a dag regression. Open defects: rows 10-13.
