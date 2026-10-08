# Compile-time scaling, main aa93a260 (tinytim, instructions:u, median of 3, 6 axes run 6 at a time)

Slopes are over the sizes llrm reached; "top 4" is the largest four. "Total" ratios are whole-process instructions; "net" subtract each compiler's empty-file cost. Pass tables: steps of llrm -O2 above exponent 1.15 (own ms, wall, so approximate). `mulconst` is N multiplies by large odd constants (#951: isel's multiply search); `straight` and `branches` use small multipliers. Regenerate: `scaling.py run --programs && scaling.py report`.

### functions

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.18 / 1.39 | 0.93 / 1.01 | 0.97 / 1.00 | 0.98 / 1.03 | 3.06x @ 2^4 | 13.60x @ 2^11 | 13.64x | 8 / 12 |
| -O1 | 1.21 / 1.38 | 0.98 / 1.01 | 1.09 / 1.18 | 1.00 / 1.03 | 4.61x @ 2^4 | 14.22x @ 2^10 | 14.25x | 7 / 10 |
| -O2 | 1.20 / 1.37 | 1.00 / 1.03 | 1.14 / 1.26 | 1.06 / 1.24 | 2.73x @ 2^4 | 6.60x @ 2^10 | 6.61x | 7 / 9 |
| -O3 | 1.20 / 1.36 | 1.05 / 1.09 | 1.14 / 1.26 | 1.07 / 1.13 | 2.23x @ 2^4 | 4.42x @ 2^10 | 4.42x | 7 / 8 |
| -Os | 1.20 / 1.36 | 0.98 / 1.03 | 1.09 / 1.17 | 1.03 / 1.15 | 4.42x @ 2^4 | 12.34x @ 2^10 | 12.36x | 7 / 9 |

Passes of llrm -O2 above exponent 1.15 on functions (top 4 sizes, own ms; largest N = 1024):

| step | ms @ largest | slope |
|---|---|---|
| mir interprocedural | 6835.8 | 2.13 |
| isel | 4236.1 | 1.66 |
| analysis call-effects | 3388.6 | 1.72 |
| analysis summaries | 1186.3 | 1.59 |
| summaries points-to | 667.2 | 1.29 |
| analysis points-to | 566.7 | 1.17 |
| mir verify after pass | 334.9 | 1.17 |
| regalloc base | 295.0 | 1.57 |
| frontend translate | 280.1 | 1.19 |
| mir pipeline | 272.0 | 1.18 |

### straight

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.47 / 1.85 | 0.88 / 1.14 | 0.88 / 0.98 | 1.06 / 1.59 | 1.77x @ 2^4 | 57.92x @ 2^12 | 58.11x | 9 / 12 |
| -O1 | 1.18 / 1.57 | 0.95 / 1.08 | 1.05 / 1.19 | 0.97 / 1.15 | 1.73x @ 2^4 | 9.72x @ 2^13 | 9.72x | 10 / 11 |
| -O2 | 1.12 / 1.40 | 0.95 / 1.02 | 1.02 / 1.10 | 0.99 / 1.12 | 1.50x @ 2^4 | 4.56x @ 2^12 | 4.57x | 9 / 11 |
| -O3 | 1.16 / 1.56 | 0.97 / 1.06 | 1.04 / 1.18 | 0.99 / 1.12 | 1.65x @ 2^4 | 7.01x @ 2^13 | 7.01x | 10 / 11 |
| -Os | 1.16 / 1.53 | 0.93 / 1.01 | 1.04 / 1.18 | 0.95 / 1.05 | 1.73x @ 2^4 | 9.78x @ 2^13 | 9.79x | 10 / 12 |

Passes of llrm -O2 above exponent 1.15 on straight (top 4 sizes, own ms; largest N = 4096):

| step | ms @ largest | slope |
|---|---|---|
| lir coalesce | 5492.6 | 2.36 |
| regalloc base | 3484.5 | 2.60 |
| lir twoaddr | 2962.3 | 2.21 |
| mir gvn | 1560.4 | 1.65 |
| lir peephole | 1251.7 | 1.29 |
| lir verify | 543.0 | 1.40 |
| analysis published | 463.0 | 2.03 |
| hir to mir | 424.7 | 1.87 |
| frontend translate | 365.9 | 1.51 |
| regalloc prepare | 221.8 | 1.78 |

### branches

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.72 / 1.90 | 0.81 / 0.93 | 0.90 / 0.97 | 0.92 / 1.02 | 3.89x @ 2^4 | 210.25x @ 2^10 | 212.86x | 7 / 12 |
| -O1 | 1.76 / 1.83 | 0.88 / 0.92 | 1.00 / 1.02 | 1.06 / 1.47 | 10.19x @ 2^4 | 137.86x @ 2^8 | 139.96x | 5 / 11 |
| -O2 | 1.75 / 1.82 | 0.92 / 0.95 | 1.01 / 1.02 | 1.02 / 1.17 | 7.98x @ 2^4 | 88.55x @ 2^8 | 89.38x | 5 / 10 |
| -O3 | 1.73 / 1.82 | 0.92 / 0.95 | 1.01 / 1.02 | 1.01 / 1.15 | 8.25x @ 2^4 | 86.12x @ 2^8 | 86.90x | 5 / 10 |
| -Os | 1.74 / 1.84 | 0.90 / 0.94 | 1.01 / 1.02 | 1.02 / 1.19 | 8.34x @ 2^4 | 94.23x @ 2^8 | 95.31x | 5 / 10 |

Passes of llrm -O2 above exponent 1.15 on branches (top 4 sizes, own ms; largest N = 256):

| step | ms @ largest | slope |
|---|---|---|
| regalloc recolor | 8227.4 | 1.76 |
| siblings interference | 925.6 | 2.10 |
| lir peephole | 791.3 | 1.30 |
| intervals walk | 771.5 | 1.87 |
| mir gvn | 530.8 | 1.49 |
| split carving | 450.9 | 1.81 |
| regalloc spill | 428.7 | 1.71 |
| regalloc candidates | 421.9 | 3.05 |
| analysis registers | 358.7 | 1.78 |
| siblings widths | 334.4 | 1.98 |

### live

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.49 / 1.88 | 0.92 / 1.27 | 0.85 / 0.97 | 1.18 / 1.77 | 2.95x @ 2^4 | 53.94x @ 2^11 | 54.19x | 8 / 11 |
| -O1 | 1.53 / 1.76 | 1.07 / 1.29 | 1.02 / 1.27 | 1.42 / 1.87 | 6.19x @ 2^4 | 37.26x @ 2^9 | 37.51x | 6 / 10 |
| -O2 | 1.53 / 1.76 | 1.09 / 1.30 | 1.02 / 1.28 | 1.42 / 1.83 | 5.68x @ 2^4 | 29.68x @ 2^9 | 29.83x | 6 / 10 |
| -O3 | 1.52 / 1.76 | 1.11 / 1.34 | 1.01 / 1.19 | 1.38 / 1.78 | 5.60x @ 2^4 | 26.71x @ 2^9 | 26.83x | 6 / 9 |
| -Os | 1.55 / 1.84 | 1.14 / 1.46 | 1.20 / 1.40 | 1.38 / 1.80 | 5.68x @ 2^4 | 36.25x @ 2^10 | 36.33x | 7 / 10 |

Passes of llrm -O2 above exponent 1.15 on live (top 4 sizes, own ms; largest N = 512):

| step | ms @ largest | slope |
|---|---|---|
| spill cleanup | 2572.7 | 2.11 |
| regalloc spill | 1533.3 | 1.83 |
| siblings interference | 1523.1 | 2.30 |
| spill color slots | 1204.3 | 2.18 |
| mir gvn | 1071.5 | 1.80 |
| spill rewrite | 1070.5 | 2.06 |
| facts widths | 805.5 | 2.05 |
| facts sibling prices | 791.9 | 1.86 |
| classes scan | 776.4 | 2.29 |
| intervals walk | 755.9 | 1.82 |

### callers

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.14 / 1.38 | 0.88 / 0.98 | 0.93 / 1.00 | 0.94 / 1.02 | 3.63x @ 2^4 | 17.79x @ 2^11 | 17.85x | 8 / 12 |
| -O1 | 1.28 / 1.59 | 0.99 / 1.05 | 0.98 / 1.00 | 1.03 / 1.19 | 3.40x @ 2^4 | 16.27x @ 2^11 | 16.28x | 8 / 10 |
| -O2 | 1.74 / 1.89 | 0.95 / 0.98 | 0.96 / 0.98 | 0.98 / 1.00 | 4.95x @ 2^4 | 81.00x @ 2^9 | 81.30x | 6 / 10 |
| -O3 | 1.74 / 1.89 | 0.95 / 0.98 | 0.96 / 0.98 | 0.98 / 1.00 | 4.99x @ 2^4 | 82.27x @ 2^9 | 82.57x | 6 / 10 |
| -Os | 2.17 / 2.17 | 0.89 / 0.89 | 0.94 / 0.94 | 0.97 / 1.00 | 15.81x @ 2^4 | 250.96x @ 2^7 | 256.06x | 4 / 11 |

Passes of llrm -O2 above exponent 1.15 on callers (top 4 sizes, own ms; largest N = 512):

| step | ms @ largest | slope |
|---|---|---|
| mir gvn | 5741.6 | 1.65 |
| analysis call-effects | 5315.9 | 2.14 |
| mir interprocedural | 4829.3 | 1.61 |
| analysis through-memory | 4504.3 | 1.71 |
| analysis summaries | 3691.2 | 1.88 |
| mir decide | 3579.9 | 1.92 |
| summaries points-to | 3463.7 | 1.47 |
| analysis globals-aa | 3197.3 | 1.93 |
| summaries direct | 2748.2 | 1.76 |
| analysis points-to | 2720.1 | 1.89 |

### chain

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.10 / 1.31 | 0.87 / 0.98 | 0.96 / 1.00 | 0.92 / 1.00 | 2.99x @ 2^4 | 16.52x @ 2^12 | 16.57x | 9 / 12 |
| -O1 | 2.06 / 2.11 | 0.77 / 0.83 | 1.82 / 1.89 | 0.94 / 1.00 | 9.56x @ 2^4 | 427.27x @ 2^8 | 444.20x | 5 / 12 |
| -O2 | 2.05 / 2.09 | 0.80 / 0.86 | 1.84 / 1.90 | 0.94 / 1.00 | 9.04x @ 2^4 | 349.79x @ 2^8 | 360.76x | 5 / 12 |
| -O3 | 2.05 / 2.09 | 0.80 / 0.86 | 1.84 / 1.90 | 0.94 / 1.00 | 9.05x @ 2^4 | 344.35x @ 2^8 | 354.95x | 5 / 12 |
| -Os | 2.05 / 2.09 | 0.80 / 0.86 | 1.84 / 1.90 | 0.95 / 1.00 | 9.51x @ 2^4 | 372.52x @ 2^8 | 384.96x | 5 / 12 |

Passes of llrm -O2 above exponent 1.15 on chain (top 4 sizes, own ms; largest N = 256):

| step | ms @ largest | slope |
|---|---|---|
| analysis globals-aa | 14824.5 | 2.81 |
| summaries points-to | 12611.4 | 2.16 |
| summaries direct | 11641.9 | 2.34 |
| analysis summaries | 5455.4 | 2.35 |
| mir interprocedural | 4734.9 | 2.19 |
| analysis call-registers | 1997.9 | 2.09 |
| mir gvn | 1258.6 | 1.56 |
| analysis callee-effects | 633.3 | 1.61 |
| regalloc spill | 580.2 | 1.68 |
| mir decide | 492.0 | 1.45 |

### mulconst

| level | llrm slope (all / top 4 sizes) | gcc, same N | clang, same N | gcc, its own range | llrm/gcc total, smallest N | llrm/gcc total, largest N | llrm/gcc net of empty file, largest N | sizes llrm / gcc |
|---|---|---|---|---|---|---|---|---|
| -O0 | 1.18 / 1.36 | 1.02 / 1.22 | 0.78 / 0.95 | 1.13 / 1.51 | 8.70x @ 2^4 | 26.24x @ 2^11 | 26.29x | 8 / 10 |
| -O1 | 1.06 / 1.14 | 0.98 / 1.01 | 0.97 / 1.06 | 1.00 / 1.08 | 5.46x @ 2^4 | 9.87x @ 2^11 | 9.88x | 8 / 11 |
| -O2 | 1.06 / 1.14 | 0.98 / 1.01 | 0.97 / 1.06 | 1.01 / 1.09 | 4.82x @ 2^4 | 8.37x @ 2^11 | 8.38x | 8 / 11 |
| -O3 | 1.06 / 1.14 | 0.98 / 1.01 | 0.97 / 1.06 | 1.01 / 1.09 | 4.89x @ 2^4 | 8.36x @ 2^11 | 8.37x | 8 / 11 |
| -Os | 1.01 / 1.05 | 0.85 / 1.03 | 0.99 / 1.12 | 0.97 / 1.30 | 7.02x @ 2^4 | 24.74x @ 2^12 | 24.83x | 9 / 12 |

Passes of llrm -O2 above exponent 1.15 on mulconst (top 4 sizes, own ms; largest N = 2048):

| step | ms @ largest | slope |
|---|---|---|
| lir twoaddr | 1678.4 | 2.08 |
| lir coalesce | 1370.0 | 1.89 |
| regalloc base | 874.2 | 1.90 |
| analysis published | 112.5 | 1.88 |
| analysis globals-aa | 68.3 | 1.30 |
| analysis registers | 34.1 | 1.17 |
| lir loopslots | 27.9 | 1.15 |
| hir to mir | 27.7 | 1.27 |
| mir sroa | 16.5 | 1.60 |
| lir owned bytes | 14.6 | 1.56 |

### QCport: 65 modules, cost against llrm's MIR instructions (instructions:u; slopes and "net" ratios are less the empty file's cost: gcc 18.7 M, clang 41 M, llrm 8.1 M)

| level | llrm slope (net) | gcc | clang | llrm/gcc total, geomean | llrm/gcc net of empty file, geomean | worst total llrm/gcc | modules |
|---|---|---|---|---|---|---|---|
| -O0 | 1.13 | 0.51 | 0.64 | 6.25x | 7.30x | 159.42x render/d_faces | 59 |
| -O1 | 1.32 | 0.67 | 0.86 | 12.75x | 14.19x | 600.09x render/d_faces | 62 |
| -O2 | 1.33 | 0.77 | 0.88 | 9.75x | 10.56x | 368.32x render/d_faces | 62 |
| -O3 | 1.34 | 0.85 | 0.91 | 8.17x | 8.79x | 335.49x render/d_faces | 62 |
| -Os | 1.32 | 0.73 | 0.85 | 10.99x | 12.01x | 427.04x render/d_faces | 61 |

Modules a compiler failed on (levels, reason):

- host/dbg: llrm O0,O1,O2,O3,Os: front end E1060 Invalid type (dos.h _FAR)
- host/keybind: llrm O0: peephole: value read but never defined
- render/d_alias: llrm Os: peephole: value read but never defined
- render/sb_build: llrm O0,O1,O2,O3,Os: peephole: value read but never defined
- render/sc: llrm O0: peephole: value read but never defined
- model/mdl: llrm O0: peephole: value read but never defined
- game/mdl_ai: llrm O0,O1,O2,O3,Os: front end E1060 Invalid type (dos.h _FAR)

Stubbed so gcc and clang can read QCport (llrm reads the same stubbed text):

- 16-bit struct-size asserts (`typedef char rec_*_ok[..]`; the sizes are 16-bit ones): 12 files
- inline assembly (`__asm { .. }`, `_asm ..`), which gcc and clang cannot read: 4 files
- ^Z end-of-file byte in Borland headers (gcc: stray \32): 4 files
- Borland names declared for gcc/clang only (`-include prelude.h`): _fmemcpy, _fmemset, _fmemmove, _fmemcmp, _fstricmp, _fstrncmp, stricmp, FP_OFF, FP_SEG, __emit__; flags -Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -Wno-implicit-function-declaration -w (-Dfar etc. for gcc/clang only; llrm-c has no -D)

Largest modules, llrm -O2 steps by own ms:

- host/host (9616 MIR instructions, 46 functions): analysis summaries 7638, analysis call-effects 4798, mir gvn 3241, analysis through-memory 3201, analysis float-facts 1816, spill color slots 1757
- game/weapons (8360 MIR instructions, 37 functions): lir peephole 536, mir gvn 434, intervals walk 265, regalloc spill 238, split placed 210, spill color slots 203
- render/d_alias (6937 MIR instructions, 24 functions): regalloc recolor 1374, lir peephole 879, intervals walk 816, split placed 797, regalloc spill 728, spill color slots 703
- render/sc (5144 MIR instructions, 48 functions): lir peephole 763, mir decide 524, mir gvn 355, regalloc trial 161, analysis through-memory 157, summaries direct 143
- game/ent (4973 MIR instructions, 35 functions): mir gvn 192, lir peephole 189, analysis call-effects 94, regalloc recolor 86, analysis through-memory 66, isel 61

