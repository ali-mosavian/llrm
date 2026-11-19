| __mulsi3: the low 32 bits of a*b. Hand written from the 16-bit partial products; no libgcc source is copied.
| 68000/68010 (mulu.w only): operands under 64 K are one product, others three. 68020 and later: one mulu.l.
| Arguments on the stack (a at 4(sp), b at 8(sp)), result in d0; d1 is the only other register touched.
| Assemble with --defsym M68020=1 for the 68020 and later. The cost does not depend on the order of the operands.
	.text
	.globl	__mulsi3
__mulsi3:
.ifdef M68020
	move.l	4(%sp),%d0
	mulu.l	8(%sp),%d0
	rts
.else
	move.w	4(%sp),%d0		| a.hi | b.hi == 0: both under 64 K, one product
	or.w	8(%sp),%d0
	bne.s	.Lwide
	move.w	6(%sp),%d0
	mulu.w	10(%sp),%d0
	rts
.Lwide:
	move.w	4(%sp),%d0		| a.hi * b.lo + b.hi * a.lo, shifted up a word
	mulu.w	10(%sp),%d0
	move.w	8(%sp),%d1
	mulu.w	6(%sp),%d1
	add.w	%d1,%d0
	swap	%d0
	clr.w	%d0
	move.w	6(%sp),%d1		| + a.lo * b.lo
	mulu.w	10(%sp),%d1
	add.l	%d1,%d0
	rts
.endif
