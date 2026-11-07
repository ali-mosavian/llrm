| 64-bit multiply (low 64 bits of x*y) for the 68020/030/040: three `mulu.l` (the 32x32 -> 64 form for the low halves);
| the 68000 and 68060 (no 64-bit-result mulu.l) keep lib/int/muldi3.c.
.ifdef M68020
.ifndef M68060
	.text
	.globl	__muldi3
__muldi3:				| x = 8(sp):12(sp), y = 16(sp):20(sp) after the push
	move.l	%d2,-(%sp)
	move.l	8(%sp),%d0		| x.hi * y.lo
	mulu.l	20(%sp),%d0
	move.l	16(%sp),%d1		| y.hi * x.lo
	mulu.l	12(%sp),%d1
	add.l	%d1,%d0
	move.l	12(%sp),%d1		| x.lo * y.lo as d2:d1
	mulu.l	20(%sp),%d2:%d1
	add.l	%d2,%d0
	move.l	(%sp)+,%d2
	rts
.endif
.endif
