; Unsigned 16-bit divide: hl / de -> hl (restoring, remainder in bc-shift scheme).
	.module udivhi3
	.globl	___udivhi3
	.area	_CODE
___udivhi3:
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
	ld	h,b
	ld	l,c
	ret
