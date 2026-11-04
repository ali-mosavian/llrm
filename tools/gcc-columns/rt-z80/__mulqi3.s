; QImode multiply: a * e -> a (low 8 bits).
	.module mulqi3
	.globl	___mulqi3
	.area	_CODE
___mulqi3:
	ld	c,a
	xor	a
	ld	b,#8
1$:	add	a,a
	rl	e
	jr	nc,2$
	add	a,c
2$:	djnz	1$
	ret
