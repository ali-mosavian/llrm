| 64-bit multiply (low 64 bits of x*y) for the 68020/030/040: three `mulu.l` (the 32x32 -> 64 form for the low halves);
| the 68060 (no 64-bit-result mulu.l) keeps lib/int/muldi3.c; the 68000 and 68010 (16x16 mulu.w only) take the second block.
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
.ifndef M68020
	.text
	.globl	__muldi3
__muldi3:				| x = 16(sp):20(sp), y = 24(sp):28(sp) after the push; words big-endian
	movem.l	%d2-%d4,-(%sp)
	move.w	18(%sp),%d0		| cross terms: low 32 bits of xh*yl + xl*yh
	mulu.w	30(%sp),%d0
	move.w	22(%sp),%d2
	mulu.w	26(%sp),%d2
	add.l	%d2,%d0
	move.w	16(%sp),%d1
	mulu.w	30(%sp),%d1
	move.w	18(%sp),%d2
	mulu.w	28(%sp),%d2
	add.l	%d2,%d1
	move.w	20(%sp),%d2
	mulu.w	26(%sp),%d2
	add.l	%d2,%d1
	move.w	22(%sp),%d2
	mulu.w	24(%sp),%d2
	add.l	%d2,%d1
	swap	%d1
	clr.w	%d1
	add.l	%d1,%d0			| d0 = cross terms
	move.w	22(%sp),%d1		| xl*yl: P0 = Al*Bl
	mulu.w	30(%sp),%d1
	move.w	20(%sp),%d2		| P1 = Ah*Bl
	mulu.w	30(%sp),%d2
	move.w	22(%sp),%d3		| P2 = Al*Bh
	mulu.w	28(%sp),%d3
	moveq	#0,%d4
	add.l	%d3,%d2			| P1 + P2, the carry out is bit 32 of the sum
	addx.l	%d4,%d4
	swap	%d2
	moveq	#0,%d3
	move.w	%d2,%d3			| (P1 + P2) >> 16, low part
	clr.w	%d2			| (P1 + P2) << 16
	add.l	%d2,%d1			| lo = P0 + (P1 + P2) << 16
	moveq	#0,%d2
	addx.l	%d2,%d3			| the carry of lo
	move.w	20(%sp),%d2		| P3 = Ah*Bh
	mulu.w	28(%sp),%d2
	add.l	%d2,%d3
	swap	%d4			| the carry of P1 + P2, at bit 16 of hi
	add.l	%d4,%d3
	add.l	%d0,%d3
	move.l	%d3,%d0
	movem.l	(%sp)+,%d2-%d4
	rts
.endif
