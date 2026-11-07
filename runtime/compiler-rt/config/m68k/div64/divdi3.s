| Signed 64-bit divide (truncating; d == 0 gives ~0 as lib/int/divdi3.c) for the 68020/030/040 over the unsigned routine; the 68000 and 68060 keep lib/int/divdi3.c.
.ifdef M68020
.ifndef M68060
	.text
	.globl	__divdi3
__divdi3:
	lea	4(%sp),%a1
	move.l	(%a1),%d0
	or.l	8(%a1),%d0
	jbmi	.Lslow
	jmp	__sf_udiv_a1		| both nonnegative: the unsigned result
.Lslow:
	clr.l	-(%sp)			| flag: -1 if exactly one operand is negative
	tst.l	(%a1)
	jbpl	1f
	neg.l	4(%a1)
	negx.l	(%a1)
	not.l	(%sp)
1:	tst.l	8(%a1)
	jbpl	2f
	neg.l	12(%a1)
	negx.l	8(%a1)
	not.l	(%sp)
2:
	jbsr	__sf_udiv_a1
	tst.l	(%sp)+
	jbpl	3f
	tst.l	8(%a1)			| d == 0: keep ~0
	jbne	4f
	tst.l	12(%a1)
	jbeq	3f
4:	neg.l	%d1
	negx.l	%d0
3:	rts
.endif
.endif
