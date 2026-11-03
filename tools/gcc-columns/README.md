# gcc columns (Z80, 6502)

Scripts and results for the gcc column of the vs rows. The compiler is a local fork of gcc master with
spike Z80 and MOS 6502 backends; the fork is GPLv3 and stays outside this repository. Only the scripts, the
hand-written runtime helpers and the measured tables are here.

    gcc_z80_bench.py  --cc1 PATH   # 30 kernels x O1/O2/Os, ucsim_z80; --tsv writes matrix rows
    gcc_6502_bench.py --cc1 PATH [--cpu 65c02]   # 30 C kernels + 51 hand-corpus kernels x O1/O2/Os, sim65
    vs_6502.py MATRIX_DIR OUT.md DATE [65c02]   # MATRIX_DIR holds 6502.tsv (the other columns) and 6502-gcc.tsv

`PATH` is the fork's `cc1` (`make all-gcc`, target `z80-unknown-elf` or `mos6502-unknown-elf`). Methods are
the benches' own: cycles per call from a 1- and 101-call run, bytes from the linked code. Every cell is
checked against the hand-derived result; a cell that fails or links foreign library code is `n/a`.
Instruction counts need a sim65 that prints them (cc65 V2.19 source plus a counter; `--sim`); the 6502 cells have them. `tables/` holds the cells with the cc1 md5 in the header.

Result convention: gcc returns in rc2:rc3 (Z80: HL), the other columns in A:X (Z80: DE). gcc cells are charged the
caller's move (6502 +2 B/3 clk/1 insn, 16-bit +4/6/2; Z80 16-bit +1 B/4 T/1 insn). Cells that call hand helpers or carry a
table are noted "hand helper, not codegen" / "table, not codegen" and are not compiler wins.

Softfloat: `tables/softfloat-6502.tsv` (hand asm and gcc cells) and `tables/gcc-softfloat-vs-hand-6502-*.md` compare `runtime/softfloat` C through gcc with `src/6502/*.s`. Produced by scratch scripts outside the repo.
Z80 softfloat: `tables/softfloat-z80.tsv` and `tables/gcc-softfloat-vs-hand-z80-*.md`; the hand-written SImode helpers in `rt-z80/` (`__ashlsi3`, `__lshrsi3`, `__ashrsi3`, `__mulsi3`, `__umodhi3`) serve the gcc side.
