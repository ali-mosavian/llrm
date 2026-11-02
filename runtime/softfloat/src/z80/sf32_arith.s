; binary32 add, subtract, multiply and divide for the Z80 (sdasz80), round to nearest even, denormals, inf, NaN. The llrm-z80 convention:
; first float in HL (low) and DE (high), second float's low word in BC and its high word on the stack above the return
; address, popped by the caller; result low word in DE... as a long: HL low, DE high is the argument layout, and the
; result comes back as a long: low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Every NaN result is the quiet NaN 0x7fc00000. The work is done in a 24-byte frame addressed through IY:
;   0..3 a   4..7 b   8 ea  9 eb  10 sign of a / of the result  11 sign of b  12 b's sign mask / na
;   13 quotient group count  14..15 product low bytes  20 nb  21 operation  22..23 mul/div exponent
; Results are rounded from a 32-bit block in d:e:h:l (pack32): leading one at bit 31, l the guard byte, sticky in its bit 0.
	.module sf32_arith
	.area _CODE

	.globl __subsf3
__subsf3:
	ld	a,#0x80
	jr	fas
	.globl __mulsf3
__mulsf3:
	ld	a,#1
	jr	fas
	.globl __divsf3
__divsf3:
	ld	a,#2
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
	ld	a,12(iy)		; 1 multiply, 2 divide
	and	a,#0x7f
	jp	nz,md
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
pack32:	bit	7,b
	jr	nz,low32
	ld	a,b
	or	a,a
	jp	nz,inf
	ld	a,c
	inc	a
	jp	z,inf
	dec	a
	jr	nz,rnd32
low32:	ld	a,b		; e <= 0: shift right by 1 - e, folding the lost bits into bit 0
	or	a,a
	jr	z,11$
	inc	a
	jr	nz,1$		; far below: everything is sticky
	ld	a,c
	cp	a,#2
	jr	c,1$
11$:	ld	a,#1
	sub	a,c
	cp	a,#32
	jr	c,2$
1$:	ld	a,d
	or	a,e
	or	a,h
	or	a,l
	ld	de,#0
	ld	h,d
	ld	l,d
	jr	z,5$
	inc	l
	jr	5$
2$:	ld	b,a
	ld	c,#0		; c: lost bits
3$:	ld	a,b
	cp	a,#8
	jr	c,4$
	ld	a,c
	or	a,l
	ld	c,a
	ld	l,h
	ld	h,e
	ld	e,d
	ld	d,#0
	ld	a,b
	sub	a,#8
	ld	b,a
	jr	3$
4$:	or	a,a
	jr	z,6$
7$:	srl	d
	rr	e
	rr	h
	rr	l
	jr	nc,8$
	set	0,c
8$:	djnz	7$
6$:	ld	a,c
	or	a,a
	jr	z,5$
	set	0,l
5$:	ld	c,#1
rnd32:	ld	a,l
	cp	a,#0x80
	jr	c,nou32
	jr	nz,9$
	bit	0,h
	jr	z,nou32
9$:	inc	h
	jr	nz,nou32
	inc	e
	jr	nz,nou32
	inc	d
	jr	nz,nou32
	ld	d,#0x80		; the mantissa wrapped: 1.0 at the next exponent
	inc	c
	jr	z,inf
nou32:	ld	l,h
	ld	h,e
	ld	a,d
	add	a,a		; carry: the leading one (normal)
	ld	e,a
	ld	a,c
	jr	c,10$
	xor	a,a		; denormal: exponent field 0
10$:	srl	a
	rr	e
	or	a,10(iy)
	ld	d,a
	jp	done
inf:	ld	a,10(iy)
	or	a,#0x7f
	ld	d,a
	ld	e,#0x80
	ld	hl,#0
	jp	done

; exponent bytes of a and b
exps:	ld	a,2(iy)
	add	a,a
	ld	a,3(iy)
	adc	a,a
	ld	8(iy),a
	ld	a,6(iy)
	add	a,a
	ld	a,7(iy)
	adc	a,a
	ld	9(iy),a
	ret

special:
	ld	a,8(iy)		; a NaN: exponent 255 and a fraction
	inc	a
	jr	nz,22$
	ld	a,2(iy)
	and	a,#0x7f
	or	a,1(iy)
	or	a,0(iy)
	jr	nz,retnan
22$:	ld	a,9(iy)
	inc	a
	jr	nz,23$
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jr	nz,retnan
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
	jr	done
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
	jr	done
retb:	ld	l,4(iy)
	ld	h,5(iy)
	ld	e,6(iy)
	ld	d,7(iy)
	jr	done
retnan:	ld	hl,#0
	ld	de,#0x7fc0
done:	ex	de,hl		; the long result: low word in de, high word in hl
	ld	iy,#24
	add	iy,sp
	ld	sp,iy
	ret

; ---- multiply and divide: the operation (1 or 2) is in A, the frame holds a and b as for add
md:	ld	21(iy),a
	ld	a,3(iy)
	xor	a,7(iy)
	and	a,#0x80
	ld	10(iy),a
	ld	a,8(iy)		; both normal: no special case, nothing to normalize
	inc	a
	cp	a,#2
	jr	c,mdsp
	ld	a,9(iy)
	inc	a
	cp	a,#2
	jr	c,mdsp
	set	7,2(iy)
	set	7,6(iy)
	xor	a,a
	ld	12(iy),a
	ld	20(iy),a
	ld	a,21(iy)
	dec	a
	jp	z,mulnrm
	jp	divnrm
mdsp:	ld	a,8(iy)		; a NaN operand
	inc	a
	jr	nz,1$
	ld	a,2(iy)
	and	a,#0x7f
	or	a,1(iy)
	or	a,0(iy)
	jp	nz,retnan
1$:	ld	a,9(iy)
	inc	a
	jr	nz,2$
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jp	nz,retnan
2$:	ld	d,#0		; d, e: a, b is a zero
	ld	a,8(iy)
	or	a,a
	jr	nz,3$
	ld	a,2(iy)
	and	a,#0x7f
	or	a,1(iy)
	or	a,0(iy)
	jr	nz,3$
	ld	d,#1
3$:	ld	e,#0
	ld	a,9(iy)
	or	a,a
	jr	nz,4$
	ld	a,6(iy)
	and	a,#0x7f
	or	a,5(iy)
	or	a,4(iy)
	jr	nz,4$
	ld	e,#1
4$:	ld	a,21(iy)
	cp	a,#1
	jr	nz,divsp
	ld	a,8(iy)		; multiply: inf * 0 is NaN, inf * x is inf, 0 * x is 0
	inc	a
	jr	z,5$
	ld	a,9(iy)
	inc	a
	jr	z,6$
	ld	a,d
	or	a,e
	jp	z,mulgo
	jr	retzero
5$:	ld	a,e
	or	a,a
	jp	nz,retnan
	jr	retinf
6$:	ld	a,d
	or	a,a
	jp	nz,retnan
	jr	retinf
divsp:	ld	a,8(iy)		; divide: inf / inf and 0 / 0 are NaN, inf / x is inf, x / inf is 0, x / 0 is inf
	inc	a
	jr	nz,7$
	ld	a,9(iy)
	inc	a
	jp	z,retnan
	jr	retinf
7$:	ld	a,9(iy)
	inc	a
	jr	z,retzero
	ld	a,e
	or	a,a
	jr	z,8$
	ld	a,d
	or	a,a
	jp	nz,retnan
	jr	retinf
8$:	ld	a,d
	or	a,a
	jr	nz,retzero
	jp	divgo
retzero:
	ld	d,10(iy)
	ld	e,#0
	ld	hl,#0
	jp	done
retinf:	ld	a,10(iy)
	or	a,#0x7f
	ld	d,a
	ld	e,#0x80
	ld	hl,#0
	jp	done

; mantissas with the hidden one in bit 23, denormals normalized: na, nb (12, 20) count the shifts
unp:	ld	a,2(iy)
	and	a,#0x7f
	ld	2(iy),a
	ld	3(iy),#0
	ld	12(iy),#0
	ld	a,8(iy)
	or	a,a
	jr	z,1$
	set	7,2(iy)
	jr	3$
1$:	ld	8(iy),#1
2$:	bit	7,2(iy)
	jr	nz,3$
	sla	0(iy)
	rl	1(iy)
	rl	2(iy)
	inc	12(iy)
	jr	2$
3$:	ld	a,6(iy)
	and	a,#0x7f
	ld	6(iy),a
	ld	7(iy),#0
	ld	20(iy),#0
	ld	a,9(iy)
	or	a,a
	jr	z,4$
	set	7,6(iy)
	ret
4$:	ld	9(iy),#1
5$:	bit	7,6(iy)
	ret	nz
	sla	4(iy)
	rl	5(iy)
	rl	6(iy)
	inc	20(iy)
	jr	5$

mulgo:	call	unp
mulnrm:
	ld	a,8(iy)		; e = ea + eb - 127 - na - nb
	add	a,9(iy)
	ld	c,a
	ld	a,#0
	adc	a,#0
	ld	b,a
	ld	a,12(iy)
	add	a,20(iy)
	add	a,#127
	ld	e,a
	ld	d,#0
	ld	h,b
	ld	l,c
	or	a,a
	sbc	hl,de
	ld	22(iy),l
	ld	23(iy),h
.ifdef SF_MUL_LUT
	xor	a,a		; quarter squares (measured slower than m8): x*y = q1[x+y] - q2[255-x+y], nine products into 14..19(iy)
	ld	14(iy),a
	ld	15(iy),a
	ld	16(iy),a
	ld	17(iy),a
	ld	18(iy),a
	ld	19(iy),a
	ld	b,0(iy)
	ld	a,b
	cpl
	ld	c,a
	ld	a,4(iy)
	call	qm
	ld	l,#14
	call	qadd
	ld	a,5(iy)
	call	qm
	ld	l,#15
	call	qadd
	ld	a,6(iy)
	call	qm
	ld	l,#16
	call	qadd
	ld	b,1(iy)
	ld	a,b
	cpl
	ld	c,a
	ld	a,4(iy)
	call	qm
	ld	l,#15
	call	qadd
	ld	a,5(iy)
	call	qm
	ld	l,#16
	call	qadd
	ld	a,6(iy)
	call	qm
	ld	l,#17
	call	qadd
	ld	b,2(iy)
	ld	a,b
	cpl
	ld	c,a
	ld	a,4(iy)
	call	qm
	ld	l,#16
	call	qadd
	ld	a,5(iy)
	call	qm
	ld	l,#17
	call	qadd
	ld	a,6(iy)
	call	qm
	ld	l,#18
	call	qadd
	ld	c,22(iy)
	ld	b,23(iy)
	ld	d,19(iy)
	ld	e,18(iy)
	ld	h,17(iy)
	ld	l,16(iy)
	bit	7,d
	jr	z,1$
	inc	bc		; product >= 2: exponent up
	ld	a,15(iy)
	jr	2$
1$:	ld	a,15(iy)		; else one place left
	add	a,a
	adc	hl,hl
	rl	e
	rl	d
2$:	or	a,14(iy)
	jp	z,pack32
	set	0,l
	jp	pack32
; de = b * a, c = ~b (tables page aligned)
qm:	ld	e,a
	add	a,b
	ld	l,a
	ld	a,#>q1lo
	adc	a,#0
	ld	h,a
	ld	a,e
	add	a,c
	ld	e,(hl)
	inc	h
	inc	h
	ld	d,(hl)
	ld	l,a
	ld	a,#>q2lo
	adc	a,#0
	ld	h,a
	ld	a,e
	sub	a,(hl)
	ld	e,a
	inc	h
	inc	h
	ld	a,d
	sbc	a,(hl)
	ld	d,a
	ret
; adds de into the sum at offset l in the caller's frame (sp + 2 here), the carry rippling up
qadd:	ld	h,#0
	add	hl,sp
	inc	hl
	inc	hl
	ld	a,(hl)
	add	a,e
	ld	(hl),a
	inc	hl
	ld	a,(hl)
	adc	a,d
	ld	(hl),a
	ret	nc
3$:	inc	hl
	inc	(hl)
	jr	z,3$
	ret
.else
	ld	c,0(iy)		; multiplicand a in e:bc, sum in a:hl, multiplier byte in d: one rr chain per bit
	ld	b,1(iy)
	ld	e,2(iy)
	xor	a,a
	ld	h,a
	ld	l,a
	ld	d,4(iy)
	call	m8
	ld	14(iy),d
	ld	d,5(iy)
	call	m8
	ld	15(iy),d
	ld	d,6(iy)
	call	m8
	ld	c,22(iy)
	ld	b,23(iy)
	jr	nc,1$		; the adds came one place early: a:h:l:d is twice the product's top 32 bits
	rra			; product >= 2: once more right, exponent up
	rr	h
	rr	l
	rr	d
	inc	bc
	jr	nc,1$
	set	0,14(iy)
1$:	ld	e,a		; block a:h:l:d -> d:e:h:l, the low bytes into the sticky bit
	ld	a,14(iy)
	or	a,15(iy)
	ld	a,d
	ld	d,e
	ld	e,h
	ld	h,l
	ld	l,a
	jp	z,pack32
	set	0,l
	jp	pack32
m8:	.rept	8		; eight multiplier bits from d; the add carry enters the next rr chain
	rra
	rr	h
	rr	l
	rr	d
	jr	nc,.+4
	add	hl,bc
	adc	a,e
	.endm
	ret
.endif

divgo:	call	unp
divnrm:
	ld	a,8(iy)		; e = ea - eb + 126 + nb - na
	sub	a,9(iy)
	ld	c,a
	ld	a,#0
	sbc	a,#0
	ld	b,a
	ld	a,20(iy)
	sub	a,12(iy)
	ld	e,a
	ld	a,#0
	sbc	a,#0
	ld	d,a
	ld	hl,#126
	add	hl,de
	add	hl,bc
	ld	22(iy),l
	ld	23(iy),h
	ld	l,0(iy)		; remainder in a:hl starts as ma; the divisor in e:bc
	ld	h,1(iy)
	ld	a,2(iy)
	ld	c,4(iy)
	ld	b,5(iy)
	ld	e,6(iy)
	ld	d,#1		; 24 quotient bits, complemented, in three groups collected in d behind a sentinel
	ld	13(iy),#3
	or	a,a		; ratio < 1: one step down, so the first quotient bit is always 1
	sbc	hl,bc
	sbc	a,e
	jr	nc,16$
	add	hl,bc
	adc	a,e
	jr	14$
16$:	inc	22(iy)
	jr	nz,12$
	inc	23(iy)
	or	a,a
	jr	12$
11$:	or	a,a
	sbc	hl,bc
	sbc	a,e
	jr	nc,12$
	add	hl,bc		; too far: put it back; the carry (1) is the complemented bit 0
	adc	a,e
12$:	rl	d
	jr	c,15$		; the sentinel fell out: a group is complete
14$:	add	hl,hl
	adc	a,a
	jr	nc,11$
	or	a,a		; the remainder reached bit 24: above the divisor, the bit is 1
	sbc	hl,bc
	sbc	a,e
	or	a,a
	jr	12$
15$:	push	de		; groups wait on the stack
	ld	d,#1
	dec	13(iy)
	jr	nz,14$
	ld	d,a		; guard byte from 2R against the divisor: 0, 1 (below half), 0x80 (half), 0x81 (above)
	or	a,h
	or	a,l
	jr	z,17$
	ld	a,d
	add	hl,hl
	adc	a,a
	jr	c,18$
	sbc	hl,bc
	sbc	a,e
	ld	l,#1
	jr	c,19$
	or	a,h
	or	a,l
	ld	l,#0x80
	jr	z,19$
18$:	ld	l,#0x81
	jr	19$
17$:	ld	l,a
19$:	pop	de
	ld	a,d
	cpl
	ld	h,a
	pop	de
	ld	a,d
	cpl
	ld	c,a
	pop	de
	ld	a,d
	cpl
	ld	d,a
	ld	e,c
	ld	c,22(iy)
	ld	b,23(iy)
	jp	pack32

; ---- conversions with 32-bit integers: the integer is in HL (low) and DE (high), the float likewise -------------------

	.globl __floatsisf
__floatsisf:
	ld	a,#1
	jr	flent
	.globl __floatunsisf
__floatunsisf:
	xor	a,a
flent:	ld	iy,#-24
	add	iy,sp
	ld	sp,iy
	ld	10(iy),#0
	or	a,a
	jr	z,1$
	bit	7,d		; signed and negative: keep the sign, work on the magnitude
	jr	z,1$
	ld	10(iy),#0x80
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
1$:	ld	a,d
	or	a,e
	or	a,h
	or	a,l
	jp	z,retzero32
	ld	bc,#158		; 127 + 31: the exponent when the leading one is at bit 31
2$:	ld	a,d		; a zero top byte moves up whole
	or	a,a
	jr	nz,3$
	ld	d,e
	ld	e,h
	ld	h,l
	ld	l,a
	ld	a,c
	sub	a,#8
	ld	c,a
	jr	2$
3$:	bit	7,d
	jp	nz,pack32
	add	hl,hl
	rl	e
	rl	d
	dec	c
	jr	3$

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
retzero32:
	ld	hl,#0
	ld	d,h
	ld	e,l
	jp	done

.ifdef SF_MUL_LUT
	.area	_SFLUT		; 256-byte aligned (linker -b); q1 lo, q1 hi, q2 lo, q2 hi, 512 entries each
q1lo:
n = 0
	.rept	512
	.db	(n*n/4)&0xff
n = n + 1
	.endm
n = 0
	.rept	512
	.db	(n*n/4)>>8
n = n + 1
	.endm
q2lo:
n = 0
	.rept	512
	.db	((n-255)*(n-255)/4)&0xff
n = n + 1
	.endm
n = 0
	.rept	512
	.db	((n-255)*(n-255)/4)>>8
n = n + 1
	.endm
.endif
