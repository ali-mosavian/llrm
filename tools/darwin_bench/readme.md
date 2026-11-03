# darwin_bench

Head-versus-base bench of `llrm-cc` against the pin, on the Mac: kernel-only bytes, static instructions and in-process
µs per run, per `bench/` program and level.

    LLRM_PIN_CLANG=<pin clang> tools/darwin_bench/rerun.sh HEAD_SHA BASE_SHA [O1 O2 O3 Os]

- Builds each sha once into `$W/bin/<sha12>/` (`$W` defaults to `../llrm-bench`). The source tree
  `$W/src-<sha12>` stays: `llrm-cc` reads its data files from the path it was built in. The target dir (~4 GB) goes.
- Refuses above load 45, and when the two `llrm-cc` binaries are byte-identical.
- Output `$W/results/<head12>-<base12>.<level>.txt`; the header holds both shas, both md5s and the load.
- Kernel bytes are every code section minus the harness's `orig_main` and `report` (`codebytes.kernel`).

Tests: `rerun_test.sh` (a failed build stops the run), `rerun_test2.sh` (the built `llrm-cc` still runs afterwards),
`test_codebytes.py` (the harness is not counted in a kernel).

Cause-card instruments (static, deterministic; the µs columns are noisy while the Mac runs other work):

- `novec_all.sh HEAD12` runs `novec.py`: pin and gcc-13 with and without vectorization next to llrm-cc, 29 arm64 programs,
  bytes / insns / SIMD-arithmetic insns / µs; the header holds the compiled-cell count and the compilers' md5.
- `sameir.py LEVEL prog...`: the pin's `-fno-vectorize` IR through `llrm-arm64-llc`. llc(pinIR) against the pin is the
  backend gap; llrm-cc against llc(pinIR) is the mid-end gap.

Host shim (DOS-flavoured programs): `tools/hostshim/host.py`; flags and input md5s are printed in each novec result header.
