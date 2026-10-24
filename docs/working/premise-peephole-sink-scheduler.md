# Premise tests: Peephole, MachineSink, Machine Scheduler

TL;DR: by bytes only Peephole is worth something on the rows corpus (m32 i486); by clocks MachineSink is worth +1.0..1.6% (+25..35% on x_collatz) and ours lacks that piece, and the Machine Scheduler is worth +0.13..0.19% (DEVIATE MS1 on clocks).

Instrument: `tools/pairs/premise.py OURS_LLRM_C LEVEL PASSFLAG` (a copy of w-4's `leacmp.py` shape). Our pipeline's last MIR (`*-switchconv.ll`) of each of the 71 programs of the rows corpus goes to the pin's llc (`pin.py` target m32: `i486-unknown-linux-gnu`, cpu i486, no SSE, optsize at -Os) with and without one pass. It counts instructions per program, and `.text` bytes of the pin's object. Ours is our own `-S` listing of the same source. Run on dist `4d72d84065` (own worktree, stamped).

| pass off in the pin | level | ours | pin with | pin without | pin's value (instr / bytes) |
|---|---|--:|--:|--:|---|
| `--disable-peephole` | -Os | 4899 | 5048 | 5302 | +254 / +554 B |
| `--disable-peephole` | -O2 | 9228 | 8414 | 8742 | +328 / +786 B |
| `--disable-machine-sink` | -Os | 4899 | 5048 | 5060 | +12 / +24 B |
| `--disable-machine-sink` | -O2 | 9228 | 8414 | 8404 | -10 / -16 B |
| `-enable-misched=false` | -Os | 4899 | 5048 | 5041 | -7 / -29 B |
| `-enable-misched=false` | -O2 | 9228 | 8414 | 8430 | +16 / +96 B |

**Correction (the first version of this note counted our listing's directive lines as instructions, `public f`, `extern g`, `f endp`, `align 16`: ours was 5241 / 9800, not 4899 / 9228).** Corrected, ours is 3.0% below the pin with all passes at -Os (4899 against 5048) and 9.7% above it at -O2 (9228 against 8414); the "3.8% / 16.5% above" that went into the queue was that miscount.

By mnemonic (`tools/pairs/premise_classes.py`, 71 programs, ours / pin with / pin without Peephole), -O2: `cmp` 346 / 325 / **8** and `sub` 144 / 275 / **598**: without its Peephole the pin selects `sub` where a `cmp` is meant (the pass turns a `sub` whose result is dead into `cmp`), and ours selects `cmp` directly, so that part of the pin's value (323 instructions) is not a gap of ours. `mov` 2328 / 2225 / 2535: the pass removes 310 copies in the pin and ours is 103 above the pin with it. `lea` 888 / 419 / 416 (+469) against `imul` 84 / 182 (-98) and `shl` +77: a difference of lowering (ours decomposes constant multiplies, which the i486 price asks for), not of the pass. At -Os the large classes are `sub` 69 / 202 / 425, `pop` 381 / 244 / 249 (+137 callee-save pops), `pushd` 57 / 0 (+57).

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
- **Peephole**: its value in the pin is mostly the `sub`-to-`cmp` conversion (323 of the -O2 instructions), which ours does not need; what is left is the copy side (`mov`: the pin's pass removes 310, ours is +103 above the pin with it). A translation PR needs a function where ours keeps a copy the pin's `optimizeCoalescableCopy`/`foldImmediate`/copy rewriters remove; the next step is that per-function `mov` diff (not an instruction-count gap); plan note 11-02 stands and the PR date 11-06 is open until a piece is shown missing.

## Not measured

P5, m16, and the pin's sub-features inside Peephole (`--disable-peephole` is the whole pass). Per-function spill/reload of ours: its listing does not mark spill slots, so ours is compared in clocks and instructions only.

## Peephole: per-function `mov` (2026-10-23, `tools/pairs/premise_mov.py`, -O2, m32 i486, 71 programs, 158 functions matched)

Ours is 72 movs above the pin over all functions (39 above, 98 below, 21 equal). The pin's pass removes 312 movs
(without minus with). The largest gaps are `recchop:chop` +38, `matmul:bench_matmul` +37, `x_mergesort:msort` +33, `quicksort:sort` +28, `x_radix` +24, `nbody` +23; their `mov` counts include spill and reload moves, so a gap there is allocation first. No function shows a copy the pass's rewriters would remove and ours keeps; the next check is the listing of `chop` and `msort` against the pin's (copy or spill).

## Scheduler: the pin's own clocks, per row (2026-10-25, pinned clang, m32 i486, 71 programs, 486 clock model; `pinclocks.sh`-style run with and without `-mllvm -enable-misched=false`)

-O2: clocks 40,423,086 with the scheduler, 40,430,356 without (+0.02%), code 25,674 / 25,690 B; 14 of 71 rows change. -O3: 40,361,309 / 40,282,931 (-0.19%: off is better), code 30,874 / 30,922 B; 17 rows change. The largest effects: sieve +4.9% without it, x_list +2.4% (-O2) and +7.1% (-O3), matmul +3.6% (-O3); against x_dct -1.7%, fpbench -1.9% (without it better). So the pin's pre-RA scheduler is worth about nothing on this corpus in total clocks; a handful of rows gain 2..7% and a handful lose as much. Consequence for row 65: our nearly inert pass (the region count, #4391) is not far from the pin's value here, and a latency-driven `limit/2` policy is a per-row gamble the pin's own numbers do not support. The reason to keep the pass is register pressure (its `tryCandidate` order), and it earns that only where a region exceeds the limit. DEVIATE MS3 (regions at or under `limit / 2` keep source order) stays, priced here: the pin still orders those by latency, which is worth the rows above and nothing in total.

