; binary32 add, subtract, multiply and divide for the Z80 (sdasz80), round to nearest even, denormals, inf, NaN. The llrm-z80 convention:
; first float in HL (low) and DE (high), second float's low word in BC and its high word on the stack above the return
; address, popped by the caller; result low word in DE... as a long: HL low, DE high is the argument layout, and the
; result comes back as a long: low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Every NaN result is the quiet NaN 0x7fc00000. The work is done in a 24-byte frame addressed through IY:
;   0..3 a   4..7 b   8 ea  9 eb  10 sign of a / of the result  11 sign of b  12 b's sign mask / na
;   13 quotient group count  14..15 product low bytes  20 nb  21 operation  22..23 mul/div exponent
; Results are rounded from a 32-bit block in d:e:h:l (pack32): leading one at bit 31, l the guard byte, sticky in its bit 0.
	.module sf32_fix
	.area _CODE

	.globl __fixsfsi
	.globl __fixunssfsi
	.globl done
	.globl __fixsfsi
__fixsfsi:
	ld	c,#1
	jr	fixent
	.globl __fixunssfsi
__fixunssfsi:
	ld	c,#0
fixent:	ld	a,d		; c: bit 7 the sign, bit 0 signed; no frame
	and	a,#0x80
	or	a,c
	ld	c,a
	ld	a,e
	add	a,a
	ld	a,d
	adc	a,a
	cp	a,#127		; |x| < 1
	jr	c,fzero
	cp	a,#159		; |x| >= 2^32, inf, NaN
	jr	nc,fbig
	set	7,e
	ld	d,#0
	sub	a,#150		; shift by e - 23
	jr	z,fsign
	jr	c,frt
	ld	b,a
1$:	add	hl,hl
	rl	e
	rl	d
	djnz	1$
	jr	fsign
frt:	neg
2$:	cp	a,#8		; whole bytes first
	jr	c,3$
	ld	l,h
	ld	h,e
	ld	e,d
	sub	a,#8
	jr	2$
3$:	or	a,a
	jr	z,fsign
	ld	b,a
4$:	srl	e
	rr	h
	rr	l
	djnz	4$
fsign:	bit	0,c
	jr	z,funs
	bit	7,d		; 2^31 or more
	jr	nz,fsat
	bit	7,c
	jr	z,fret
	xor	a,a
	sub	a,l
	ld	l,a
	ld	a,#0
	sbc	a,h
	ld	h,a
	ld	a,#0
	sbc	a,e
	ld	e,a
	ld	a,#0
	sbc	a,d
	ld	d,a
fret:	ex	de,hl
	ret
funs:	bit	7,c
	jr	z,fret
fzero:	ld	hl,#0
	ld	d,h
	ld	e,l
	ret
fbig:	inc	a		; NaN gives 0
	jr	nz,fsat
	ld	a,e
	and	a,#0x7f
	or	a,h
	or	a,l
	jr	nz,fzero
fsat:	bit	7,c
	jr	nz,5$
	ld	hl,#0x7fff		; the largest: 0x7fffffff, or 0xffffffff unsigned
	bit	0,c
	jr	nz,6$
	ld	h,l
6$:	ld	de,#0xffff
	ret
5$:	bit	0,c		; negative: 0x80000000, or 0 unsigned
	jr	z,fzero
	ld	hl,#0x8000
	ld	de,#0
	ret
