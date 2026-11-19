; HImode multiply: hl * de -> hl (low 16 bits).
	.module mulhi3
	.globl	___mulhi3
	.area	_CODE
___mulhi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	ld	a,#16
1$:	add	hl,hl
	sla	e
	rl	d
	jr	nc,2$
	add	hl,bc
2$:	dec	a
	jr	nz,1$
	ret
