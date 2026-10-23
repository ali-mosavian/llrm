# The ledger command

Owner: w-6. A perf PR's compile-cost before/after pair is taken with this command, on dists of the base and the new commit
(`tools/pairs/pair_build.sh`, `CARGO_INCREMENTAL=0`, no `LLRM_*`), and the PR body states the command line and the number:

```bash
valgrind --tool=callgrind --callgrind-out-file=OUT.cg DIST/llrm-c -m32 -mabi=sysv -march=i486 -O2 \
  -Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near= \
  -I qbq-llrm/src -I qbq-llrm/src/render -I qbq-llrm/src/game -I qbq-llrm/src/host -I qbq-llrm/src/qgl \
  -o REAL_FILE.o qbq-llrm/src/render/d_faces.c
```

`-o` goes to a real file (`/dev/null` skips the object write: emit reads 0). The number is the `summary:` line of OUT.cg, written
in the body as `d_faces qbq-llrm: BASE -> NEW M Ir`. Other programs (QCport's `render/d_faces.c`, which needs its struct-size
asserts removed at m32; see `qcsrc`) may be added, labelled with the program's name.

The per-bucket table: `ledger4.py` on `callgrind_annotate --inclusive=yes --threshold=100 OUT.cg`, and
`ledger5.py` for the callee-attributed view. `tools/landing/precheck.sh` refuses a perf PR whose body lacks the command and the number.
