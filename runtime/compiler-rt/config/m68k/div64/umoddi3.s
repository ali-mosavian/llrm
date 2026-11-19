| Unsigned 64-bit remainder for the 68020/030/040; the 68000 and 68060 keep lib/int/umoddi3.c. Assemble with -I config/m68k/div64.
.ifdef M68020
.ifndef M68060
	.set	WANT_R,1
	.include "core.inc"
.endif
.endif
