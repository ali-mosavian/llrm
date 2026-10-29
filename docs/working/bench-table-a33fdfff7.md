# Bench table vs min(gcc, clang), tip a33fdfff7 (not the pin-llc table)

TL;DR: at -O2 the dag selector is 1.143x old on clocks (1.063 vs 0.930 of min(gcc, clang)) and 0.957x on code bytes (1.128 vs 1.179). -Os code: dag 1.007, old 0.956.

Method: tip a33fdfff7ab1088d71225b2e502a9f9abcd3652e (includes #4780); llrm-cc md5 7a42e6825e865ea80e03894e5d042de8, dist built by `tools/pairs/pair_build.sh` at a33fdfff7ab1088d71225b2e502a9f9abcd3652e; its llrm-cc.d names only `wt-pair-a33fdfff7ab1088d71225b2e502a9f9abcd3652e` sources (1190 refs, no other sha); `--isel=old` / `--isel=dag` per column; 71 programs, m32; gcc 13.4 and pinned clang d1106deb71cc rebuilt by vsgcc build.sh at stack alignment 4; clocks from the vsgcc harness price table (486); no LLRM_* env. All 71 programs ok in both columns; no failed or missing rows.

Geomean of llrm / max(1, min(gcc, clang)) per program; below 1.0 beats the best reference.

| level | counter | old | dag | dag/old | worst dag rows |
|---|---|---|---|---|---|
| -O1 | clocks | 0.876 | 0.983 | 1.122 | x_iir 2.20, x_hash 1.66, x_mergesort 1.65 |
| -O1 | code bytes | 1.050 | 1.010 | 0.962 | matmul 1.33, nbody_single 1.30, fpbench 1.28 |
| -O2 | clocks | 0.930 | 1.063 | 1.143 | rectwo 3.32, x_iir 2.20, fib 1.62 |
| -O2 | code bytes | 1.179 | 1.128 | 0.957 | matmul 5.43, fpbench 3.35, x_nbody_f 3.18 |
| -O3 | clocks | 0.961 | 1.104 | 1.149 | rectwo 3.32, x_hanoi2 2.22, x_iir 2.20 |
| -O3 | code bytes | 1.233 | 1.182 | 0.959 | recchop 22.32, shellsort 4.64, queens 3.00 |
| -Os | clocks | 0.926 | 0.918 | 0.991 | x_ll_div 1.50, x_insertion 1.26, recpow 1.24 |
| -Os | code bytes | 0.956 | 1.007 | 1.054 | recpow 1.31, switch 1.26, x_strlen 1.26 |

Reference-bound rows (clocks, -O2; ratio is against the faster of gcc/clang):

| prog | old | dag | gcc | clang |
|---|---|---|---|---|
| rectwo | 1798940 | 1847560 | 555823 | 1823245 |
| x_iir | 55530 | 132025 | 60034 | 130533 |
| fib | 340921 | 340921 | 210636 | 362808 |

rectwo and fib: old equals dag; gcc's recursion rewrite is the gap, not selection. x_iir: dag equals clang (130533), old equals gcc (60034); the 2.20 is gcc beating clang 2.2x. Against the pin llc (w12) x_iir is faithful, so the dag-vs-old gap here is the pin's own gap to gcc, not a flip defect.
