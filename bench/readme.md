# Benchmarks

One directory per benchmark: `NAME.bas`, `NAME.c`, `NAME.nib` and the one `NAME.out`
that all three print. `tests/run` builds and runs each variant in DOSBox.

Each variant has a kernel function (`bench_NAME` in C and Nib, `BenchName` in
BASIC) that `main` calls and whose result it prints, as a signed 32-bit decimal on
its own line: C through `report(long)` (runtime/c/<target>/ext.asm), Nib with
`print`, BASIC with `PRINT LTRIM$(STR$(result))`.

A header comment sets a variant's flags, or marks it `known: #ISSUE`.
