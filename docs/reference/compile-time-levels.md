# Compile time of every level, llrm against gcc and clang

TL;DR: llrm compiles the 29 bench kernels in 1.7x (`-O0`) to 3.2x (`-O1`, `-Os`) the instructions gcc 13.4.0 spends and 1.9x to 3.6x clang 20.1.8's; at `-O2` 2.36x gcc and 3.10x clang, worst program 7.1x (matmul) and 7.0x (nbody_fixed). Levels are those of PR #887 (gcc's definitions), measured at its head.

Reproduce: `uv run --project tools python crates/target/llrm-x86-m32/vsgcc/levels_time.py` (about two minutes; `--extra DIR` adds the `*.c` of DIR that gcc and clang both accept).

## What is timed

User instructions retired (`perf stat -e instructions:u`, children included), median of 3 runs, one process per compile, source to object, `-m32 -march=i486` on the same source text (the bench kernels include no header, so the text is the preprocessed text; gcc and clang get `-Dfar=`). Not wall clock: the machine is shared. Worst spread over the three runs of any cell: 0.5%.

- llrm-c: C front end (preprocessor included), HIR, MIR passes, instruction selection, register allocation, OMF object writer; one process, no assembler.
- gcc -c: driver, cc1 (preprocessor and compiler) and `as`.
- clang -c: driver and the cc1 process with its integrated assembler.

Start-up, compiling an empty file (instructions): llrm 8.1M (`-O0`), 8.7M (`-O2`); gcc 18.7M, 18.7M; clang 41.3M, 43.4M. Each ratio below includes it; it is a larger part of the smaller programs. Not measured: wall clock, memory, `-g`, and the m16 target against ia16-elf-gcc (not installed on this host).

## Ratio of llrm to the other compiler, geomean and worst program

| level | llrm/gcc | worst | llrm/clang | worst |
|---|---|---|---|---|
| -O0 | 1.69x | 4.45x fpbench | 1.95x | 5.25x fpbench |
| -O1 | 3.25x | 8.90x matmul | 3.19x | 8.81x nbody_fixed |
| -O2 | 2.36x | 7.09x matmul | 3.10x | 6.98x nbody_fixed |
| -O3 | 1.99x | 14.40x nbody_fixed | 3.54x | 24.79x nbody_fixed |
| -Os | 3.24x | 7.84x matmul | 3.60x | 8.68x matmul |
| -Omax (against -O3) | 2.03x | 14.39x nbody_fixed | 3.60x | 24.78x nbody_fixed |

## Per program at -O2 (millions of instructions)

| program | llrm | gcc | clang | llrm/gcc | llrm/clang |
|---|---|---|---|---|---|
| bintree | 380.0 | 111.6 | 82.3 | 3.40x | 4.62x |
| crc | 132.0 | 64.0 | 78.1 | 2.06x | 1.69x |
| fib | 88.2 | 203.0 | 56.1 | 0.43x | 1.57x |
| floats | 85.5 | 59.7 | 62.1 | 1.43x | 1.38x |
| fpbench | 583.9 | 140.9 | 130.1 | 4.14x | 4.49x |
| frames | 176.0 | 213.5 | 71.6 | 0.82x | 2.46x |
| hanoi | 102.2 | 145.5 | 57.7 | 0.70x | 1.77x |
| histo | 289.1 | 84.1 | 80.3 | 3.44x | 3.60x |
| lru | 548.3 | 108.5 | 84.0 | 5.05x | 6.53x |
| mandel | 287.0 | 81.4 | 84.2 | 3.53x | 3.41x |
| matmul | 868.2 | 122.4 | 204.7 | 7.09x | 4.24x |
| nbody | 561.5 | 121.8 | 99.8 | 4.61x | 5.62x |
| nbody_fixed | 810.7 | 137.4 | 116.2 | 5.90x | 6.98x |
| nbody_single | 588.9 | 123.9 | 116.0 | 4.75x | 5.08x |
| particle | 420.7 | 107.2 | 99.1 | 3.92x | 4.25x |
| queens | 433.2 | 88.4 | 71.1 | 4.90x | 6.09x |
| quicksort | 517.2 | 108.0 | 90.4 | 4.79x | 5.72x |
| recchop | 238.3 | 304.8 | 75.2 | 0.78x | 3.17x |
| recfloat | 94.7 | 67.1 | 63.9 | 1.41x | 1.48x |
| recgcd | 207.5 | 72.7 | 68.0 | 2.85x | 3.05x |
| recmany | 204.2 | 74.9 | 76.7 | 2.73x | 2.66x |
| recpow | 168.4 | 358.5 | 74.9 | 0.47x | 2.25x |
| recsum | 93.3 | 69.4 | 64.1 | 1.34x | 1.46x |
| rectwo | 110.5 | 185.6 | 56.2 | 0.60x | 1.97x |
| ring | 127.7 | 65.5 | 66.8 | 1.95x | 1.91x |
| scroll | 340.3 | 95.3 | 84.5 | 3.57x | 4.03x |
| shellsort | 464.9 | 95.0 | 182.7 | 4.89x | 2.55x |
| sieve | 345.1 | 83.7 | 81.2 | 4.12x | 4.25x |
| tile | 217.6 | 81.8 | 79.3 | 2.66x | 2.74x |

## With the 37 x_* kernels (66 programs)

Same method, `vsgcc/kernels/` included (llrm at the levels of main on 2026-10-08, `-O2` still the growing one): llrm/gcc and llrm/clang, geomean and worst program.

| level | llrm/gcc | worst | llrm/clang | worst |
|---|---|---|---|---|
| -O0 | 1.68x | 4.5x fpbench | 1.94x | 5.2x fpbench |
| -O1 | 3.90x | 14.3x x_life | 3.75x | 18.4x x_life |
| -O2 | 3.19x | 29.8x nbody_fixed | 3.85x | 35.3x nbody_fixed |
| -O3 | 2.43x | 14.4x nbody_fixed | 3.70x | 24.8x nbody_fixed |
| -Os | 3.47x | 11.8x x_life | 3.88x | 16.8x x_life |
