# Worse cell: unroll keeps one ScalarEvolution (#9543)

- known_loads_walks.c, x86_64 -O3: .text 3879 -> 3895 B (+16), 155978 -> 155995 instructions (+17). -O2 equal in bytes.
- Cause: the next remainder guard reuses an earlier loop's `n-1`; the value stays live and spills.
- Owner w8. Exit 2026-11-20.
