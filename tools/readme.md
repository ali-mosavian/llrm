# Tools

Run the Python ones with `uv run --project tools python tools/<dir>/<tool>.py`.

| Tool | Does |
|---|---|
| `nib-build.sh` | builds a Nib program into a DOS `.EXE` |
| `baseline.sh OUT` | every stage dump of the OMF corpus and Nib examples; `diff -r` two of them |
| `e2e/e2e.py` | BC compiles `tests/suite`, `llrm-omf` rewrites, LINK links, DOSBox runs, output compared |
| `e2e/matrix.py` | `e2e` over all twelve BC configurations |
| `e2e/fuzzgen.py`, `e2e/fuzzcheck.py` | random BASIC programs, judged by an evaluator, BC and `llrm-omf` |
| `e2e/mkgolden.py` | the suite's expected outputs, computed from what each program means |
| `e2e/mkfixtures.py` | rebuilds `tests/fixtures/omf` with the BC toolchains |
| `e2e/dosbox.py`, `e2e/cache.py`, `e2e/configs.py` | the DOSBox runner, its launch cache, the BC switch sets |
| `analysis/qbfootprint.py` | linked code bytes per module from two LINK maps |
| `analysis/runtime_writes.py` | what the QuickBASIC runtime writes, read off a linked image |

`toolchain/` holds what `build.rs` bootstraps: Open Watcom's `wccq`, jwasm,
jwlink and DOSBox-X.
