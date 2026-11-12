# 6502 bytes are linked image bytes, in every column

Read from `crates/target/llrm-6502/tests/bench.rs`, `bench/{llvmmos,sdcc}.rs` and `tools/6502/ours_cr.py`, 2026-11-12.

Kernel table (`bench`): bytes = linked image with the kernel minus the image with `fn: rts`, plus 1. Library members the kernel calls are in. A cell prints `N B linked, M B body`; `rank.py` reads N.

| column | build | link command |
|---|---|---|
| ours | llrm-cc 6502-sim65 `.s` + reachable `mos_rt.s` routines | cl65-style ca65/ld65 link; body = CODE segment alone (`od65 --dump-segsize`) |
| cc65 | `cl65 -t sim6502 -O` | `cl65 -t sim6502 -o p.bin d.s f.s` |
| sdcc | `sdcc -mmos6502 -c k.c` | `sdcc -mmos6502 d.rel k.rel -o p.ihx`, bytes = sum of the ihx data records (SDCC library linked) |
| llvm-mos | `mos-sim-clang -Os\|-O2 -fno-lto -c k.c` | `mos-sim-clang -Os d.c k.o -fno-lto -o p.bin` (libc linked) |

Corpus (`ours_cr.py`): CODE+RODATA+DATA of the program object plus the runtime members the map file shows linked; not the driver or startup.

Not linked image: the `bench` rows of the multi-target program set (`bench/*/`, e.g. `jpeg_single`), which report the function's code section. #9457's EXP-6502 row is of that kind: jpeg_single Os 7646 to 7760 is code only; linked it is 8019 to 8058 (+39 B, `__mulhisi3` library 75 B).
