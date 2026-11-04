	.module drv
	.globl _main
	.globl OP
	.area _CODE
_main:
	ld	ix,#vec
	ld	bc,#NV
	ld	hl,#0
	ld	(0x8f00),hl
next:
	push	bc
	ld	l,6(ix)
	ld	h,7(ix)
	push	hl
	ld	c,4(ix)
	ld	b,5(ix)
	ld	e,2(ix)
	ld	d,3(ix)
	ld	l,0(ix)
	ld	h,1(ix)
	call	OP
	inc	sp
	inc	sp
	ld	a,e
	cp	a,8(ix)
	jr	nz,bad
	ld	a,d
	cp	a,9(ix)
	jr	z,good
bad:	ld	hl,(0x8f00)
	inc	hl
	ld	(0x8f00),hl
good:	ld	de,#12
	add	ix,de
	pop	bc
	dec	bc
	ld	a,b
	or	a,c
	jr	nz,next
	ret
	.include	"vec.inc"
