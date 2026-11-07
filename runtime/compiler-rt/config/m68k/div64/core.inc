| Unsigned 64-bit divide (__udivdi3) or remainder (WANT_R: __umoddi3) for the 68020/030/040, using divu.l 64/32.
| Hand written, after Knuth's algorithm D for a two-word divisor; no libgcc source is copied.
| Arguments on the stack (n hi, n lo, d hi, d lo), result in d0:d1. The second label takes a1 pointing at the arguments (divdi3.s and moddi3.s
| negate them in place and call it). A one-word divisor touches d0, d1, a0 and a1 only; the general case saves d2-d7.
| d == 0 gives q = ~0, r = n, as lib/int/udivmod_w.c. The 68000 and 68060 (no 64/32 divu.l) keep the C.
	.text
.ifdef WANT_R
	.globl	__umoddi3
__umoddi3:
.else
	.globl	__udivdi3
__udivdi3:
.endif
	lea	4(%sp),%a1
.ifdef WANT_R
	.globl	__sf_umod_a1
__sf_umod_a1:
.else
	.globl	__sf_udiv_a1
__sf_udiv_a1:
.endif
	tst.l	8(%a1)
	jbne	.Lwide
	move.l	(%a1),%d0
	tst.l	12(%a1)
	jbeq	.Lzero
	cmp.l	12(%a1),%d0
	jbcc	.Ltwo
	move.l	4(%a1),%d1
	divu.l	12(%a1),%d0:%d1		| n.hi < d: one divide, d0 = r, d1 = q
.ifdef WANT_R
	move.l	%d0,%d1
.endif
	moveq	#0,%d0
	rts
.Ltwo:	divul.l	12(%a1),%d1:%d0		| d0 = q.hi = n.hi / d, d1 = n.hi % d
.ifndef WANT_R
	move.l	%d0,%a0
.endif
	move.l	4(%a1),%d0
	divu.l	12(%a1),%d1:%d0		| d0 = q.lo, d1 = r
.ifdef WANT_R
	moveq	#0,%d0
.else
	move.l	%d0,%d1
	move.l	%a0,%d0
.endif
	rts
.Lzero:
.ifdef WANT_R
	move.l	4(%a1),%d1		| r = n
.else
	moveq	#-1,%d0
	moveq	#-1,%d1
.endif
	rts
| d.hi != 0, so q < 2^32: n < d is decided by the high words.
.Lwide:	move.l	(%a1),%d0
	cmp.l	8(%a1),%d0
	jbcc	.Lw1
.ifdef WANT_R
	move.l	4(%a1),%d1		| r = n
.else
	moveq	#0,%d0
	moveq	#0,%d1
.endif
	rts
| Normalise d to d1':d0' = d << s (top bit set) and n to n2:n1':n0' = n << s (counts of 32 shift everything out). q1 = (n2:n1') / d1'
| is q or q + 1 (the n2 < d1' bound keeps the divide from overflowing); the product q1 * d0' against r1:n0' (r1 the remainder) corrects it.
.Lw1:	movem.l	%d2-%d7,-(%sp)
	movem.l	(%a1),%d0-%d3		| n, d
	bfffo	%d2{#0:#0},%d4		| s
	moveq	#32,%d5
	sub.l	%d4,%d5			| 32 - s
	move.l	%d0,%d6
	lsr.l	%d5,%d6			| n2
	lsl.l	%d4,%d0
	move.l	%d1,%d7
	lsr.l	%d5,%d7
	or.l	%d7,%d0			| n1'
	lsl.l	%d4,%d1			| n0'
	lsl.l	%d4,%d2
	move.l	%d3,%d7
	lsr.l	%d5,%d7
	or.l	%d7,%d2			| d1'
	lsl.l	%d4,%d3			| d0'
	divu.l	%d2,%d6:%d0		| d0 = q1, d6 = r1
	move.l	%d0,%d7
	mulu.l	%d3,%d5:%d7		| m = q1 * d0' = d5:d7
	sub.l	%d7,%d1
	subx.l	%d5,%d6			| (r1:n0') - m, borrow if m is larger
	jbcc	.Lnoc
	subq.l	#1,%d0
.ifdef WANT_R
	add.l	%d3,%d1
	addx.l	%d2,%d6
.endif
.Lnoc:
.ifdef WANT_R
	moveq	#32,%d5
	sub.l	%d4,%d5
	lsr.l	%d4,%d1			| r = (d6:d1) >> s
	move.l	%d6,%d3
	lsl.l	%d5,%d3
	or.l	%d3,%d1
	lsr.l	%d4,%d6
	move.l	%d6,%d0
.else
	move.l	%d0,%d1
	moveq	#0,%d0
.endif
	movem.l	(%sp)+,%d2-%d7
	rts
