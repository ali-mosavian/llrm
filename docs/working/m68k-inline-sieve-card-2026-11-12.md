# m68k sieve -O2: why the inline trial cannot see its loss (2026-11-12)

TL;DR: no static price separates sieve (+2.3% clk, +100 B) from recsum (-12.1% clk); the pin's LSR
makes the same induction variables, and gcc's IVOPTS scaling has no discount for a rare use.

- Cell: `bench/sieve` m68k-68020 O2, 154 -> 254 B, 79,803 -> 81,623 clk (qemu, `cycles --exe`), with
  the SZ2 trial inlining `bench_sieve` into `main` (PR "SZ2 trial weighs clocks by loop trips").
- Cause (asm): with the trip count constant, LSR makes `i*i` two induction variables of the outer
  loop (`add.w`, `addq.l`, 1022 trips) for a use that runs 11 times (`i <= 31`, `i` prime).
- Pin: `llc -mcpu=M68020 -O2` on the pin's own IR with `bench_sieve` inlined makes the same
  per-iteration IVs (phis 5, 4 in `main`). Ours is faithful. The pin's default clang -O2
  IR already has `bench_sieve` inlined into `main` (0 calls; the first reading said the opposite
  because that run passed `-fno-inline-functions`). Only gcc keeps the call (172 B for
  `bench_sieve` alone) and wins.
- gcc IVOPTS: `get_scaled_computation_cost_at` / `determine_scaling_factor` scale a use's cost
  UP by its block's frequency over the loop header's (bound 20), only for blocks hotter than the
  header. A rarely used block keeps factor 1: no discount to translate.
- Put-back trial (WIP fe59237a34, not pushed): the accepted site compiled again refused, each group
  priced on lowered code (`trial.rs price_of`: loop depth x10, branch shares halve). Reads sieve
  inlined 254 B / 1080 clk vs called 154 B / 380 clk, recsum inlined 146 B / 940 clk vs called
  94 B / 578 clk. Measured: +2.3% and -12.1%. Wrong in magnitude on both and in direction on
  recsum: the gate would put recsum back. Instrument failure, not a finding.
- Why no static weight works: the loss is data-dependent (the inner loop runs under 11 of 1022 outer
  trips); SCEV trips, BPI and lowered shares all call it 1/4 or 1/8.
- Decision: keep row M68K-INLINE-SIEVE (exit 2026-11-24). A rare-use discount in LSR would be a
  deviation from both the pin and gcc, shared by every target, for one cell.
- Margin tried and rejected: requiring the trial's saved clocks to cover 200% of the copy's byte price
  closed sieve and moved no other cell (2088-cell A/B), but 200% was picked by that one cell and 300%
  lost sfm16_dbg (-14 B -> +60 B). A one-cell knob, not a rule; dropped. It returns only as a
  calibration: the price error of the lowered price against executed qemu clocks over the corpus.
