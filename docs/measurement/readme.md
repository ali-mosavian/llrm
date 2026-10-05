# Measuring

Each instrument answers one question. Quoting one for another is how the project acquired figures it later
had to withdraw.

## Does it run right: `tests/`

| Folder | Judged by | Run by |
|---|---|---|
| `tests/check/` | CHECK lines against a tool's text output (lit and FileCheck style) | `cargo test --test check` |
| `tests/run/`, `examples/`, `bench/` | the program's stdout against its `.out`, built by llrm and run in DOSBox | `cargo test --test run` |
| `tests/differential/` | another compiler: BC 4.5 against llrm-qb, and the Microsoft BASIC conformance suites on their own runtimes | `cargo test --test differential` |

A `.out` is derived from what the program means, never from a compiler's output alone. A header comment
(`' flags:`, `' dialect:`, `' known: #N`) holds a program's settings; a known program that passes fails the
run until its mark goes. `bench/NAME/` holds `NAME.bas`, `NAME.c`, `NAME.nib` and the one `NAME.out` all three
print; a missing variant needs an issue in `bench.toml`'s `[gaps]`.

## How much work: `tools/bench`

Executed instructions and memory operands of each benchmark's kernel, per language and level, counted in a
real-mode emulator. Deterministic: no clock, no DOSBox setting. `bench/NAME/expected.toml` is the baseline and
the PR gate:

    uv run --project tools python tools/bench/bench.py

A count above its baseline fails; one below fails too until `--bless --reason "..."` records it, so the baseline
only ratchets and every change carries its reason. See `tools/bench/readme.md`.

This is an executed-instruction count. It models no latency, so it ranks code for an in-order machine and says
nothing about a Pentium's pairing or a 486's cache.

## What the compiler expects: `tools/sizes.py`

Object bytes and the backend's cost estimate (instructions and memory operands per call, summed) of every
program. An estimate on MIR before lowering, not an execution: use it to find what moved, and `tools/bench` to
say what it cost. Totals for the current program set are in `numbers.md`.

## How long: RDTSC, never the PIT

`bench.py --time` reads RDTSC around the whole program (`tools/bench/timeit.asm`) in a DOSBox at a fixed 75000
cycles per millisecond, so cycles / 75000 is milliseconds on any host. DOSBox-X charges about one cycle per
instruction and models no latency: the time follows the whole program's executed instructions, start-up and
printing included. It compares one program across commits and is never gated.

The PIT is out. `TIMER` ticks at 18.2 Hz, and latching the 8253 under DOSBox-X still tears by a tick on the
same binary: the noise is DOSBox-X's own scheduling, about one 18.2 Hz tick, not something a mask or a retry
removes.

## Over time: the history and the dashboard

`tools/bench/history.py` appends one JSON line per main commit x benchmark x language x level to
`results.jsonl` on the `bench-history` branch, which holds only that file. `tools/bench/dashboard.py` draws a
static HTML page from it: each benchmark's trend per language, the gap between languages, and the reference
compilers. A gap that widens is a regression, so the page ranks the benchmarks by it.

`bench.py --references` measures Open Watcom (C) and BC 4.5 (BASIC) on the same programs. They are never gated.
A reference that cannot be built records why: Open Watcom needs its 16-bit libraries (#373).

## What makes a number quotable

Every row in `numbers.md` carries the dosbox-x version, the sha256 of the conf, of BC.EXE, LINK.EXE and the
runtime library, the configuration tag, the llrm git revision, the host and the date. A number without that stamp
is not quotable. `tools/bench` counts need only the commit: they do not depend on the host.
