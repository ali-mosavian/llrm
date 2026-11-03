; SImode shift left: (a on the stack, n above it) -> e d l h = byte 0..3.
; Stack at entry: ret(2) a(4) n(1).
	.module ashlsi3
	.globl	___ashlsi3
	.area	_CODE
___ashlsi3:
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
1$:	sla	e
	rl	d
	rl	c
	rl	b
	dec	a
	jr	nz,1$
2$:	ld	l,c
	ld	h,b
	ret
