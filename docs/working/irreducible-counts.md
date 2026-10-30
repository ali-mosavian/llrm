# Irreducible cycles in the corpora (BRIDGE-1)

Owner: cycles-queue-quiet-host. 2026-10-23. Parity 7d12750b3f54, release build with a trace on the three places the compile asks for irreducible cycles (`executed`, `machinesink`, the MIR pipeline's promote check; patch irr-trace.patch, not committed). A function counts once per compile.

| corpus | compiles | functions with an irreducible cycle |
|---|--:|--:|
| QCport 65 modules, m16, -O1/-O2/-O3/-Os | 260 | 0 |
| d_faces m32 -O2 | 1 | 0 |
| vsgcc kernels m32 sysv, -O1/-Os | 142 | 0 |
| vsgcc kernels m32 sysv, -O2/-O3 | 142 | 1 per level: `_bench_x_switch` |
| QB bench bq32, -O1/-O2/-Os | 90 | 0 |
| Nib bench, -O1/-O2/-Os | 90 | 0 |

Control: x_switch (the one case) is found at the MIR pipeline and at the LIR stage, so the trace is live; the zeros are not a dead counter. Consequence: the irreducible-SCC approximation of `BlockFrequencyInfoImpl` (limited support, `.h:650`) is reached by one function of the corpora at two levels; its frequency check exemption (`Frequency::irreducible_cycles`) covers it. Not examined: tests/run, -O0, m64, the origin of the idle-bridge idea and the pin's table for irreducible SCCs (owed).
