# The ledger command

Owner: w-6. A perf PR's compile-cost before/after pair is taken with this command, on dists of the base and the new commit
(`tools/pairs/pair_build.sh`, `CARGO_INCREMENTAL=0`, no `LLRM_*`), and the PR body states the command line and the number:

```bash
valgrind --tool=callgrind --callgrind-out-file=OUT.cg DIST/llrm-cc -m32 -mabi=sysv -march=i486 -O2 \
  -Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near= \
  -I qbq-llrm/src -I qbq-llrm/src/render -I qbq-llrm/src/game -I qbq-llrm/src/host -I qbq-llrm/src/qgl \
  -o REAL_FILE.o qbq-llrm/src/render/d_faces.c
```

The pin side is the same file, flags and `-I` set, with `-c -o` to a real file (encoder and object write included, 553 MC frames in
its profile; not `-S`, not `/dev/null`):

```bash
# tools/pin_clang.py: the clang named by LLRM_PIN_CLANG; prints its --version, refuses any build but d1106deb71cc.
valgrind --tool=callgrind --callgrind-out-file=CLANG.cg "$(tools/pin_clang.py)" -w -m32 -march=i486 -O2 \
  -Dfar= -Dhuge= -Dnear= -Dpascal= -Dcdecl= -D__far= -D_far= -D__near= -D_near= \
  -I qbq-llrm/src -I qbq-llrm/src/render -I qbq-llrm/src/game -I qbq-llrm/src/host -I qbq-llrm/src/qgl \
  -c -o REAL_FILE.o qbq-llrm/src/render/d_faces.c
```

A bare `clang` on PATH is the distro's 20.1.8, a different pass pipeline: its Ir is not the pin's. The pin profile's `Profiled target:` line must name the pin's path.

QCport's pin side adds `-std=gnu89 -DQCPORT_NATIVE_TEST` and QCport's `-I` set. `-w` only silences warnings.

`-o` goes to a real file (`/dev/null` skips the object write: emit reads 0). The number is the `summary:` line of OUT.cg, written
in the body as `d_faces qbq-llrm: BASE -> NEW M Ir`. Other programs (QCport's `render/d_faces.c`, which needs its struct-size
asserts removed at m32; see `qcsrc`) may be added, labelled with the program's name.

The per-bucket table: `tools/ledger/ledger4.py` on `callgrind_annotate --inclusive=yes --threshold=100 OUT.cg` (inputs by `LEDGER_OURS`, `LEDGER_CLANG`, `LEDGER_OT`, `LEDGER_PT`, all four required: no default file is read, positional arguments are refused, `--detail` lists the functions), and
`tools/ledger/ledger5.py` for the callee-attributed view. With `LEDGER_CALLING` (the same profile through `--tree=calling`) it refuses overlapping buckets (a function in two, or one listed function calling another: the Together/Sroa pair counted 9.3 M twice) and prints `double-count check: 0 overlaps over N buckets`. `LEDGER_PIN_CALLING` (the pinned clang's profile through `--tree=calling`, taken with the pin command above) does the same over the pin's bucket lists and prints `pin double-count check: ...`. `tools/landing/precheck.sh` refuses a perf PR whose body lacks the command and the number.
