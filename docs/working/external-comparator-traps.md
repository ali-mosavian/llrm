# External columns that are not comparisons

- `tests/run/c/variadic.c` and `long_double_variadic.c` pick their code by `__clang__`: ours and clang build stdarg, gcc builds a stack-walk hack. x86_64 -Os read 662 B ours against gcc 219 B (2026-11-12). Never quote a gcc column for them.
- `tools/ab/comparable.py` lists such programs; `test_comparable.py` fails when a third appears.
- The other 142 `tests/run/c` programs build with gcc and clang and print what ours prints at x86_64 -O1 (138 for both, 6 differ for gcc).
