; binary32 add, subtract, multiply and divide for the Z80 (sdasz80), round to nearest even, denormals, inf, NaN. The llrm-z80 convention:
; first float in HL (low) and DE (high), second float's low word in BC and its high word on the stack above the return
; address, popped by the caller; result low word in DE... as a long: HL low, DE high is the argument layout, and the
; result comes back as a long: low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Every NaN result is the quiet NaN 0x7fc00000. The work is done in a 24-byte frame addressed through IY:
;   0..3 a   4..7 b   8 ea  9 eb  10 sign of a / of the result  11 sign of b  12 b's sign mask / na
;   13 quotient group count  14..15 product low bytes  20 nb  21 operation  22..23 mul/div exponent
; Results are rounded from a 32-bit block in d:e:h:l (pack32): leading one at bit 31, l the guard byte, sticky in its bit 0.
	.module sf32_addsub
	.area _CODE

	.globl __addsf3
	.globl __subsf3
	.globl pack32
	.globl done
	.globl retnan
	.globl inf
	.globl exps
	.globl __subsf3
__subsf3:
	ld	a,#0x80
	jr	fas
	.globl __addsf3
__addsf3:
	xor	a,a
fas:	ld	iy,#-24
	add	iy,sp
	ld	sp,iy
	ld	12(iy),a
	ld	0(iy),l
	ld	1(iy),h
	ld	2(iy),e
	ld	3(iy),d
	ld	4(iy),c
	ld	5(iy),b
	ld	a,26(iy)
	ld	6(iy),a
	ld	a,12(iy)
	and	a,#0x80
	xor	a,27(iy)
	ld	7(iy),a
	call	exps
	ld	a,8(iy)		; an infinity or a NaN
	inc	a
	jp	z,special
	ld	a,9(iy)
	inc	a
	jp	z,special
	ld	a,8(iy)		; a zero operand
	or	a,a
	jr	nz,1$
	ld	a,2(iy)
	and	a,#0x7f
	or	a,1(iy)
	or	a,0(iy)
	jp	z,azero
1$:	ld	a,9(iy)
	or	a,a
	jr	nz,2$
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jp	z,reta		; b is zero
2$:	ld	a,7(iy)		; order by magnitude, larger first
	and	a,#0x7f
	ld	c,a
	ld	a,3(iy)
	and	a,#0x7f
	cp	a,c
	jr	c,swap		; a below b
	jr	nz,noswap
	ld	a,2(iy)
	cp	a,6(iy)
	jr	c,swap
	jr	nz,noswap
	ld	a,1(iy)
	cp	a,5(iy)
	jr	c,swap
	jr	nz,noswap
	ld	a,0(iy)
	cp	a,4(iy)
	jr	nc,noswap
swap:	ld	a,0(iy)
	ld	c,4(iy)
	ld	0(iy),c
	ld	4(iy),a
	ld	a,1(iy)
	ld	c,5(iy)
	ld	1(iy),c
	ld	5(iy),a
	ld	a,2(iy)
	ld	c,6(iy)
	ld	2(iy),c
	ld	6(iy),a
	ld	a,3(iy)
	ld	c,7(iy)
	ld	3(iy),c
	ld	7(iy),a
	ld	a,8(iy)
	ld	c,9(iy)
	ld	8(iy),c
	ld	9(iy),a
noswap:
	ld	a,3(iy)		; signs: a's is the result's
	and	a,#0x80
	ld	10(iy),a
	ld	a,7(iy)
	and	a,#0x80
	ld	11(iy),a
	ld	c,8(iy)		; ea in c; a's block 2(iy):1(iy):0(iy) with the hidden one, guard byte 0
	ld	a,c
	or	a,a
	jr	nz,4$
	inc	c
	jr	5$
4$:	set	7,2(iy)
5$:	ld	d,6(iy)		; b's block in d:e:h:l
	ld	e,5(iy)
	ld	h,4(iy)
	ld	l,#0
	ld	a,9(iy)
	or	a,a
	jr	nz,6$
	inc	a
	jr	7$
6$:	set	7,d
7$:	neg			; align b: shift right by ea - eb, lost bits folded into bit 0
	add	a,c
	jr	z,aligned
	cp	a,#26
	jr	c,8$
	ld	de,#0		; b is all sticky
	ld	hl,#1
	jr	aligned
8$:	ld	b,a
9$:	ld	a,b
	cp	a,#8
	jr	c,11$
	ld	a,l
	or	a,a
	ld	l,h
	jr	z,10$
	set	0,l
10$:	ld	h,e
	ld	e,d
	ld	d,#0
	ld	a,b
	sub	a,#8
	ld	b,a
	jr	9$
11$:	or	a,a
	jr	z,aligned
12$:	srl	d
	rr	e
	rr	h
	rr	l
	jr	nc,13$
	set	0,l
13$:	djnz	12$
aligned:
	ld	a,10(iy)
	xor	a,11(iy)
	jr	nz,dosub
	ld	a,h
	add	a,0(iy)
	ld	h,a
	ld	a,e
	adc	a,1(iy)
	ld	e,a
	ld	a,d
	adc	a,2(iy)
	ld	d,a
	jr	nc,pk
	rr	d		; carry out: one right, the lost bit joins the sticky bit
	rr	e
	rr	h
	rr	l
	jr	nc,14$
	set	0,l
14$:	inc	c
	jr	pk
dosub:	xor	a,a		; a's block minus b's: |a| >= |b|
	sub	a,l
	ld	l,a
	ld	a,0(iy)
	sbc	a,h
	ld	h,a
	ld	a,1(iy)
	sbc	a,e
	ld	e,a
	ld	a,2(iy)
	sbc	a,d
	ld	d,a
	or	a,e
	or	a,h
	or	a,l
	jr	nz,16$
	ld	h,a		; exact cancellation is +0
	ld	l,a
	jp	done
16$:	ld	a,d		; normalize: whole bytes, then bits, while the exponent allows
	or	a,a
	jr	nz,17$
	ld	a,c
	cp	a,#9
	jr	c,17$
	sub	a,#8
	ld	c,a
	ld	d,e
	ld	e,h
	ld	h,l
	ld	l,#0
	jr	16$
17$:	bit	7,d
	jr	nz,pk
	ld	a,c
	cp	a,#2
	jr	c,pk
	dec	c
	add	hl,hl
	rl	e
	rl	d
	jr	17$
pk:	ld	b,#0
	jp	pack32
; round and pack: bc = exponent (signed) of bit 31 of the block d:e:h:l (l the guard byte, sticky in its bit 0), 10(iy) the sign
special:
	ld	a,8(iy)		; a NaN: exponent 255 and a fraction
	inc	a
	jr	nz,22$
	ld	a,2(iy)
	and	a,#0x7f
	or	a,1(iy)
	or	a,0(iy)
	jp	nz,retnan
22$:	ld	a,9(iy)
	inc	a
	jr	nz,23$
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jp	nz,retnan
23$:	ld	a,8(iy)		; no NaN: an infinity, and inf - inf is NaN
	inc	a
	jr	nz,retb
	ld	a,9(iy)
	inc	a
	jr	nz,reta
	ld	a,3(iy)
	xor	a,7(iy)
	jp	m,retnan
reta:	ld	l,0(iy)
	ld	h,1(iy)
	ld	e,2(iy)
	ld	d,3(iy)
	jp	done
azero:	ld	a,9(iy)		; a is zero: b, or when both are zero the and of the signs
	or	a,a
	jr	nz,retb
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jr	nz,retb
	ld	a,3(iy)
	and	a,7(iy)
	ld	d,a
	ld	hl,#0
	ld	e,l
	jp	done
retb:	ld	l,4(iy)
	ld	h,5(iy)
	ld	e,6(iy)
	ld	d,7(iy)
	jp	done
