---
architecture-index: compile-cost gap by date
architecture-order: 410
---
# Compile-cost gap against clang and gcc, by date

Each table is one run of `tools/compile-gap.py` (`-m32 -march=i486 -fno-inline-functions`, an object written by each compiler) with the tree's dist-profile `llrm-cc`, on a given commit of `main`. Cost is user-space instructions (`perf stat`, quickest of three by cycles), in millions. The ratio is llrm over the cheaper of clang 20.1.8 and gcc 13.4.0. **Reference note:** every clang column in this file before the section of `5e2e995ef` is the distro clang 20.1.8 (`tools/compile-gap.py` defaults `--clang` to the `PATH` clang), not the pinned 24.0.0git `d1106deb71cc`; those clang figures (for example 553 / 555 M for `d_faces` -O2 / -O3) and the ratios derived from them (5.1x, 5.6x) are void as a reference to clang 24, whose `d_faces` -O2 is 415.5 M. The section of `5e2e995ef` gives both; the scoreboard in the plan uses the pinned binary only. Instructions are the claim: the host is loaded, so cycles are not compared.

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

Checked: one commit on all sides (the compilers do not change between tables; llrm-cc is the dist build of the commit above); each compile writes an object; a failed compile stops the run (the first run stopped on `lru`, whose `far` pointers clang cannot parse: the Borland keywords are now defined away for every program, as for d_faces); `queens` -O3 was run twice more for llrm alone (1,552,975,343 and 1,552,968,042 instructions).

## 2026-10-17, `5344082af` (after the first batch)

Landed since the baseline: the live sets as bit sets (#1926, #1934), the graph's predecessors and splitkit's live blocks (#1953), the crate's hashed collections as Fx (#1927), gvn's forecast skipped past crossing's price (#1957), a result derived from the body as it stands surviving an invalidation (#1922), the dead code after a loop found from what the loop stopped reading (#1936), and "don't ask for what the body cannot use" (#1968). The delta is llrm's instructions against the baseline table.

| program | level | llrm | delta | clang | gcc | ratio |
|---|---|---:|---:|---:|---:|---:|
| d_faces | -O1 | 2,263 | -159 (-6.6%) | 478 | 675 | 4.7x |
| d_faces | -O2 | 2,830 | -313 (-10.0%) | 553 | 1,176 | 5.1x |
| d_faces | -O3 | 3,133 | -336 (-9.7%) | 555 | 1,460 | 5.6x |
| d_faces | -Os | 2,896 | -520 (-15.2%) | 550 | 979 | 5.3x |
| lru | -O1 | 98 | -6 (-5.8%) | 74 | 91 | 1.3x |
| lru | -O2 | 114 | -8 (-6.6%) | 76 | 123 | 1.5x |
| lru | -O3 | 117 | -8 (-6.4%) | 76 | 125 | 1.5x |
| lru | -Os | 111 | -8 (-6.7%) | 76 | 110 | 1.5x |
| bintree | -O1 | 66 | -5 (-7.0%) | 69 | 86 | 1.0x |
| bintree | -O2 | 91 | -11 (-10.8%) | 75 | 114 | 1.2x |
| bintree | -O3 | 97 | -11 (-10.2%) | 75 | 115 | 1.3x |
| bintree | -Os | 88 | -6 (-6.4%) | 74 | 96 | 1.2x |
| queens | -O1 | 75 | -4 (-5.1%) | 69 | 73 | 1.1x |
| queens | -O2 | 94 | -7 (-6.9%) | 71 | 93 | 1.3x |
| queens | -O3 | 1,419 | -134 (-8.6%) | 71 | 634 | 20.0x |
| queens | -Os | 86 | -6 (-6.5%) | 71 | 86 | 1.2x |
| sieve | -O1 | 52 | -3 (-5.5%) | 66 | 71 | 0.8x |
| sieve | -O2 | 61 | -4 (-6.2%) | 68 | 86 | 0.9x |
| sieve | -O3 | 86 | -6 (-6.5%) | 68 | 97 | 1.3x |
| sieve | -Os | 54 | -4 (-6.9%) | 68 | 79 | 0.8x |

Worst ratio: queens -O3, 20.0x (was 21.9x), still `analysis bounded` and the loop passes. Geometric mean of the 20 ratios: 1.8x (was 2.0x); d_faces alone over its four levels: 5.2x (was 5.8x). Every row fell by 5% to 15%; the gap closed by about a tenth and no row is below one but sieve.

d_faces, llrm's own work by phase at -O2: `mir` 1,111 (38%), `analysis` 651 (22%), `regalloc` 456 (16%), `lir` 365 (13%); clang's optimizer is 274 M and its codegen 220 M.

Checked: both sides of each delta are a dist build, user-space instructions, quickest of three by cycles, the compilers unchanged; clang's and gcc's columns moved by at most one in the last digit between the two runs (noise of the best-of-three), so the delta is llrm's.

## 2026-10-17, `954bc48fa`: the static clang, and the profile-guided compiler

The clang column changes here: it is a static build of the pinned tag llvmorg-20.1.8 (87f0227c; recipe in [measured.md](../comparison/llvm/measured.md)), not the distro's. The distro's links libLLVM as a shared library, which adds about 30 M of dynamic-loader start-up to every compile and calls into the library through the PLT: d_faces -O2 reads 553 M with it and 462 M static, an empty compile 43 M against 13 M. The distro build stays as a column, what users run; gcc is the distro's. `tools/compile-gap.py` takes `--clang PATH` and `--distro-clang PATH`. The ratio is llrm over the cheaper of the static clang and gcc, so the small programs, whose cost the distro's start-up inflated, read higher than in the tables above; no llrm number changed from the table of `5344082af`.

The profile-guided column is `tools/pgo/pgo.py build` of the same commit ([pgo.md](../working/pgo.md)). Its profile was made from 264 compiles of the bench C programs at -m32 and -m16 and -O1, -O2 and -Os, leaving out lru, bintree, queens and sieve (`--exclude`) and QCport's render modules (no `QCPORT`), so no program here trained it and -O3 is a level it never saw. `tools/identical.py` finds the same bytes from it and from the plain build over the whole corpus: 1,583 objects, 0 differ.

| program | level | llrm | llrm PGO | clang | gcc | clang (distro) | ratio | ratio PGO |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| d_faces | -O1 | 2,263 | 1,943 | 397 | 675 | 479 | 5.7x | 4.9x |
| d_faces | -O2 | 2,830 | 2,412 | 462 | 1,176 | 553 | 6.1x | 5.2x |
| d_faces | -O3 | 3,133 | 2,666 | 464 | 1,460 | 556 | 6.8x | 5.7x |
| d_faces | -Os | 2,896 | 2,479 | 459 | 979 | 550 | 6.3x | 5.4x |
| lru | -O1 | 98 | 82 | 41 | 91 | 74 | 2.4x | 2.0x |
| lru | -O2 | 114 | 95 | 43 | 123 | 76 | 2.7x | 2.2x |
| lru | -O3 | 117 | 99 | 43 | 125 | 76 | 2.7x | 2.3x |
| lru | -Os | 111 | 93 | 44 | 110 | 76 | 2.5x | 2.1x |
| bintree | -O1 | 66 | 56 | 37 | 86 | 69 | 1.8x | 1.5x |
| bintree | -O2 | 91 | 77 | 42 | 114 | 74 | 2.2x | 1.8x |
| bintree | -O3 | 97 | 82 | 42 | 115 | 75 | 2.3x | 2.0x |
| bintree | -Os | 88 | 74 | 41 | 96 | 74 | 2.1x | 1.8x |
| queens | -O1 | 75 | 63 | 38 | 73 | 69 | 2.0x | 1.7x |
| queens | -O2 | 94 | 79 | 39 | 93 | 71 | 2.4x | 2.0x |
| queens | -O3 | 1,419 | 1,165 | 39 | 634 | 71 | 36.4x | 29.9x |
| queens | -Os | 86 | 72 | 39 | 86 | 71 | 2.2x | 1.8x |
| sieve | -O1 | 52 | 45 | 35 | 71 | 66 | 1.5x | 1.3x |
| sieve | -O2 | 61 | 52 | 37 | 86 | 68 | 1.6x | 1.4x |
| sieve | -O3 | 86 | 72 | 37 | 97 | 68 | 2.3x | 1.9x |
| sieve | -Os | 54 | 46 | 36 | 79 | 68 | 1.5x | 1.3x |

Worst ratio: queens -O3, 36.4x (29.9x profile-guided). Geometric mean of the 20 ratios: 3.0x (2.6x profile-guided); d_faces alone over its four levels: 6.2x (5.3x). Profile-guided instructions are 14.1% to 17.9% below the plain build's on every row. Against the distro clang the same mean is 1.8x.

Where the compile goes beside the pass work, d_faces (callgrind, dist build): copying a `Function` (gvn's forks, hoist's clones, interprocedural's, sink's) is about 0.1% of the compile, not the 6% to 10% of the scopes that contain the copies: what those scopes cost is the numbering and the spill forecast. Idle reruns, the passes' own work in a run that changed nothing (`LLRM_DEBUG=runs`, `5344082af`, release build): 637 runs, 247.0 M = 7.8% of the -O2 compile (decide 42.3 M, fill 32.8, hoist 24.8, promote 24.8, algebraic 21.8, indvars 21.5), and 318.9 M = 9.9% at -Os (hoist 96.7 M, 3.0%).

## 2026-10-17, `5e2e995ef`: one baseline for the plan (step 0.1)

One table at one main SHA (`5e2e995ef`, `--profile dist` and the same tree with `--features system-alloc`), `perf stat -e instructions:u` min of 3 including the Open Watcom front end child, `d_faces.c` of qbq-llrm with `-m32 -march=i486 -fno-inline-functions` and the Borland keywords defined away. Clang 24 is the static build at `d1106deb71cc`, clang 20.1.8 the static build of the tag, each `-c`. Phases are `LLRM_DEBUG=time` inclusive totals; the in-process front end excludes the Open Watcom front end child (about 18 M). Computes per pass run counts `WHY fn` lines of `LLRM_WHY=1` over the pass runs of `LLRM_DEBUG=runs`; the spec's 2.8 uses its own counter.

| level | llrm M | clang 24 M | ratio | clang 20.1.8 M | ratio | front end (in process) | `mir pipeline` | `assemble` | computes per pass run |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| -O1 | 1,789 | 348 | 5.1x | 397 | 4.5x | 48.4 | 1,094 | 669 | 2121 / 473 = 4.5 |
| -O2 | 2,253 | 413 | 5.5x | 462 | 4.9x | 48.4 | 1,519 | 718 | 2502 / 746 = 3.4 |
| -O3 | 2,497 | 415 | 6.0x | 464 | 5.4x | 48.4 | 1,748 | 747 | 3008 / 1021 = 2.9 |
| -Os | 2,322 | 407 | 5.7x | 459 | 5.1x | 48.4 | 1,560 | 729 | 2227 / 758 = 2.9 |

The -O2 row against the plan's start (`21f5c9fc8`, 2,903 M, 6.8x): 2,253 M, 5.5x clang 24. Rows at this SHA (distro clang 20.1.8 as the clang reference, void as a clang-24 reference and replaced by `rows/main24d` in the plan's section 2.0; 71 vsgcc programs, m32, llrm / min(gcc, clang), `tools/rows/run.sh base-5e2e995ef`; 852 results): clocks per level, geomean, worst program, rows above 1.00:

| level | clocks geomean | worst (program) | rows above 1.00 | code bytes geomean | worst (program) | rows above 1.00 |
|---|---:|---|---:|---:|---|---:|
| -O1 | 0.8875 | 2.01 (`recfloat`) | 27 | 0.9435 | 2.68 (`x_switch_dense`) | 17 |
| -O2 | 0.9044 | 1.80 (`x_dct`) | 31 | 1.0304 | 4.85 (`rectwo`) | 23 |
| -O3 | 0.9021 | 1.91 (`tile`) | 27 | 1.2813 | 8.58 (`queens`) | 36 |
| -Os | 0.9169 | 2.19 (`lru`) | 36 | 0.9471 | 1.77 (`x_switch_ranges`) | 19 |

These are the baseline for the "output never worse" column: a step that changes output is compared with them. m16 rows exist only through `tools/bench` and were not re-run.

Ladder (`tools/ladder/ladder-5e2e995ef.json`, N=25 to 1,600; slopes fit N up to 200, exponents over all N):

| shape | optimizer k/IR LLVM / llrm | backend k/IR LLVM / llrm | optimizer exponent LLVM / llrm | backend exponent LLVM / llrm | N=1600 total M clang / llrm |
|---|---:|---:|---:|---:|---:|
| arith | 46 / 46 | 27 / 37 | 0.95 / 1.10 | 0.82 / 0.92 | 989 / 1,981 |
| mem | 39 / 109 | 16 / 39 | 0.94 / 1.23 | 0.70 / 0.98 | 1,133 / 6,544 |
| loop | 46 / 338 | n/a (LSR cliff) / 201 | 0.82 / 1.40 | n/a / 0.91 | 1,254 / 20,952 |
| diamond | 59 / 134 | 58 / 80 | 0.83 / 1.09 | 0.88 / 1.00 | 795 / 2,067 |
| calls | 45 / 69 | 74 / 108 | 0.76 / 1.05 | 0.83 / 1.00 | 609 / 1,206 |
| float | 24 / 37 | 22 / 20 | 0.84 / 1.02 | 0.92 / 0.95 | 735 / 1,345 |
| mixed | 39 / 178 | 40 / 66 | 0.86 / 1.11 | 0.90 / 1.05 | 2,036 / 9,025 |

Allocations and memory at -O2 (`d_faces`; DHAT on the system-allocator build): 2,979,190 blocks, 601.7 MB allocated, peak live 18.7 MB in 88,000 blocks; by phase: optimizer 1,069,863 blocks (35.9%, 226.7 MB), backend 642,017 (21.6%, 91.1 MB), front end 210,006 (7.0%, 27.2 MB), the rest 1,056,587 (35.5%, 256.7 MB); about 900 per IR instruction against clang's 19. Peak resident set (`/usr/bin/time`): `d_faces` 84 MB with mimalloc and 39 MB with the system allocator; `mem` N=1,600 339 MB and 250 MB.

## 2026-10-21, `0b494eb7ab`: the walk, the -O1 inliner, the pass_ch stage, retrange (w-6)

d_faces `-m32`, release profile, `perf stat -e instructions:u` of `llrm-cc -S` (no `-fno-inline-functions`), one change at a time on `feat/parity`. Row C1: `clip_w_` is inlined as the pin does since #3354 (`visitCondBrInst`; the callee's `br (not X)` guards hid the loop-entry guard, so it was priced and kept as a call): d_faces `-O2` has the pin's 6 defines, and the compile is 1457.7 -> 1580.9 M (+8.5%; `float-facts` 22.5 -> 43.0 M, `gvn@787` +6.5, `fold@677` +5, `regalloc` +7), `-O1`, `-O3` and `-Os` flat. Oversight accepted it on 2026-10-21 (no row above main that was not above it at the base).

| change | -O1 M | -O2 M | -O3 M | -Os M |
|---|---:|---:|---:|---:|
| `feat/parity` 42bec159e | 1561.3 | 1457.7 | 1645.3 | 1280.0 |
| + #3354 `visitCondBrInst` (C1) | 1556.9 | 1573.5 | 1637.6 | 1273.2 |
| + #3358 dead pure calls deleted by InstSimplify | 1239.9 | 1573.5 | 1637.6 | 1273.2 |
| + #3350 no gcc `pass_ch` stage (held, collatz and cordic rows) | 1183.1 | 1501.5 | 1566.0 | 1273.2 |
