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
| memcpy | 94 | 98 | 40 | 98 | 40 | 157 | 145 | 51 | 145 | 102 | -O2 +58 B, -O2 +94 insns, -Os +58 B, -Os +43 insns |
| memset | 78 | 86 | 52 | 86 | 44 | 82 | 115 | 64 | 115 | 93 | -O2 +34 B, -O2 +51 insns, -Os +42 B, -Os +22 insns |
| strcmp | 100 | 104 | 82 | 104 | 48 | 149 | 151 | 102 | 151 | 104 | -O2 +22 B, -O2 +49 insns, -Os +56 B, -Os +47 insns |
| hex | 124 | 128 | 78 | 128 | 72 | 77 | 78 | 54 | 78 | 57 | -O2 +50 B, -O2 +24 insns, -Os +56 B, -Os +21 insns |
| crc16 | 138 | 140 | 86 | 140 | 76 | 729 | 658 | 629 | 658 | 630 | -O2 +54 B, -O2 +29 insns, -Os +64 B, -Os +28 insns |
| bsearch | 106 | 106 | 76 | 106 | 66 | 66 | 66 | 61 | 66 | 64 | -O2 +30 B, -O2 +5 insns, -Os +40 B, -Os +2 insns |
| partition | 152 | 144 | 92 | 140 | 94 | 200 | 171 | 124 | 167 | 199 | -O2 +52 B, -O2 +47 insns, -Os +46 B |
| muldiv | 82 | 82 | 64 | 82 | 64 | - | - | - | - | - | -O2 +18 B, -Os +18 B |
| structcopy | 54 | 54 | 38 | 54 | 42 | - | - | - | - | - | -O2 +16 B, -Os +12 B |
| fnptr | 56 | 56 | 46 | 56 | 46 | 16 | 16 | 16 | 16 | 16 | -O2 +10 B, -Os +10 B |

Rows still above a reference: memcpy, memset, strcmp, hex, crc16, bsearch, partition, muldiv, structcopy, fnptr.

## Full table

| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| memcpy | 94 | 98 | 98 | 106 | 106 | 40 | 40 | 157 | 145 | 145 | 197 | 197 | 102 | 51 | 33 all |
| memset | 78 | 86 | 86 | 90 | 90 | 44 | 52 | 82 | 115 | 115 | 147 | 147 | 93 | 64 | 14 all |
| strcmp | 100 | 104 | 104 | 114 | 114 | 48 | 82 | 149 | 151 | 151 | 196 | 196 | 104 | 102 | 32 all |
| hex | 124 | 128 | 128 | 142 | 140 | 72 | 78 | 77 | 78 | 78 | 94 | 97 | 57 | 54 | 204 all |
| crc16 | 138 | 140 | 140 | 158 | 154 | 76 | 86 | 729 | 658 | 658 | 744 | 704 | 630 | 629 | 152 all |
| bsearch | 106 | 106 | 106 | 106 | 102 | 66 | 76 | 66 | 66 | 66 | 64 | 64 | 68 | 61 | 10 all |
| partition | 152 | 144 | 140 | 150 | 146 | 94 | 92 | 200 | 171 | 167 | 202 | 199 | 219 | 124 | 19 all |
| muldiv | 82 | 82 | 82 | 82 | 82 | 64 | 64 | - | - | - | - | - | - | - | 236 all |
| structcopy | 54 | 54 | 54 | 76 | 66 | 42 | 38 | - | - | - | - | - | - | - | 40 all |
| fnptr | 56 | 56 | 56 | 56 | 56 | 46 | 46 | 16 | 16 | 16 | 16 | 16 | 16 | 16 | 36 all |

## Above the best reference (bytes / insns)

Rows above gcc are the pin's output; the rows above `llc` are listed in `../PARITY.md`.

## Clocks

Executed instructions of `k` priced for the 68000 and for the 68020 (cache case); `*` marks a
data-dependent term at its bound. `cargo run -p llrm-m68k --example cycles library`.

| kernel | B | insns | 68000 clk | 68020 clk | -Os B | -Os insns | -Os 68000 | -Os 68020 |
|---|---|---|---|---|---|---|---|---|
| memcpy | 98 | 145 | 1516 | 693 | 98 | 145 | 1516 | 693 |
| memset | 86 | 115 | 1158 | 538 | 86 | 115 | 1158 | 538 |
| strcmp | 104 | 151 | 1280 | 654 | 104 | 151 | 1280 | 654 |
| hex | 128 | 78 | 866 | 384 | 128 | 78 | 866 | 384 |
| crc16 | 140 | 658 | 7398 | 3252 | 140 | 658 | 7398 | 3252 |
| bsearch | 106 | 66 | 680 | 297 | 106 | 66 | 680 | 297 |
| partition | 144 | 171 | 1862 | 810 | 140 | 167 | 1848 | 812 |
| muldiv | 82 | 22 | 336 | 137 | 82 | 22 | 336 | 137 |
| structcopy | 54 | 14 | 240 | 92 | 54 | 14 | 240 | 92 |
| fnptr | 56 | 16 | 268 | 120 | 56 | 16 | 268 | 120 |
