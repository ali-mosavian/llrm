# Compile costs measured to an object

Owner: w-8. Date: 2026-10-23.

llrm-c writes an object only for an `.o` or `.obj` output; `-o /dev/null` gets the assembly text, so a cost measured there leaves out the encoder and the
object write, which clang's `-c` includes. The measuring tools now compile to an object (`llrmbin.with_object`, a path of fixed length).

| Tool | Was | Now |
| --- | --- | --- |
| `compile-cost.py`, `measure.py` (via `levels_time.counted`), `scaling_gate.py` | `-o /dev/null` | object; `measure.method()` hashes `llrmbin.py`, so stored bases are measured again |
| `pass-profile.py`, `pipeline-runs.py`, `scoreboard.py` | `-o /dev/null` | object |
| `vsgcc/ctime.py`, `test_scaling_gate.py` | `-o /dev/null` | object |

Old and new, user instructions, dist of aa715557c (Ir, `-O2`):

| Program | Old | New | |
| --- | --- | --- | --- |
| sieve m32 | 62,133,010 | 62,342,567 | +0.34% |
| queens m32 | 72,007,789 | 72,285,298 | +0.39% |
| fib m32 | 63,439,190 | 63,661,351 | +0.35% |
| bintree m32 | 139,479,065 | 139,872,081 | +0.28% |
| mandel m32 | 67,118,385 | 67,353,390 | +0.35% |
| quicksort m32 | 231,325,783 | 231,724,506 | +0.17% |
| QCport render/d_faces m16 | 896,360,941 | 898,002,382 | +0.18% |

Still `/dev/null`, due 2026-11-09: `abmeasure.py`, `pgo/pgo.py`, `test_alloc_scaling.py`, `test_budgets.py` (`tools/test_emit_blind.py` lists them).
