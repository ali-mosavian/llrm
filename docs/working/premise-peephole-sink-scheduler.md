# Premise tests: Peephole, MachineSink, Machine Scheduler

TL;DR: only the pin's Peephole is worth something on the rows corpus (m32 i486); MachineSink and the Machine Scheduler are worth about nothing there, so neither gets a translation PR, each is a numbered DEVIATE.

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

## Decisions

- **MachineSink**: the pin's pass is worth 12 instructions (0.2%) at -Os and nothing at -O2 here. Ours is a translation already (machinesink.rs; M1 to M6, a88a614969 outside size builds, 7a574ed3ed). No PR on the pin's value. The `+2.5 KB` of the codegen ablation (#3628) is a 77-IR m16 figure; this test is m32, so m16 is open: the pin's m16 is code16 with 32-bit registers, and the same test there is the next step if oversight wants it.
- **Machine Scheduler**: absent in ours (`pressuresink.rs` is a stand-in). On i486 it is worth under 0.5% either way (its 55 changed IRs in #3628 are reorderings: net -7 / +16 instructions). DEVIATE (no number yet, MS1): no scheduler; what stands in for it is pressuresink; owner llrm-w-2; review 2026-11-20, or when a P5 model is measured (this test is i486 only; P5 is not).
- **Peephole**: a real value (+254 / +554 B at -Os). Before a translation PR, classify the gap per mnemonic class on the programs named above, to find which of PeepholeOptimizer.cpp's pieces (`optimizeCmpInstr`, `optimizeExtInstr`, `optimizeSelect`, `optimizeCondBranch`, `foldImmediate`, the copy rewriters) ours lacks; plan note and first code PR dates 11-02 / 11-06 stand.

## Not measured

P5, m16, and the pin's sub-feature switches inside Peephole (`--disable-peephole` is the whole pass); the per-program gap above is ours-minus-pin on the same MIR and includes everything between MIR and bytes.
