; gcc z80 ABI driver: arguments on the stack (b above a), result in e d l h.
; NARGS = 1 or 2.  Record: a(4) b(4) expected(4), 12 bytes, as tests/z80/drv.s.
; The expected value is also loaded into de/hl before the call, so a bare `ret` stub passes every record
; and the stub run costs exactly what the driver costs.
	.module drvg
	.globl _main
	.globl OP
	.area _CODE
_main:
	ld	ix,#vec
	ld	bc,#NV
	ld	hl,#0
	ld	(0xcf00),hl
next:
	push	bc
	.if NARGS-1
	ld	l,6(ix)
	ld	h,7(ix)
	push	hl
	ld	l,4(ix)
	ld	h,5(ix)
	push	hl
	.endif
	ld	l,2(ix)
	ld	h,3(ix)
	push	hl
	ld	l,0(ix)
	ld	h,1(ix)
	push	hl
	ld	e,8(ix)
	ld	d,9(ix)
	ld	l,10(ix)
	ld	h,11(ix)
	call	OP
	pop	af
	pop	af
	.if NARGS-1
	pop	af
	pop	af
	.endif
	ld	a,e
	cp	a,8(ix)
	jr	nz,bad
	ld	a,d
	cp	a,9(ix)
	jr	nz,bad
	ld	a,l
	cp	a,10(ix)
	jr	nz,bad
	ld	a,h
	cp	a,11(ix)
	jr	z,good
bad:	ld	(0xcf1c),de
	ld	(0xcf1e),hl
	ld	hl,(0xcf00)
	ld	a,h
	or	a,l
	jr	nz,more
	push	ix
	pop	hl
	ld	de,#0xcf10
	ld	bc,#12
	ldir
more:	ld	hl,(0xcf00)
	inc	hl
	ld	(0xcf00),hl
good:	ld	de,#12
	add	ix,de
	pop	bc
	dec	bc
	ld	a,b
	or	a,c
	jr	nz,next
	ret
	.include	"vec.inc"
