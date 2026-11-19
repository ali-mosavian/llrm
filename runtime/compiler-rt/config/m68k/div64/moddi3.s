| Signed 64-bit remainder (sign of the dividend) for the 68020/030/040 over the unsigned routine; the 68000 and 68060 keep lib/int/moddi3.c.
.ifdef M68020
.ifndef M68060
	.text
	.globl	__moddi3
__moddi3:
	lea	4(%sp),%a1
	move.l	(%a1),%d0
	or.l	8(%a1),%d0
	jbmi	.Lslow
	jmp	__sf_umod_a1		| both nonnegative: the unsigned result
.Lslow:
	move.l	(%a1),-(%sp)		| the remainder takes the dividend's sign
	tst.l	(%a1)
	jbpl	1f
	neg.l	4(%a1)
	negx.l	(%a1)
1:	tst.l	8(%a1)
	jbpl	2f
	neg.l	12(%a1)
	negx.l	8(%a1)
2:
	jbsr	__sf_umod_a1
	tst.l	(%sp)+
	jbpl	3f
	neg.l	%d1
	negx.l	%d0
3:	rts
.endif
.endif
