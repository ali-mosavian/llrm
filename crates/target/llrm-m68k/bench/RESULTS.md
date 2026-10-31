# Results, 2026-10-31 18:49Z (pin d1106deb71cc, qemu-m68k -cpu m68000)

gcc is m68k-linux-gnu-gcc 15.2.0 (`-mcpu=68000`), the Debian cross toolchain unpacked without root. "insns" are
executed instructions inside `k`; the clock columns of the last table price them from Motorola's tables
(`src/timing.rs`). The default output is the pin's own: `ours` is the pin's selection and allocation over the
LSR mid-end, with no rewrite after selection (see `../PARITY.md`).

**raw** is our output on the IR as written; **ours** adds the loop strength reduction `llc` also runs (the llc and gcc
columns include theirs). Compare ours with llc, raw with nothing.

## Summary: every row against the best reference at its level

Speed level (`ours`, `raw`) against `llc -O2` and `gcc -O2`; size level (`ours -Os`: `optsize`/`minsize` or `Options::size`) against `llc -Os` and `gcc -Os`.

| kernel | raw B | ours B | best -O2 B | ours -Os B | best -Os B | raw insns | ours insns | best -O2 insns | ours -Os insns | best -Os insns | status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 22 | 22 | 18 | 22 | 16 | 44 | 44 | 44 | 44 | 43 | -O2 +4 B, -Os +6 B, -Os +1 insns |
| popcount | 52 | 52 | 22 | 52 | 22 | 103 | 103 | 85 | 103 | 85 | -O2 +30 B, -O2 +18 insns, -Os +30 B, -Os +18 insns |
| max | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 5 | 6 | 5 | -O2 +1 insns, -Os +1 insns |
| absdiff | 22 | 22 | 22 | 22 | 22 | 7 | 7 | 7 | 7 | 7 | at or below |
| clamp | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | at or below |
| fib | 56 | 56 | 28 | 56 | 22 | 79 | 79 | 67 | 79 | 64 | -O2 +28 B, -O2 +12 insns, -Os +34 B, -Os +15 insns |
| gcd | 26 | 26 | 28 | 26 | 26 | 17 | 17 | 16 | 17 | 17 | -O2 +1 insns |
| arrsum | 56 | 60 | 22 | 60 | 20 | 105 | 121 | 69 | 121 | 68 | -O2 +38 B, -O2 +52 insns, -Os +40 B, -Os +53 insns |
| arrmax | 62 | 78 | 32 | 78 | 26 | 132 | 151 | 100 | 151 | 97 | -O2 +46 B, -O2 +51 insns, -Os +52 B, -Os +54 insns |
| countgt | 78 | 82 | 42 | 82 | 42 | 186 | 202 | 123 | 202 | 168 | -O2 +40 B, -O2 +79 insns, -Os +40 B, -Os +34 insns |
| strlen | 24 | 24 | 20 | 24 | 18 | 59 | 59 | 44 | 59 | 57 | -O2 +4 B, -O2 +15 insns, -Os +6 B, -Os +2 insns |
| copy | 38 | 38 | 34 | 38 | 34 | 59 | 59 | 59 | 59 | 59 | -O2 +4 B, -Os +4 B |

Rows still above a reference: sum, popcount, max, fib, gcd, arrsum, arrmax, countgt, strlen, copy.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| sum | 22 | 22 | 22 | 46 | 46 | 16 | 18 | 44 | 44 | 44 | 48 | 48 | 43 | 44 | 45 all |
| popcount | 52 | 52 | 52 | 52 | 52 | 22 | 22 | 103 | 103 | 103 | 103 | 103 | 85 | 85 | 8 all |
| max | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 6 | 6 | 6 | 6 | 6 | 5 | 5 | 42 all |
| absdiff | 22 | 22 | 22 | 22 | 22 | 24 | 24 | 7 | 7 | 7 | 7 | 7 | 8 | 7 | 42 all |
| clamp | 26 | 26 | 26 | 26 | 26 | 26 | 26 | 9 | 9 | 9 | 9 | 9 | 9 | 9 | 42 all |
| fib | 56 | 56 | 56 | 66 | 66 | 22 | 28 | 79 | 79 | 79 | 139 | 139 | 64 | 67 | 89 all |
| gcd | 26 | 26 | 26 | 32 | 26 | 26 | 28 | 17 | 17 | 17 | 16 | 17 | 17 | 16 | 42 all |
| arrsum | 56 | 60 | 60 | 64 | 64 | 20 | 22 | 105 | 121 | 121 | 153 | 153 | 68 | 69 | 80 all |
| arrmax | 62 | 78 | 78 | 92 | 88 | 26 | 32 | 132 | 151 | 151 | 248 | 235 | 97 | 100 | 9 all |
| countgt | 78 | 82 | 82 | 86 | 86 | 42 | 42 | 186 | 202 | 202 | 234 | 234 | 168 | 123 | 9 all |
| strlen | 24 | 24 | 24 | 36 | 36 | 18 | 20 | 59 | 59 | 59 | 89 | 89 | 57 | 44 | 13 all |
| copy | 38 | 38 | 38 | 42 | 42 | 34 | 34 | 59 | 59 | 59 | 59 | 59 | 59 | 59 | 33 all |

## Above the best reference (bytes / insns)

Rows above gcc are the pin's output; the rows above `llc` are listed in `../PARITY.md`.

## Clocks

Executed instructions of `k` priced for the 68000 and for the 68020 (cache case); `*` marks a
data-dependent term at its bound. `cargo run -p llrm-m68k --example cycles kernels`.

| kernel | B | insns | 68000 clk | 68020 clk | -Os B | -Os insns | -Os 68000 | -Os 68020 |
|---|---|---|---|---|---|---|---|---|
| sum | 22 | 44 | 458 | 179 | 22 | 44 | 458 | 179 |
| popcount | 52 | 103 | 1070 | 472 | 52 | 103 | 1070 | 472 |
| max | 16 | 6 | 68 | 32 | 16 | 6 | 68 | 32 |
| absdiff | 22 | 7 | 78 | 36 | 22 | 7 | 78 | 36 |
| clamp | 26 | 9 | 102 | 47 | 26 | 9 | 102 | 47 |
| fib | 56 | 79 | 662 | 282 | 56 | 79 | 662 | 282 |
| gcd | 26 | 17 | 168 | 74 | 26 | 17 | 168 | 74 |
| arrsum | 60 | 121 | 1282 | 545 | 60 | 121 | 1282 | 545 |
| arrmax | 78 | 151 | 1492 | 651 | 78 | 151 | 1492 | 651 |
| countgt | 82 | 202 | 2082* | 879* | 82 | 202 | 2082* | 879* |
| strlen | 24 | 59 | 706 | 367 | 24 | 59 | 706 | 367 |
| copy | 38 | 59 | 838 | 380 | 38 | 59 | 838 | 380 |
