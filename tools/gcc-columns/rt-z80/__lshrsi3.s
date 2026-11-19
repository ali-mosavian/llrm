; SImode logical shift right; layout as __ashlsi3.
	.module lshrsi3
	.globl	___lshrsi3
	.area	_CODE
___lshrsi3:
	ld	hl,#2
	add	hl,sp
	ld	e,(hl)
	inc	hl
	ld	d,(hl)
	inc	hl
	ld	c,(hl)
	inc	hl
	ld	b,(hl)
	inc	hl
	ld	a,(hl)
	or	a
	jr	z,2$
1$:	srl	b
	rr	c
	rr	d
	rr	e
	dec	a
	jr	nz,1$
2$:	ld	l,c
	ld	h,b
	ret
