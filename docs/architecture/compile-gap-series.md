# Compile-cost gap against clang and gcc, by date

Each table is one run of `tools/compile-gap.py` (`-m32 -march=i486 -fno-inline-functions`, an object written by each compiler) with the tree's dist-profile `llrm-c`, on a given commit of `main`. Cost is user-space instructions (`perf stat`, quickest of three by cycles), in millions. The ratio is llrm over the cheaper of clang 20.1.8 and gcc 13.4.0. Instructions are the claim: the host is loaded, so cycles are not compared.

A table is read against the one before it: the delta column is the change in llrm's instructions and in the ratio since the previous date. The worst ratio is named beside the geometric mean of the ratios.

## 2026-10-17, `758067eba` (main with #1964, the baseline)

| program | level | llrm | clang | gcc | ratio |
|---|---|---:|---:|---:|---:|
| d_faces | -O1 | 2,422 | 478 | 675 | 5.1x |
| d_faces | -O2 | 3,143 | 554 | 1,176 | 5.7x |
| d_faces | -O3 | 3,469 | 555 | 1,460 | 6.2x |
| d_faces | -Os | 3,416 | 550 | 979 | 6.2x |
| lru | -O1 | 104 | 74 | 91 | 1.4x |
| lru | -O2 | 122 | 76 | 123 | 1.6x |
| lru | -O3 | 125 | 76 | 125 | 1.7x |
| lru | -Os | 119 | 76 | 110 | 1.6x |
| bintree | -O1 | 71 | 69 | 86 | 1.0x |
| bintree | -O2 | 102 | 74 | 114 | 1.4x |
| bintree | -O3 | 108 | 75 | 115 | 1.5x |
| bintree | -Os | 94 | 74 | 96 | 1.3x |
| queens | -O1 | 79 | 69 | 73 | 1.1x |
| queens | -O2 | 101 | 71 | 93 | 1.4x |
| queens | -O3 | 1,553 | 71 | 634 | 21.8x |
| queens | -Os | 92 | 71 | 86 | 1.3x |
| sieve | -O1 | 55 | 66 | 71 | 0.8x |
| sieve | -O2 | 65 | 68 | 86 | 1.0x |
| sieve | -O3 | 92 | 68 | 97 | 1.4x |
| sieve | -Os | 58 | 68 | 79 | 0.9x |

Worst ratio: queens -O3, 21.8x. `analysis bounded` is 451 M of its 1,553 M (216 runs), with `mir decide` 114 M and `mir hoist` 111 M; the object is 1,519 bytes against clang's 1,420. The next worst: d_faces -O3 and -Os, 6.2x. Geometric mean of the 20 ratios: 2.0x (d_faces alone, over its four levels: 5.8x).

d_faces is the largest file of the QCport tree this script compiles. The Borland-header QCport (`dos.h`) is not a three-way row: gcc and clang cannot parse its headers.

d_faces, llrm's own work by phase (Minstr, share): at -O2 `mir` 1,342 (42%), `analysis` 702 (22%), `regalloc` 479 (15%), `lir` 365 (11%), `isel` 56 (2%); clang's optimizer is 272 M (49% of its time) and its codegen 216 M (39%).

Checked: one commit on all sides (the compilers do not change between tables; llrm-c is the dist build of the commit above); each compile writes an object; a failed compile stops the run (the first run stopped on `lru`, whose `far` pointers clang cannot parse: the Borland keywords are now defined away for every program, as for d_faces); `queens` -O3 was run twice more for llrm alone (1,552,975,343 and 1,552,968,042 instructions).
