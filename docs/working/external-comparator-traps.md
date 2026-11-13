# External columns that are not comparisons

- `tests/run/c/variadic.c` and `long_double_variadic.c` pick their code by `__clang__`: ours and clang build stdarg, gcc builds a stack-walk hack. x86_64 -Os read 662 B ours against gcc 219 B (2026-11-12). Never quote a gcc column for them.
- `tools/ab/comparable.py` lists such programs; `test_comparable.py` fails when a third appears.
- The other 142 `tests/run/c` programs build with gcc and clang and print what ours prints at x86_64 -O1 (138 for both, 6 differ for gcc).
- `-fno-inline-functions` means different things: clang takes it as no inlining but `always_inline`, gcc still inlines what shrinks at -Os (a static called once). 11 `tests/run/c` programs carry it; their gcc column must build with `-fno-inline`. x86_64 -Os with `-fno-inline` against `-fno-inline-functions` (2026-11-13): div_constants_ulong gcc 3537 not 3303, div_constants_long 5726 not 5446, frameless 616 not 376, mul_constants 2659 not 2180, old_style_extra_args 452 not 285; ours beats each (3438, 5544, 449, 2069, 221).
