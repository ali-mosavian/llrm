# Tools

Run the Python ones with `uv run --project tools python tools/<dir>/<tool>.py`.

| Tool | Does |
|---|---|
| `nib-build.sh` | builds a Nib program into a DOS `.EXE` |
| `mir-corpus.sh` | regenerates `crates/opt/llrm-analysis/corpus`, the rich MIR the corpus tests read |
| `baseline.sh OUT` | every stage dump of the OMF corpus and Nib examples; `diff -r` two of them |
| `e2e/e2e.py` | BC compiles `tests/suite`, `llrm-omf` rewrites, LINK links, DOSBox runs, output compared |
| `e2e/matrix.py` | `e2e` over all twelve BC configurations |
| `e2e/fuzzgen.py`, `e2e/fuzzcheck.py` | random BASIC programs, judged by an evaluator, BC and `llrm-omf` |
| `e2e/mkgolden.py` | the suite's expected outputs, computed from what each program means |
| `e2e/mkfixtures.py` | rebuilds `tests/fixtures/omf` with the BC toolchains |
| `e2e/dosbox.py`, `e2e/cache.py`, `e2e/configs.py` | the DOSBox runner, its launch cache, the BC switch sets |
| `innerloops.py` | each innermost loop's instructions and memory operands, from an object's bytes (OMF or ELF, x86 or msp430) |
| `loops/run.py` | the loop corpus: cases in C, BASIC and Nib, checked by an oracle, llrm-mir and DOSBox, measured against hand-derived bounds and reference compilers; see below |
| `analysis/qbfootprint.py` | linked code bytes per module from two LINK maps |
| `analysis/runtime_writes.py` | what the QuickBASIC runtime writes, read off a linked image |

## The loop corpus

`loops/spec.py` is the one language a case is written in. `loops/oracle.py`
computes what it reports, in each language's semantics: C's signed overflow and
BASIC's error 6 make an input invalid there, Nib wraps. The emitters
(`emit_c.py`, `emit_bas.py`, `emit_nib.py`) write each case with a driver that
fills its arrays, calls it once per valid input and reports the result and a
digest of every array it writes. `loops/cases/` holds the families:

- `classic`: named anchors (dot, saxpy, memcpy, ... binary search).
- `concurrent`: 1 to 12 arrays in one loop, by element sizes, index forms,
  bases, source forms, trips, steps, starts and whole-segment walks, with
  metamorphic variants composed from a recorded seed, each judged against its
  base.
- `cross`: every pair of values of every two dimensions of one base loop.
- `fuzz`: loops drawn from all of it at once; `--seed` replays a draw.
- `adversarial`: the pressure ladder, pointer chasing, counter wrap, aliasing
  arguments and a SINGLE counter.

`loops/expect.py` derives each inner loop's bound from the spec and the target's
tables (registers, address-form partners, segments); `loops/quality.py` reads a
loop's induction variables, invariant loads and exit shape from its bytes. The
references are Open Watcom (`OW_BIN`, its `bwcc`), gcc-ia16's cc1 and as
(`IA16_ROOT`; the 286 is its newest CPU) and LLVM 20 for msp430 (`LLVM20`), a
mechanism check only.

`toolchain/` holds what `build.rs` bootstraps: Open Watcom's `wccq`, jwasm,
jwlink and DOSBox-X.
