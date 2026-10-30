# AM-QC-1: QCport -O2 size, cause per module (w-6, parity 69b60cafc)

Numbers: emitted OMF bytes (LEDATA) of the 65 QCport modules, the measure of `tools/rows/qcport.py`, one `llrm-c` dist of 69b60cafc, flags added to `-O2` one at a time.

| build | bytes | vs -O2 |
|---|--:|--:|
| -O2 | 321,138 | |
| -O2 -fno-unroll-loops | 302,358 | -18,780 |
| -O2 -fno-peel-loops | 321,138 | 0 |
| -O2 -fno-inline-functions | 288,743 | -32,395 |
| -O2 -fno-inline-functions-called-once | 321,138 | 0 |
| -O1 | 303,359 | -17,779 |
| -Os | 262,455 | -58,683 |

Full unrolling is all of what -O2 adds over -O1 (+17.8 KB; switching it off gives 302.4 KB, within 1 KB of -O1). Inlining is 32.4 KB of the -O2 total. Peeling and called-once inlining change nothing.

The four largest-growth modules (bytes; "no unroll/peel", "no inline" are `-O2` with `-fno-unroll-loops -fno-peel-loops` and with `-fno-inline-functions -fno-inline-functions-called-once`):

| module | -O1 | -O2 | -Os | -O2 no unroll/peel | -O2 no inline |
|---|--:|--:|--:|--:|--:|
| sbar | 6,313 | 9,225 | 3,787 | 6,337 | 4,699 |
| cmd | 5,098 | 8,773 | 3,686 | 4,863 | 3,842 |
| mdl_ai | 27,136 | 28,789 | 22,170 | 27,227 | 25,011 |
| sc | 14,020 | 14,763 | 10,888 | 13,880 | 11,732 |

Cause: sbar +2,888 and cmd +3,910 of their -O2 are loop unrolling; inlining is 4.5 KB (sbar), 4.9 KB (cmd), 3.8 KB (mdl_ai) and 3.0 KB (sc). The comparison with Borland (296,085) is outside `qcport.py`: -O1 and no-unroll -O2 land at 302-303 KB, +2% of it.
Steps (w-6, 2026-11-06): the unroll threshold at -O2 against the pin's (`LoopFullUnroll` at OptLevel 2) on the m16 cost model; inlining against the pin's -O2 threshold on the same modules.

## New baseline after #4327 (D-I off by default), w-6, 2026-10-24
`tools/rows/qcport.py`, defaults only, emitted OMF bytes (LEDATA) of the 65 modules, 520 compiles each side, 0 failed; base c3a880d9ec (parent of #4327), main 847141867 (#4327).

| level | before | after | delta | |
|---|--:|--:|--:|--:|
| -O1 | 303,236 | 304,415 | +1,179 | +0.389% |
| -O2 | 321,359 | 322,571 | +1,212 | +0.377% |
| -O3 | 354,000 | 355,183 | +1,183 | +0.334% |
| -Os | 262,295 | 263,383 | +1,088 | +0.415% |

The expectation was +0.33..+0.42%: met at every level. The baseline for later QCport rows is the "after" column (-O2 322,571; the shipped -Os 263,383). The figures in the sections above were taken at 69b60cafc (-O2 321,138).

## Closed: the -O2 growth is the pin's own (w-6, 2026-10-25, ruling by the coordinator)
Evidence. Our full-unroll constants are the pin's (`UNROLL_THRESHOLD_DEFAULT` 150, `_AGGRESSIVE` 300, boost 400, `loopunrollpass.rs`), and the x86 TTI legalizes types by native width (`tti.rs`, `legalized`), so an i32 on m16 is priced as split. The pin's own -O2 grows the same modules: text bytes of the pinned clang (d1106deb71cc, `-std=gnu89 -m32 -march=i486`, the QCport `-I` set and `-Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near= -DQCPORT_NATIVE_TEST`, `-c`, then `size`):

| module | -O1 | -O2 | -Os | -O2 over -O1 |
|---|--:|--:|--:|--:|
| host/cmd.c | 7,243 | 10,492 | 5,930 | +45% |
| render/sc.c | 18,227 | 21,188 | 17,212 | +16% |

Ours, `qcport.py` bytes at m16: cmd 5,098 / 8,773 / 3,686 (+72%), sc 14,020 / 14,763 / 10,888 (+5%).

-O2 demo2 and demo3 out of memory (miptex.bin, attract mode) = memory, not a defect: QCport ships at -Os (263,383 B after #4327), where demo1-3 play. No size-aware -O2 deviation (it would depart from the pin for a level nobody ships).
