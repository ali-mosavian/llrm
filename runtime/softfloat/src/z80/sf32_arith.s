; binary32 add, subtract, multiply and divide for the Z80 (sdasz80), round to nearest even, denormals, inf, NaN. The llrm-z80 convention:
; first float in HL (low) and DE (high), second float's low word in BC and its high word on the stack above the return
; address, popped by the caller; result low word in DE... as a long: HL low, DE high is the argument layout, and the
; result comes back as a long: low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Every NaN result is the quiet NaN 0x7fc00000. The work is done in a 24-byte frame addressed through IY:
;   0..3 a, then the mantissa result   4..7 b   8 ea  9 eb  10 sign of a / of the result  11 sign of b  12 b's sign mask / na
;   13 sticky  14..19 product or remainder  20 nb  22..23 spare
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
fas:	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	push	hl
	ld	iy,#0
	add	iy,sp
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
	ld	a,3(iy)		; sign and mantissa of a, hidden one, three guard bits
	and	a,#0x80
	ld	10(iy),a
	ld	a,2(iy)
	and	a,#0x7f
	ld	2(iy),a
	ld	a,8(iy)
	or	a,a
	jr	nz,4$
	ld	8(iy),#1
	jr	5$
4$:	set	7,2(iy)
5$:	ld	3(iy),#0
	ld	a,7(iy)
	and	a,#0x80
	ld	11(iy),a
	ld	a,6(iy)
	and	a,#0x7f
	ld	6(iy),a
	ld	a,9(iy)
	or	a,a
	jr	nz,6$
	ld	9(iy),#1
	jr	7$
6$:	set	7,6(iy)
7$:	ld	7(iy),#0
	ld	b,#3
8$:	sla	0(iy)
	rl	1(iy)
	rl	2(iy)
	rl	3(iy)
	sla	4(iy)
	rl	5(iy)
	rl	6(iy)
	rl	7(iy)
	djnz	8$
	ld	a,8(iy)		; align b: shift right by ea - eb, lost bits folded into bit 0
	sub	a,9(iy)
	jr	z,aligned
	ld	b,a
	ld	13(iy),#0
	cp	a,#32
	jr	c,9$
	ld	b,#32
9$:	ld	a,b
	cp	a,#8
	jr	c,11$
	ld	a,13(iy)
	or	a,4(iy)
	ld	13(iy),a
	ld	a,5(iy)
	ld	4(iy),a
	ld	a,6(iy)
	ld	5(iy),a
	ld	a,7(iy)
	ld	6(iy),a
	ld	7(iy),#0
	ld	a,b
	sub	a,#8
	ld	b,a
	jr	9$
11$:	ld	a,b
	or	a,a
	jr	z,13$
12$:	srl	7(iy)
	rr	6(iy)
	rr	5(iy)
	rr	4(iy)
	jr	nc,14$
	set	0,13(iy)
14$:	djnz	12$
13$:	ld	a,13(iy)		; the byte moves left whole bytes in it
	or	a,a
	jr	z,aligned
	set	0,4(iy)
aligned:
	ld	a,10(iy)
	xor	a,11(iy)
	jr	nz,dosub
	ld	a,0(iy)
	add	a,4(iy)
	ld	0(iy),a
	ld	a,1(iy)
	adc	a,5(iy)
	ld	1(iy),a
	ld	a,2(iy)
	adc	a,6(iy)
	ld	2(iy),a
	ld	a,3(iy)
	adc	a,7(iy)
	ld	3(iy),a
	bit	3,3(iy)
	jr	z,pk
	srl	3(iy)
	rr	2(iy)
	rr	1(iy)
	rr	0(iy)
	jr	nc,15$
	set	0,0(iy)
15$:	inc	8(iy)
	jr	pk
dosub:	ld	a,0(iy)
	sub	a,4(iy)
	ld	0(iy),a
	ld	a,1(iy)
	sbc	a,5(iy)
	ld	1(iy),a
	ld	a,2(iy)
	sbc	a,6(iy)
	ld	2(iy),a
	ld	a,3(iy)
	sbc	a,7(iy)
	ld	3(iy),a
	or	a,2(iy)
	or	a,1(iy)
	or	a,0(iy)
	jr	nz,16$
	ld	hl,#0		; exact cancellation is +0
	ld	d,h
	ld	e,l
	jp	done
16$:	bit	2,3(iy)		; normalize: the leading one back to bit 26, unless the exponent runs out
	jr	nz,pk
	ld	a,8(iy)
	cp	a,#2
	jr	c,pk
	dec	8(iy)
	sla	0(iy)
	rl	1(iy)
	rl	2(iy)
	rl	3(iy)
	jr	16$
pk:	ld	b,#0
	ld	c,8(iy)
; round and pack: bc = exponent (signed), 0..3(iy) the 27-bit mantissa, 10(iy) the sign
packbc:	bit	7,b
	jr	nz,low
	ld	a,b
	or	a,a
	jp	nz,inf
	ld	a,c
	cp	a,#255
	jp	nc,inf
	or	a,a
	jr	nz,roundp
low:	ld	hl,#1		; e <= 0: shift right by 1 - e, folding the lost bits into bit 0
	or	a,a
	sbc	hl,bc
	ld	a,h
	or	a,a
	ld	b,#32
	jr	nz,17$
	ld	a,l
	cp	a,#32
	jr	nc,17$
	ld	b,a
17$:	ld	13(iy),#0
18$:	srl	3(iy)
	rr	2(iy)
	rr	1(iy)
	rr	0(iy)
	jr	nc,19$
	set	0,13(iy)
19$:	djnz	18$
	bit	0,13(iy)
	jr	z,20$
	set	0,0(iy)
20$:	ld	bc,#1
roundp:	ld	a,0(iy)
	and	a,#7
	ld	d,a
	ld	e,#3
21$:	srl	3(iy)
	rr	2(iy)
	rr	1(iy)
	rr	0(iy)
	dec	e
	jr	nz,21$
	ld	a,d
	cp	a,#5
	jr	nc,up
	cp	a,#4
	jr	nz,noup
	bit	0,0(iy)
	jr	z,noup
up:	ld	a,0(iy)
	add	a,#1
	ld	0(iy),a
	ld	a,1(iy)
	adc	a,#0
	ld	1(iy),a
	ld	a,2(iy)
	adc	a,#0
	ld	2(iy),a
	ld	a,3(iy)
	adc	a,#0
	ld	3(iy),a
noup:	ld	a,c		; add (e - 1) << 23 and the sign
	dec	a
	ld	c,a
	and	a,#1
	rrca
	ld	d,a
	ld	a,c
	srl	a
	ld	e,a
	ld	a,2(iy)
	add	a,d
	ld	2(iy),a
	ld	a,3(iy)
	adc	a,e
	or	a,10(iy)
	ld	3(iy),a
	jr	reta
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
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	ret

; ---- multiply and divide: the operation (1 or 2) is in A, the frame holds a and b as for add
md:	ld	21(iy),a
	ld	a,3(iy)
	xor	a,7(iy)
	and	a,#0x80
	ld	10(iy),a
	ld	a,8(iy)		; a NaN operand
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
	ld	a,4(iy)		; product 14..19: the multiplier in the low half, the sum in the high half
	ld	14(iy),a
	ld	a,5(iy)
	ld	15(iy),a
	ld	a,6(iy)
	ld	16(iy),a
	xor	a,a
	ld	17(iy),a
	ld	18(iy),a
	ld	19(iy),a
	ld	b,#24
1$:	bit	0,14(iy)
	jr	z,2$
	ld	a,17(iy)
	add	a,0(iy)
	ld	17(iy),a
	ld	a,18(iy)
	adc	a,1(iy)
	ld	18(iy),a
	ld	a,19(iy)
	adc	a,2(iy)
	ld	19(iy),a
	jr	3$
2$:	or	a,a
3$:	rr	19(iy)
	rr	18(iy)
	rr	17(iy)
	rr	16(iy)
	rr	15(iy)
	rr	14(iy)
	djnz	1$
	ld	c,22(iy)	; the product has its top bit at 47 or 46: keep 27 bits, fold the rest into bit 0
	ld	b,23(iy)
	ld	e,#4
	bit	7,19(iy)
	jr	z,4$
	inc	bc
	inc	e
4$:	ld	13(iy),#0
	ld	a,14(iy)
	or	a,15(iy)
	jr	z,5$
	set	0,13(iy)
5$:	ld	a,16(iy)
	ld	0(iy),a
	ld	a,17(iy)
	ld	1(iy),a
	ld	a,18(iy)
	ld	2(iy),a
	ld	a,19(iy)
	ld	3(iy),a
6$:	srl	3(iy)
	rr	2(iy)
	rr	1(iy)
	rr	0(iy)
	jr	nc,7$
	set	0,13(iy)
7$:	dec	e
	jr	nz,6$
	bit	0,13(iy)
	jp	z,packbc
	set	0,0(iy)
	jp	packbc

divgo:	call	unp
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
	ld	a,0(iy)		; remainder 14..17 starts as ma; the quotient grows in 0..3
	ld	14(iy),a
	ld	a,1(iy)
	ld	15(iy),a
	ld	a,2(iy)
	ld	16(iy),a
	xor	a,a
	ld	17(iy),a
	ld	0(iy),a
	ld	1(iy),a
	ld	2(iy),a
	ld	3(iy),a
	ld	b,#28
1$:	sla	0(iy)
	rl	1(iy)
	rl	2(iy)
	rl	3(iy)
	ld	a,14(iy)
	sub	a,4(iy)
	ld	e,a
	ld	a,15(iy)
	sbc	a,5(iy)
	ld	d,a
	ld	a,16(iy)
	sbc	a,6(iy)
	ld	c,a
	ld	a,17(iy)
	sbc	a,#0
	jr	c,2$
	ld	14(iy),e
	ld	15(iy),d
	ld	16(iy),c
	ld	17(iy),a
	set	0,0(iy)
2$:	sla	14(iy)
	rl	15(iy)
	rl	16(iy)
	rl	17(iy)
	djnz	1$
	ld	a,14(iy)
	or	a,15(iy)
	or	a,16(iy)
	or	a,17(iy)
	jr	z,3$
	set	0,0(iy)
3$:	ld	c,22(iy)
	ld	b,23(iy)
	bit	3,3(iy)
	jp	z,packbc
	srl	3(iy)
	rr	2(iy)
	rr	1(iy)
	rr	0(iy)
	jr	nc,4$
	set	0,0(iy)
4$:	inc	bc
	jp	packbc
