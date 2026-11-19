; HImode multiply, early exit: hl * de -> hl (low 16 bits); runs once per set bit of de's width.
	.module mulhi3
	.globl	___mulhi3
	.area	_CODE
___mulhi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	jr	3$
1$:	add	hl,bc
2$:	sla	c
	rl	b
3$:	srl	d
	rr	e
	jr	c,1$
	ld	a,d
	or	a,e
	jr	nz,2$
	ret
