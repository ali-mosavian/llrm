# AM-BT flip table, 22 rows by dag/old (w12, 2026-10-25)

Method: tip `b5c3edde3` (feat/parity + #4754 fixes), `llrm-c` md5 `d8d0825aef93`, m32 -O2 -march=i486 -mabi=sysv. One binary: old = `--isel=old`, dag = `--isel=dag`.
dag/pin = dag clocks / clocks of the pin's `llc -O2 -disable-lsr` on dag's own pre-isel IR (`30-switchconv.ll`, via pin.py's m32 target), linked and run by the same harness (vsgcc 486 price table, all runs `ok`).
dag as ELF = dag as OMF on all 22 (object format does not move clocks). The script and raw results are not committed.
Verdict: PIN-FAITHFUL = dag/pin <= 1.03 (faster than the pin counts); DEFECT otherwise. Static gaps of faithful rows are numbered rows of their own (w13's lists).

| # | prog | dag/old | dag/pin | verdict | cause | owner |
|---|---|---|---|---|---|---|
| 1 | x_iir | 2.38 | 1.011 | PIN-FAITHFUL | `imul` by constant (old used shift/lea; pin keeps `imul`); tuning row after parity | w12 |
| 2 | x_switch_ranges | 1.83 | 0.988 | PIN-FAITHFUL | old beat the pin | - |
| 3 | x_fir | 1.83 | 0.961 | PIN-FAITHFUL | `imul` by constant as row 1 | w12 |
| 4 | x_hash | 1.75 | 1.005 | PIN-FAITHFUL | old beat the pin; N call-argument lowering (static) | w12 |
| 5 | x_switch_dense | 1.61 | 1.000 | PIN-FAITHFUL | old beat the pin | - |
| 6 | switch | 1.36 | 0.996 | PIN-FAITHFUL | old beat the pin | - |
| 7 | x_binsearch | 1.26 | 1.022 | PIN-FAITHFUL | C entry-argument compare (static) | w13 (#4745) |
| 8 | x_isqrt | 1.18 | 1.009 | PIN-FAITHFUL | C (static) | w13 (#4745) |
| 9 | x_heapsort | 1.15 | 1.001 | PIN-FAITHFUL | H combine (static) | w2 |
| 10 | x_list | 1.13 | 1.125 | DEFECT | A constant before the first compare | w10 |
| 11 | x_crc32 | 1.11 | 1.000 | PIN-FAITHFUL | CRC loop idiom missing (loopidiom D9, IR level); the pin on our IR is the same, old was faster | loopidiom owner |
| 12 | x_dct | 1.07 | 1.052 | DEFECT | C | w13 (#4745) |
| 13 | x_cordic | 1.05 | 1.048 | DEFECT | C | w13 (#4745) |
| 14 | x_popcount | 1.05 | 0.920 | PIN-FAITHFUL | faster than the pin; I copy coalescing (static) | w13 |
| 15 | x_bubble | 1.04 | 0.997 | PIN-FAITHFUL | A constant sharing (static) | w10 |
| 16 | x_funcptr | 1.42 | 1.306 | DEFECT | accumulator copied to three homes at the loop header (unverified: coalescer); was 1.03 vs old in the 12:33 table: a regression since | w2 |
| 17 | bintree | 0.93 | 1.002 | PIN-FAITHFUL | C static gap (104 vs 89) | w13 |
| 18 | x_nbody_f | 1.02 | 0.984 | PIN-FAITHFUL | below the pin; +22 static, x87 | - |
| 19 | nbody | 1.01 | 0.852 | PIN-FAITHFUL | below the pin | - |
| 20 | matmul | 0.97 | 1.000 | PIN-FAITHFUL | +10 movs static (D, unproven) | w12 |
| 21 | x_hanoi2 | 0.99 | 1.231 | DEFECT | pass order: pin runs machine-cp before shrink-wrap/post-RA sink; #4745 sinks the arg loads | w12 / w2 |
| 22 | x_life | 1.06 | 0.844 | PIN-FAITHFUL | faster than the pin; static gap (165 vs 137): preheader zero order, one `idiv` the pin lacks | w10 / w12 |

Reading: 17 of 22 at or below the pin on clocks. DEFECT: x_list, x_dct, x_cordic, x_funcptr, x_hanoi2. The PIN-FAITHFUL x_iir..x_switch rows are the old selector beating the pin, not a dag loss.
x_funcptr: dag 35282 clocks at this tip against 24772 old; the 12:33 table had dag/old 1.03. Something in between regressed it.
