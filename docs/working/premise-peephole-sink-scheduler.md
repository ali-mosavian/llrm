# Premise tests: Peephole, MachineSink, Machine Scheduler

TL;DR: by bytes only Peephole is worth something on the rows corpus (m32 i486); by clocks MachineSink is worth +1.0..1.6% (+25..35% on x_collatz) and ours lacks that piece, and the Machine Scheduler is worth +0.13..0.19% (DEVIATE MS1 on clocks).

Instrument: `tools/pairs/premise.py OURS_LLRM_C LEVEL PASSFLAG` (a copy of w-4's `leacmp.py` shape). Our pipeline's last MIR (`*-switchconv.ll`) of each of the 71 programs of the rows corpus goes to the pin's llc (`pin.py` target m32: `i486-unknown-linux-gnu`, cpu i486, no SSE, optsize at -Os) with and without one pass. It counts instructions per program, and `.text` bytes of the pin's object. Ours is our own `-S` listing of the same source. Run on dist `4d72d84065` (own worktree, stamped).

| pass off in the pin | level | ours | pin with | pin without | pin's value (instr / bytes) |
|---|---|--:|--:|--:|---|
| `--disable-peephole` | -Os | 5241 | 5048 | 5302 | +254 / +554 B |
| `--disable-peephole` | -O2 | 9800 | 8414 | 8742 | +328 / +786 B |
| `--disable-machine-sink` | -Os | 5241 | 5048 | 5060 | +12 / +24 B |
| `--disable-machine-sink` | -O2 | 9800 | 8414 | 8404 | -10 / -16 B |
| `-enable-misched=false` | -Os | 5241 | 5048 | 5041 | -7 / -29 B |
| `-enable-misched=false` | -O2 | 9800 | 8414 | 8430 | +16 / +96 B |

Ours is above the pin with every pass by 193 instructions at -Os (3.8%) and 1386 at -O2 (16.5%): that is the whole gap, not any one pass's. Largest per-program gaps: x_ll_div 24, lru 21, x_mergesort 19 (since #3654 fixed), x_transpose 19 at -Os; recpow 184, matmul 111, quicksort 74, x_fir 70, x_switch 70 at -O2.

## Clocks, spills and copies (the corrected instrument)

Bytes do not price a scheduler or a sink. `tools/pairs/pinclocks.sh LABEL FLAGS` builds the 71 rows programs with the pinned clang at m32 i486 `-O2`/`-O3` with the flag, and runs them in the 486 clock model (`vsgcc/harness.py`); it counts the static `Spill`/`Reload` lines and register copies of the pin's `-S`. Check of the instrument: the pinned clang's default clocks are the system clang 20's to 0.04% (geomean 1.0004, 71 programs, both levels). All runs `ok`, 71 of 71.

| pin, against the pin's default | level | clocks geomean | instr | mem | spill+reload | reg copies | worst |
|---|---|--:|--:|--:|--:|--:|---|
| `-mllvm -enable-misched=false` | -O2 | 1.0013 | 1.0037 | 1.0073 | -6 | +7 | sieve +4.9%, x_list +2.4% |
| `-mllvm -enable-misched=false` | -O3 | 1.0019 | 1.0040 | 1.0077 | 0 | +16 | x_list +7.1%, sieve +4.9%, matmul +3.6% |
| `-mllvm -disable-machine-sink` | -O2 | 1.0098 | 1.0132 | 1.0060 | 0 | +14 | x_collatz +25.4%, x_hanoi2 +22.2%, x_insertion +9.5% |
| `-mllvm -disable-machine-sink` | -O3 | 1.0155 | 1.0205 | 1.0269 | +18 | +63 | x_collatz +35.2%, x_cordic +31.1%, x_hanoi2 +22.2% |

Ours against the pin on the programs the sink moves (clocks, our build 49a8d96bfb): x_collatz -O2 179697 against the pin's 147521 (+21.8%) and 185034 without its sink: ours stands where the pin stands with no sink. x_hanoi2 170292, x_cordic -O3 34442, x_insertion 56649 are below the pin's with the sink (221263, 45252, 60400): ours is ahead there through other passes.

## Decisions

- **MachineSink**: not zero. The pin's pass is worth +1.0% clocks (-O2) and +1.6% (-O3) as a geomean and up to +25..35% on x_collatz; ours is at the pin's no-sink clocks on x_collatz, so the sink is missing a step there. The x_collatz diagnosis (the constant copy on a guard edge that reaches a shared exit should sink into the shared block; the `emptyblocks` work, #3350) is a hypothesis for which step, not shown here. Translate the missing step into machinesink.rs; the byte reading above (+12 / +24 B) was the wrong price.
- **Machine Scheduler**: on i486 the pin's pass is worth +0.13% clocks (-O2), +0.19% (-O3), spills unchanged (0 / -6), static copies +7 / +16; one program stands out (x_list +2.4% / +7.1%, where ours is +9.7% / +5.3% above the pin). DEVIATE MS1 stands now on clocks, not on bytes: no scheduler; `pressuresink.rs` stands in; owner llrm-w-2; review 2026-11-20, or when a P5 model is measured (this is i486 only).
- **Peephole**: the pin's value is real in bytes and instructions (table above); its clocks are not measured yet. Before a translation PR, classify the gap per mnemonic class on the programs named above to find which of PeepholeOptimizer.cpp's pieces (`optimizeCmpInstr`, `optimizeExtInstr`, `optimizeSelect`, `optimizeCondBranch`, `foldImmediate`, the copy rewriters) ours lacks; plan note and first code PR dates 11-02 / 11-06 stand.

## Not measured

P5, m16, and the pin's sub-features inside Peephole (`--disable-peephole` is the whole pass). Per-function spill/reload of ours: its listing does not mark spill slots, so ours is compared in clocks and instructions only.
