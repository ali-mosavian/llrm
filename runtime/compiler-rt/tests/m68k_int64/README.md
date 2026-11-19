uint64 ops compiled by the target compiler vs Python (3000 fixed-seed pairs): shl shr add sub mul mulw div mod sticky lt. Every count must be 0.

Files: `ops64.c` is the code under test (one function per op, names `sf_o_*`); `gen.py > v.h` writes the expected values; `main.c` calls each op and prints `name=count`.

m68k run (needs `m68k-run-c.sh` exporting labels that start with `sf`; the stock one exports all labels and clashes on larger corpora):
`python3 gen.py > v.h; CFLAGS_EXTRA="-I. -I../../include" sh m68k-run-c.sh 68020 Os main.c ops64.c ../../lib/int/int64.c ../../lib/int/int32.c`

Port to another target: compile `ops64.c` (and `../../lib/int/int64.c`, `int32.c` if the target lowers 64-bit mul/div to `__muldi3` etc.) with the target compiler; in `main.c` replace only `sys3` (the m68k `trap #0` write/exit shim: d0=4 write(fd=1, buf, len), d0=1 exit(code)) and `_start` with the target's output and exit. `main` itself is plain C. Pass `-O2` and `-Os`; run both CPUs/modes you ship.

Known (2026-11-02, llrm-cc ctlz_ac62374): m68k 68000/68020, Os/O1/O2: add=836, sub=1440 of 3000, all else 0 (carry zero-extended without masking to 1). -O0 does not build int64.c (`@__muldi3` argument attribute).
