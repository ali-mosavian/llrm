# Measuring llrm-cc: compile time and size

Owner: w10. Date: 2026-10-27. Frontend: llrm-cc (clang), not llrm-c. These numbers never mix with llrm-c's (`tools/compile-cost.py`, `tools/sizes.py`, `docs/working/emit-baselines.md`).

## Method

```sh
export LLRM_OW=~/scratch/ow-src LLRM_PIN_CLANG=~/scratch/llvm-build/bin/clang LLRM_CLANG_M16=~/scratch/llvm-build-m16b/bin/clang
tools/cc_cost.py --dist <dir with llrm-cc> [--programs jpeg_fixed,matmul] [--configs m32-O2,m32-Os,m16-Os]
```

- **Time**: user-space instructions of the whole `llrm-cc -c` process tree (clang front end and `llrm-llc` included), `perf stat -e instructions:u`. Instructions, not seconds: load does not move them.
- **Pin time**: the same count for `clang -cc1 -triple i486-unknown-linux-gnu -target-cpu i486 -O2 -fno-builtin -internal-isystem <pin>/lib/clang/24/include -I <posix> -I <src dir> -emit-obj -o x.o x.c` of the pinned clang (d1106deb71cc): the compile a user runs. Without `-internal-isystem` it fails at `stddef.h` and reads a bogus 20 M. m32 only; the m16 pin is a separate clang build, with no column here.
- **Size**: `.text` bytes (`size -A`, alignment padding included) of our ELF object and of the **pin on our IR**: `clang -S -emit-llvm -Xclang -disable-llvm-passes -Xclang -disable-O0-optnone` (the front end llrm-cc runs, datalayout line rewritten to `targets.toml`'s) -> `opt -passes='default<O2>'` -> `llc -O2 -mtriple=i486-unknown-linux-gnu -mcpu=i486 -filetype=obj`. Do not compare against the bytes of `clang -cc1 -O2`: it optimises the C itself (inlines, folds) from other IR. m16 has no size column (OMF).
- The two pin columns are two runs; the same-IR chain's instruction count is not used (IR printed and parsed twice adds ~300 M on jpeg_fixed).
- **Per pass**: `LLRM_DEBUG=time LLRM_TIME_TOP=1000 llrm-cc ...` prints `[instr]` rows by own work; `[instr] total` is the llrm part. For a pin per-pass split use `valgrind --tool=callgrind` and `callgrind_annotate --inclusive=yes --threshold=100` (at the default threshold small frames are cut off and look absent).
- A failed compile stops the run: a count of a failed compile is a few million, not a measurement.

## Baseline: feat/parity b5400efa9e (2026-10-27)

| program | config | ours Minstr | pin Minstr | ratio | ours B | pin B (same IR) |
|---|---|--:|--:|--:|--:|--:|
| jpeg_fixed | m32-O2 | 1431.8 | 671.1 | 2.13 | 10421 | 13133 |
| jpeg_fixed | m32-Os | 805.2 | 377.2 | 2.13 | 3847 | 3694 |
| jpeg_fixed | m16-Os | 792.5 | – | – | – | – |
| stft_fixed | m32-O2 | 197.5 | 100.3 | 1.97 | 1153 | 1830 |
| stft_fixed | m32-Os | 195.5 | 99.3 | 1.97 | 1009 | 1012 |
| stft_fixed | m16-Os | 206.7 | – | – | – | – |
| nbody | m32-O2 | 110.7 | 58.6 | 1.89 | 701 | 541 |
| nbody | m32-Os | 103.2 | 55.7 | 1.85 | 435 | 430 |
| nbody | m16-Os | 106.1 | – | – | – | – |
| matmul | m32-O2 | 1236.0 | 176.5 | **7.00** | 2326 | 1741 |
| matmul | m32-Os | 120.4 | 58.2 | 2.07 | 366 | 520 |
| matmul | m16-Os | 116.2 | – | – | – | – |
| crc | m32-O2 | 102.7 | 41.0 | 2.51 | 379 | 213 |
| crc | m32-Os | 60.1 | 28.0 | 2.14 | 114 | 117 |
| crc | m16-Os | 55.5 | – | – | – | – |

- Time is about 2x everywhere except matmul -O2: `mir dse` 831 and `mir gvn` 184 of 1217 Minstr, after unroll makes the body straight-line.
- crc -O2 379 vs 213 B is real, not the instrument: `bench_crc` is 177 B on both sides (62 instructions, w12), but the pin's `main` is 21 B (it folds the call of the pure function on constants) and ours is 187 B (inlined, unrolled, not folded).
- An earlier version of this table compared against `clang -cc1 -O2` bytes (crc 243, jpeg "+6.5%"); those numbers came from different IR and are withdrawn.
- Raw output: `~/scratch/w10/cc_cost_baseline3.txt`.
