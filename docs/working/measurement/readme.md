# Measuring

For contributors: which instrument answers which question. Quoting one for another is how the project acquired figures it later had to withdraw.

## Does it run right: `tests/`

| Folder | Judged by | Run by |
|---|---|---|
| `tests/check/` | CHECK lines against a tool's text output (lit and FileCheck style) | `cargo test --test check` |
| `tests/run/`, `examples/`, `bench/` | the program's stdout against its `.out`, built by llrm and run in DOSBox | `cargo test --test run` |
| `tests/differential/` | another compiler: BC 4.5 against llrm-qb, and the Microsoft BASIC conformance suites on their own runtimes | `cargo test --test differential` |

A `.out` is derived from what the program means, never from a compiler's output alone. See [testing](../testing.md).

## How much work: `tools/bench`

Executed instructions and memory operands of each benchmark's kernel, per language and level, counted in a real-mode emulator. Deterministic: no clock, no DOSBox setting. The base's counts (the merge-base with `origin/main`, measured by its own tree) are the baseline and the PR gate:

    uv run --project tools python tools/bench/bench.py

A count above the base's fails unless a waiver (`tools/bench/waivers`) names it; one below passes and is the next base. No file holds the compiler's counts: only the reference compilers' are stored (`bench/NAME/expected.toml`). See [tools/bench](../../../tools/bench/readme.md) and [bench](../../../bench/readme.md).

It is an executed-instruction count. It models no latency, so it ranks code for an in-order machine and says nothing about a Pentium's pairing or a 486's cache.

## The 486 clock model: `vsgcc/harness.py`

The rows table's clocks are the i486 data sheet's per-instruction counts (Intel 240440-002, table 10.1) summed over the instructions executed, plus three clocks for each misaligned memory operand. It is blind to code alignment, the prefetch queue and cache-line fills: a change that only moves code (function and block alignment, E2) cannot change a clock, and that is by construction. "No clock gain" from such a change is not evidence that it gains nothing on hardware; its cost, in bytes, is the only thing the rows show. Judge such a change by the bytes and by a hardware or cycle-accurate measurement, not by the clocks column.

## What the compiler expects: `tools/sizes.py`

Object code bytes of every program, for two builds side by side. It prints no instruction or memory-operand column: the backend's per-call cost estimate, summed, was not comparable across builds (#390, #433, #481). `tools/bench` counts what runs.

    python3 tools/sizes.py BASE_BIN_DIR [NEW_BIN_DIR] [-O2 -Os ...]

## How long: RDTSC, never the PIT

`bench.py --time` reads RDTSC around the whole program (`tools/bench/timeit.asm`) in a DOSBox at a fixed 75000 cycles per millisecond, so cycles / 75000 is milliseconds on any host. DOSBox-X charges about one cycle per instruction and models no latency: the time follows the whole program's executed instructions, start-up and printing included. It compares one program across commits and is never gated.

The PIT is out. `TIMER` ticks at 18.2 Hz, and latching the 8253 under DOSBox-X still tears by a tick on the same binary: the noise is DOSBox-X's own scheduling.

## Over time: the history and the dashboard

`tools/bench/history.py` appends one JSON line per main commit, benchmark, language and level to `results.jsonl` on the `bench-history` branch, which holds only that file. `tools/bench/dashboard.py` draws a static HTML page from it: each benchmark's trend per language, the gap between languages, and the reference compilers.

`bench.py --references` measures the reference compilers on the same programs; they are never gated.

## What the compiler costs: `tools/measure.py`, `tools/compile-cost.py`

User-space instructions of the compiler itself (`perf stat -e instructions:u`), so host load does not move them. `tools/compile-cost.py` prints the measurement, `tools/measure.py` compares it with the merge-base's, `tools/pass-profile.py` shows where it goes by step, and `tools/scoreboard.py` sets it beside gcc's. [compile-time](compile-time.md) holds the method and the gate.

## Against other compilers

`crates/target/llrm-x86-m32/vsgcc` measures llrm's m32 output against gcc and clang in one emulator; [vsgcc](../../../crates/target/llrm-x86-m32/vsgcc/readme.md) is its usage. Comparisons with gcc, clang, BC, Turbo C, tcc and bcc are in [comparison](../../comparison/readme.md).

## Asking the right question

A measurement that contradicts what is known of its subject is evidence the instrument is wrong. A question shaped so the subject's own style cannot answer it returns zero: asking whether BC leaves anything on the table by comparing programs whose loops constant-fold measures the folding, not the loop. Give a program inputs the compiler cannot see (a value read from `DATA`, a bound that is not a literal) before reading a ratio off it, and read the listing before the number.
