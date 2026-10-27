# DAG refusals by corpus, 2026-10-25

Parity `121f6f17e`, release binaries, `--isel=dag` (strict) with `LLRM_DEBUG=isel` and `LLRM_ISEL_REPORT`. A failed compile, a `refused:` row or a function selected by `old` counts. Driver: `ROOT=$PWD LLRM_BIN=... python3 sweep` over `tools/dosbatch/run_tests.py discover` (compile only, no DOSBox).

| corpus | target | compiled | functions | refused or old | failed compiles |
|---|---|---|---|---|---|
| tests/run c, alias, huge, nib, qb, examples, bench (1040 builds) | m16, m32 | 1040 | 2686 | 0 | 0 |
| QCport 65 modules, -Os and -O2 | m16 | 65 | 701 (Os) / 574 (O2) | 0 | 0 |
| QCport, -Os | m32 | 15 of 65 | 115 | 0 | 50 (frontend: `E1020 Dimension cannot be 0`, missing prototypes; the same under `--isel=old`) |
| tests/llc C programs (27) | m64 | 26 | - | 0 | 1: `vla`, `DynamicStackAlloc` has no lowering (#4666/#4678) |

No corpus program reaches a refusal at m16 or m32; `qb/bounds.bas`, the one reached string of the earlier inventory, is gone. No frontend produces a dynamic `alloca` at m16 or m32 (llrm-c has no VLA and no `__builtin_alloca`), so m32 `vla` is IR-only.

Against `refusal-inventory.md` (187 sites): reached 0 at m16/m32, 1 at m64 (`DynamicStackAlloc`); the rest stay unreached.

Not covered: runtime output (the DOSBox groups), `-O0`/`-O1`, QCport at m32 beyond the 15 modules that compile, m64 outside tests/llc.
