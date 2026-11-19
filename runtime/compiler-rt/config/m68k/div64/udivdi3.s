| Unsigned 64-bit divide for the 68020/030/040; the 68000 and 68060 keep lib/int/udivdi3.c. Assemble with -I config/m68k/div64.
.ifdef M68020
.ifndef M68060
	.include "core.inc"
.endif
.endif
