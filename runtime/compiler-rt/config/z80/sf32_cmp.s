; binary32 comparisons for the Z80 (sdasz80), the llrm-z80 convention: first float in HL (low) and DE (high), second
; float's low word in BC and its high word on the stack above the return address, popped by the caller; the int result
; in DE. compiler-rt's values: -1, 0, 1 for less, equal, greater; an unordered pair gives 1 from __eqsf2 __nesf2
; __ltsf2 __lesf2 and -1 from __gtsf2 __gesf2. Changes A BC DE HL IY and the flags; keeps IX and SP.
	.module sf32_cmp
	.area _CODE

	.globl __eqsf2
	.globl __nesf2
	.globl __ltsf2
	.globl __lesf2
__eqsf2:
__nesf2:
__ltsf2:
__lesf2:
	ld	iy,#2
	add	iy,sp
	call	cmpf
	cp	a,#2
	jr	nz,ret8
	ld	a,#1
	jr	ret8

	.globl __gtsf2
	.globl __gesf2
__gtsf2:
__gesf2:
	ld	iy,#2
	add	iy,sp
	call	cmpf
	cp	a,#2
	jr	nz,ret8
	ld	a,#0xff
ret8:	ld	e,a
	add	a,a
	sbc	a,a
	ld	d,a
	ret

	.globl __unordsf2
__unordsf2:
	ld	iy,#2
	add	iy,sp
	call	cmpf
	cp	a,#2
	ld	a,#0
	jr	nz,ret8
	inc	a
	jr	ret8

; A = -1, 0, 1 for a <, ==, > b; 2 when either is a NaN
cmpf:
	ld	a,d		; a NaN: magnitude above 0x7f800000
	and	a,#0x7f
	cp	a,#0x7f
	jr	c,1$
	jr	nz,unord
	ld	a,e
	cp	a,#0x80
	jr	c,1$
	jr	nz,unord
	ld	a,h
	or	a,l
	jr	nz,unord
1$:	ld	a,1(iy)		; b the same
	and	a,#0x7f
	cp	a,#0x7f
	jr	c,2$
	jr	nz,unord
	ld	a,0(iy)
	cp	a,#0x80
	jr	c,2$
	jr	nz,unord
	ld	a,b
	or	a,c
	jr	nz,unord
2$:	ld	a,d
	xor	a,1(iy)
	jp	m,3$
	ld	a,1(iy)		; same sign: the raw bytes order the magnitudes, high byte first
	cp	a,d
	jr	nz,4$
	ld	a,0(iy)
	cp	a,e
	jr	nz,4$
	ld	a,b
	cp	a,h
	jr	nz,4$
	ld	a,c
	cp	a,l
	jr	z,5$
4$:	ld	a,#1		; carry: b below a in magnitude
	jr	c,6$
	ld	a,#0xff
6$:	bit	7,d		; negative operands order the other way
	ret	z
	neg
	ret
5$:	xor	a,a
	ret
3$:	ld	a,d		; signs differ: +0 equals -0, otherwise the negative one is less
	and	a,#0x7f
	or	a,e
	or	a,h
	or	a,l
	jr	nz,7$
	ld	a,1(iy)
	and	a,#0x7f
	or	a,0(iy)
	or	a,b
	or	a,c
	jr	z,5$
7$:	bit	7,d
	ld	a,#0xff
	ret	nz
	ld	a,#1
	ret
unord:	ld	a,#2
	ret
