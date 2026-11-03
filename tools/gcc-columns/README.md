# gcc columns (Z80, 6502)

Scripts and results for the gcc column of the vs rows. The compiler is a local fork of gcc master with
spike Z80 and MOS 6502 backends; the fork is GPLv3 and stays outside this repository. Only the scripts, the
hand-written runtime helpers and the measured tables are here.

    gcc_z80_bench.py  --cc1 PATH   # 23 kernels x O1/O2/Os, ucsim_z80; --tsv writes matrix rows
    gcc_6502_bench.py --cc1 PATH   # 30 C kernels + 51 hand-corpus kernels x O1/O2/Os, sim65
    vs_6502.py MATRIX_DIR OUT.md DATE

`PATH` is the fork's `cc1` (`make all-gcc`, target `z80-unknown-elf` or `mos6502-unknown-elf`). Methods are
the benches' own: cycles per call from a 1- and 101-call run, bytes from the linked code. Every cell is
checked against the hand-derived result; a cell that fails or links foreign library code is `n/a`.
Instruction counts need a sim65 that prints them (cc65 V2.19 source plus a counter; `--sim`); the 6502 cells have them. `tables/` holds the cells with the cc1 md5 in the header.
