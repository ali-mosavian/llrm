; SImode multiply, low 32 bits: a * b -> e d l h.  Stack at entry: ret(2) a(4) b(4).
; ix is callee-saved and holds the frame; the shadow set carries the high halves.
	.module mulsi3
	.globl	___mulsi3
	.area	_CODE
___mulsi3:
	push	ix
	ld	ix,#0
	add	ix,sp
	ld	e,4 (ix)
	ld	d,5 (ix)
	exx
	ld	e,6 (ix)
	ld	d,7 (ix)
	ld	hl,#0
	exx
	ld	hl,#0
	ld	b,#32
1$:	srl	11 (ix)
	rr	10 (ix)
	rr	9 (ix)
	rr	8 (ix)
	jr	nc,2$
	add	hl,de
	exx
	adc	hl,de
	exx
2$:	sla	e
	rl	d
	exx
	rl	e
	rl	d
	exx
	djnz	1$
	push	hl
	exx
	push	hl
	exx
	pop	hl
	pop	de
	pop	ix
	ret
