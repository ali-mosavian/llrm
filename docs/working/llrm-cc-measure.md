# Measuring llrm-cc: compile time and size

Owner: w10. Date: 2026-10-27. Frontend: llrm-cc (clang), not llrm-c. These numbers never mix with llrm-c's (`tools/compile-cost.py`, `tools/sizes.py`, `docs/working/emit-baselines.md`).

## Method

```sh
export LLRM_OW=~/scratch/ow-src LLRM_PIN_CLANG=~/scratch/llvm-build/bin/clang LLRM_CLANG_M16=~/scratch/llvm-build-m16b/bin/clang
tools/cc_cost.py --dist <dir with llrm-cc> [--programs jpeg_fixed,matmul] [--configs m32-O2,m32-Os,m16-Os]
```

- **Time**: user-space instructions of the whole `llrm-cc -c` process tree (clang front end and `llrm-llc` included), `perf stat -e instructions:u`. Instructions, not seconds: load does not move them.
- **Pin**: the same count for `clang -cc1 -triple i486-unknown-linux-gnu -target-cpu i486 -O2 -fno-builtin -internal-isystem <pin>/lib/clang/24/include ... -emit-obj` of the pinned clang (d1106deb71cc). Without `-internal-isystem` the compile fails at `stddef.h` and reads a bogus 20 M. m32 only; the m16 pin is a separate clang build, with no column here.
- **Size**: `.text` bytes of the two ELF objects (`size -A`). m16 has no size column (OMF).
- **Per pass**: `LLRM_DEBUG=time LLRM_TIME_TOP=1000 llrm-cc ...` prints `[instr]` rows by own work; `[instr] total` is the llrm part. For a pin per-pass split use `valgrind --tool=callgrind` and `callgrind_annotate --inclusive=yes --threshold=100` (at the default threshold small frames are cut off and look absent).
- A failed compile stops the run: a count of a failed compile is a few million, not a measurement.

## Baseline: feat/parity b5400efa9e (2026-10-27)

| program | config | ours Minstr | pin Minstr | ratio | ours B | pin B |
|---|---|--:|--:|--:|--:|--:|
| jpeg_fixed | m32-O2 | 1431.8 | 671.6 | 2.13 | 10421 | 9787 |
| jpeg_fixed | m32-Os | 805.2 | 377.7 | 2.13 | 3847 | 3762 |
| jpeg_fixed | m16-Os | 792.4 | – | – | – | – |
| stft_fixed | m32-O2 | 197.5 | 100.3 | 1.97 | 1153 | 1252 |
| stft_fixed | m32-Os | 195.5 | 99.2 | 1.97 | 1009 | 1143 |
| stft_fixed | m16-Os | 206.7 | – | – | – | – |
| nbody | m32-O2 | 110.8 | 58.6 | 1.89 | 701 | 779 |
| nbody | m32-Os | 103.2 | 55.7 | 1.85 | 435 | 459 |
| nbody | m16-Os | 106.1 | – | – | – | – |
| matmul | m32-O2 | 1236.0 | 176.5 | **7.00** | 2326 | 2155 |
| matmul | m32-Os | 120.4 | 58.3 | 2.07 | 366 | 301 |
| matmul | m16-Os | 116.2 | – | – | – | – |
| crc | m32-O2 | 102.7 | 40.9 | 2.51 | 379 | 243 |
| crc | m32-Os | 60.1 | 28.0 | 2.15 | 114 | 145 |
| crc | m16-Os | 55.5 | – | – | – | – |

The ratio is about 2x everywhere except matmul -O2: `[instr]` rows say `mir dse` 831 and `mir gvn` 184 of 1217 Minstr, after unroll makes the body straight-line. Raw output: `~/scratch/w10/cc_cost_baseline.txt`.
