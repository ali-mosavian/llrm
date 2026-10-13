# Benchmarks

Layout of the `bench/` directory, for contributors adding or changing a benchmark.
The tool that measures them is described in [tools/bench](../tools/bench/readme.md).

## A benchmark

One directory per benchmark, `bench/NAME/` (or `bench/parity/NAME/`, same layout):

```
bench/NAME/
  NAME.bas  NAME.c  NAME.nib    one variant per language (the .bas is built three ways: bas, bq16, bq32)
  NAME.out                      what all three print
  bench.toml                    description, kernel names, gaps
  expected.toml                 the blessed counts (bench.py --bless)
```

Each variant has a kernel function (`bench_NAME` in C and Nib, `BenchName` in
BASIC) that `main` calls and whose result it prints, as a signed 32-bit decimal
on its own line: C through `report(long)` (`runtime/c/<target>/ext.asm`), Nib
with `print`, BASIC with `PRINT LTRIM$(STR$(result))`.

A header comment sets a variant's flags (`// flags: -O2 -march=i486 | ...`
builds it once per alternative), or marks it `known: #ISSUE`. `tests/run` builds
and runs every variant in DOSBox against `NAME.out`.

## bench.toml

```toml
description = "..."
repetitions = 1
timed_only = true      # optional: no instruction count, timed in DOSBox

[region]               # the kernel's symbol, per language
bas = "BenchFib"
c = "bench_fib"
nib = "bench_fib"

[gaps]                 # optional: a language the benchmark lacks, or fakes
bas = "#364 no unsigned types"
```

Every benchmark has all three variants, its `.out` and its `bench.toml`. A
variant may be missing, or use an idiom instead of the real thing, only when
`[gaps]` names the language with the issue that tracks it. `tests/run.rs`
(`test_every_benchmark_exists_in_basic_c_and_nib`) enforces it.

## expected.toml

`reason` and `blessed` (a date) say why and when the file last changed. One
table per language and level, `[c.O2]`, `[nib.Os]`, ..., holds `instructions`,
`memory_operands`, `code_bytes`, `data_bytes`, `bss_bytes` and, for DOSBox-timed
variants, `kernel_ms`. Tables `[bcc.*]`, `[ow.*]`, `[tc.*]`, `[tcpp.*]`,
`[bc.*]`, `[pds71.*]` and `[vbdos.*]` hold the reference compilers' counts.

Only `tools/bench/bench.py --bless --reason TEXT` writes it.

`bench/parity/expected.json` is the value each parity program returns, read by
`tests/toolchain.rs`.

`bench_io.h` is the file API the benchmarks that read or write files share.
