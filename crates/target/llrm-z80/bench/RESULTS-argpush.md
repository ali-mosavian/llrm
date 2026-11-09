# Z80-ARGPUSH cells, 2026-11-09

Stack-argument stores become one `push` per word (`expand::push_arguments`, after `X86CallFrameOptimization`), the stores chained top word first in `dagisel/call.rs`.
Base = release/next `bda0acc507`; new = `dcd167243f`. Each side's soft-float library is built from its own compiler (`runtime/compiler-rt/tests/z80_lib.sh`, -Os).
Instrument: `z80-abf.py` copy (ucsim_z80 T-states, bytes = linked .ihx minus harness plus runtime members reached); SDCC 4.5.0 best of its four variants per axis, same harness. SDCC does not take the level, so its bar is one per cell.

**Correctness.** A row exists only when the program's report equals its `.out`. Those 11 cells are in `applies` for z80 and ran correct on both compilers: long_double_arguments, x87_arguments, over_aligned_local_after_even_pushes, tail_calls, int64_division, data_int64, float_in_memory_loop, float_in_place_update, int_compared_with_float, recdeep, recmany_o1.
`os_locals_between_pushes` and `shrink_wrapped_saves` are `-m32`-forced (32-bit int `.out`), dropped from z80 by `applies`: their rows are static bytes / T only, not correctness.
Wider sweep (`run.py`, `applies`-kept corpus, 116 programs): O2 and Os, base and new both 109 PASS with identical non-PASS lists.

**Not flipped by this change** (cause is not the argument stores): os_locals_between_pushes (O1/O2 +61 B: twelve volatile loads are held in spill slots and reloaded to push; Os/Oz T: `__s_umodsi3` in the fill loop), int64_division and data_int64 T at Os/Oz (division runtime), the three float cells (our f64 against SDCC's f32).

| cell | level | base B / T | new B / T | best SDCC B / T | new vs SDCC (B, T) |
|---|---|---|---|---|---|
| long_double_arguments | Os | 10943 / 238225 | 10376 / 234163 | 3167 / 87501 | +7209, +146662 |
| os_locals_between_pushes (static) | Os | 1011 / 131893 | 938 / 131113 | 1201 / 80170 | -263, +50943 |
| shrink_wrapped_saves (static) | Os | 31 / 494 | 31 / 494 | 1230 / 208575 | -1199, -208081 |
| x87_arguments | Os | 10909 / 1687103 | 10264 / 1645279 | none | - |
| over_aligned_local_after_even_pushes | Os | 2274 / 486579 | 2168 / 486015 | none | - |
| tail_calls | Os | 1426 / 76353 | 1426 / 76353 | 1584 / 171994 | -158, -95641 |
| int64_division | Os | 2141 / 13707966 | 2027 / 13677642 | 3453 / 9068024 | -1426, +4609618 |
| data_int64 | Os | 557 / 105794 | 521 / 105566 | 982 / 70515 | -461, +35051 |
| float_in_memory_loop | Os | 4724 / 37834683 | 4608 / 36851083 | 1754 / 9499571 | +2854, +27351512 |
| float_in_place_update | Os | 8436 / 187230 | 8217 / 184480 | 2559 / 82305 | +5658, +102175 |
| int_compared_with_float | Os | 5275 / 42676 | 4937 / 40655 | none | - |
| recdeep | Os | 657 / 31627098 | 657 / 31627098 | none | - |
| recmany_o1 | Os | 663 / 31456217 | 663 / 31456217 | none | - |
| long_double_arguments | O2 | 10943 / 238225 | 10376 / 234163 | 3167 / 87501 | +7209, +146662 |
| os_locals_between_pushes (static) | O2 | 1335 / 46389 | 1262 / 45609 | 1201 / 80170 | +61, -34561 |
| shrink_wrapped_saves (static) | O2 | 31 / 494 | 31 / 494 | 1230 / 208575 | -1199, -208081 |
| x87_arguments | O2 | 10909 / 1687103 | 10264 / 1645279 | none | - |
| over_aligned_local_after_even_pushes | O2 | 2140 / 460101 | 2034 / 459537 | none | - |
| tail_calls | O2 | 1439 / 64125 | 1439 / 64125 | 1584 / 171994 | -145, -107869 |
| int64_division | O2 | 2445 / 6618686 | 2328 / 6589846 | 3453 / 9068024 | -1125, -2478178 |
| data_int64 | O2 | 837 / 19288 | 801 / 19060 | 982 / 70515 | -181, -51455 |
| float_in_memory_loop | O2 | 4993 / 37832358 | 4841 / 36853958 | 1754 / 9499571 | +3087, +27354387 |
| float_in_place_update | O2 | 13 / 216 | 13 / 216 | 2559 / 82305 | -2546, -82089 |
| int_compared_with_float | O2 | 5275 / 42676 | 4937 / 40655 | none | - |
| recdeep | O2 | 844 / 18081688 | 844 / 18081688 | none | - |
| recmany_o1 | O2 | 850 / 17910807 | 850 / 17910807 | none | - |
| long_double_arguments | O1 | 10943 / 238225 | 10376 / 234163 | 3167 / 87501 | +7209, +146662 |
| os_locals_between_pushes (static) | O1 | 1335 / 46389 | 1262 / 45609 | 1201 / 80170 | +61, -34561 |
| shrink_wrapped_saves (static) | O1 | 1101 / 20901 | 1101 / 20901 | 1230 / 208575 | -129, -187674 |
| x87_arguments | O1 | 11279 / 1683013 | 10573 / 1648150 | none | - |
| over_aligned_local_after_even_pushes | O1 | 2510 / 490963 | 2404 / 490399 | none | - |
| tail_calls | O1 | 1490 / 71260 | 1490 / 71260 | 1584 / 171994 | -94, -100734 |
| int64_division | O1 | 2601 / 6620478 | 2480 / 6591582 | 3453 / 9068024 | -973, -2476442 |
| data_int64 | O1 | 837 / 19288 | 801 / 19060 | 982 / 70515 | -181, -51455 |
| float_in_memory_loop | O1 | 4724 / 37834683 | 4609 / 36856483 | 1754 / 9499571 | +2855, +27356912 |
| float_in_place_update | O1 | 8434 / 187184 | 8217 / 184442 | 2559 / 82305 | +5658, +102137 |
| int_compared_with_float | O1 | 5275 / 42676 | 4937 / 40655 | none | - |
| recdeep | O1 | 991 / 18647865 | 991 / 18647865 | none | - |
| recmany_o1 | O1 | 1029 / 18647865 | 1029 / 18647865 | none | - |
| long_double_arguments | Oz | 10943 / 238225 | 10376 / 234163 | 3167 / 87501 | +7209, +146662 |
| os_locals_between_pushes (static) | Oz | 1019 / 132001 | 946 / 131221 | 1201 / 80170 | -255, +51051 |
| shrink_wrapped_saves (static) | Oz | 538 / 9320 | 538 / 9320 | 1230 / 208575 | -692, -199255 |
| x87_arguments | Oz | 10863 / 1685300 | 10239 / 1646939 | none | - |
| over_aligned_local_after_even_pushes | Oz | 2275 / 489723 | 2169 / 489159 | none | - |
| tail_calls | Oz | 1399 / 78411 | 1399 / 78411 | 1584 / 171994 | -185, -93583 |
| int64_division | Oz | 2165 / 13711292 | 2051 / 13680968 | 3453 / 9068024 | -1402, +4612944 |
| data_int64 | Oz | 557 / 105794 | 521 / 105566 | 982 / 70515 | -461, +35051 |
| float_in_memory_loop | Oz | 4720 / 37848701 | 4583 / 36863601 | 1754 / 9499571 | +2829, +27364030 |
| float_in_place_update | Oz | 8450 / 187821 | 8240 / 185090 | 2559 / 82305 | +5681, +102785 |
| int_compared_with_float | Oz | 5275 / 42676 | 4937 / 40655 | none | - |
| recdeep | Oz | 740 / 32246810 | 740 / 32246810 | none | - |
| recmany_o1 | Oz | 750 / 32329350 | 750 / 32329350 | none | - |

WORSE than base on an axis: none

## Z80-ARGPUSH-SPILL (spilled stack words stored in place)

Same harness, base = release/next `62ed3db6e0`, new = + `spilled_args.rs`. Only one cell moves (static counts, -m32-forced): the other 12 are identical at Os/O2/O1/Oz.

| cell | level | before B / T | after B / T | best SDCC B / T |
|---|---|---|---|---|
| os_locals_between_pushes (static) | Os | 938 / 131113 | 891 / 130373 | 1201 / 80170 |
| os_locals_between_pushes (static) | O2, O1 | 1262 / 45609 | 1215 / 44869 | 1201 / 80170 |
| os_locals_between_pushes (static) | Oz | 946 / 131221 | 899 / 130481 | 1201 / 80170 |
