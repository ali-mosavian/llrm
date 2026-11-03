; Unsigned 16-bit remainder: hl % de -> hl (the restoring loop of __umodhi3, remainder kept).
	.module umodhi3
	.globl	___umodhi3
	.area	_CODE
___umodhi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	ld	a,#16
1$:	sla	c
	rl	b
	adc	hl,hl
	jr	c,5$
	or	a
	sbc	hl,de
	jr	nc,2$
	add	hl,de
	jr	3$
5$:	or	a
	sbc	hl,de
2$:	inc	c
3$:	dec	a
	jr	nz,1$
	ret
