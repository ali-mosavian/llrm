# m68k ISA table

Owner: llrm-target-m68k

The opcode table of the 68000 family (68000, 68010, 68020, 68030, 68040, 68060):
`crates/target/llrm-m68k/src/isa/table.tsv`, read by `llrm_m68k::isa`.
One row per mnemonic and operand shape: the cubes of first words and command
words, the FPU formats, the CPUs that decode it, the CPUs that trap it to
software.

## How it is checked

- Lines 0-E equal binutils' disassembler (`m68k-linux-gnu-objdump -m m68k:680x0`)
  for every first word, six extension words, each CPU.
- Line F, FPU and MMU (cpid 0 and 1): every command word, 68020 to 68060.
- The Motorola reference manual's printed patterns (`isa/prm/*.tsv`,
  transcribed from M68000PRM by a second reader) contain every word the table
  decodes, and each of its patterns reaches a decoded word.
- `tests/isa.rs` fails without `GCC_M68K` unless `M68K_ISA_SKIP_ORACLE=1`.

## Limits

1. binutils decodes the FPU and MMU under `-m68000`. Those CPUs have no
   coprocessor interface (line F is an exception), so the table starts them at
   the 68020.
2. binutils has no mnemonics for coprocessors 2 to 7. The generic `cp*` rows
   are the manual's patterns, on the 68020 and 68030.
3. The MC68851 is a separate chip: its forms are the 68020's rows.
4. Extension words of lines 0-E are not modelled; operand length follows from
   the mode bits. Forms that depend on one (`chk2`, 64-bit `mul`/`div`) are
   listed per first word, not per command word.
5. `emulated` is per form: the 68040 from Table 9-10 of its manual (FLOG2 is
   missing there; the reference manual names it as software), the 68060 from
   Tables 6-11, C-3 and D-1. The 68060's dynamic `fmovem` list, the 64-bit
   `mul`/`div` forms and a misaligned `cas` also trap; the table rows cover both
   widths and cannot say so.
6. The manual prints BFFFO with BFEXTU's first word (4-43); the table follows
   binutils (`edc0`).

## Sources

| Manual | sha256 |
|---|---|
| M68000PRM | 06e4864b78da0e815054cead9326b7ec9914661f240fd39a455f2061ff47c4e8 |
| MC68000UM (68000, 68010) | 89b690b1923f8a3cff508567090bcab0bcd07511c2deff8ffa393a08efda18e1 |
| M68020UM | 883cae47d3741c686f1eb347577a77a968d1d946246c38ecd43cf695c0c85857 |
| MC68030UM | 07a676828a5e476b96f24a8b963fd06b79b5860dac35166a182205ecbb9c7f95 |
| MC68040UM | aed5a038586dd2f610068efa01919fc552dc65bfd1a9274a333bad29dbd48894 |
| M68060UM (scan) | 646837005a0e4ada5ebae6c10ab1e669268ea8947251da82761163f006bdd13c |
