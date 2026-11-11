; 32-bit multiply, divide and remainder for the Z80 (sdasz80), the llrm-z80 convention: first long in HL (low) and DE
; (high); second long's low word in BC, its high word on the stack above the return address, popped by the caller;
; long result low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Division by zero gives the all-ones quotient and the dividend as remainder; signed division truncates toward zero
; and the remainder has the dividend's sign.
	.module int32
	.area _CODE

; hl * de -> hl, the low 16 bits
m16lo:
	ld	b,h
	ld	c,l
	ld	hl,#0
1$:	srl	d
	rr	e
	jr	nc,2$
	add	hl,bc
2$:	sla	c
	rl	b
	ld	a,d
	or	a,e
	jr	nz,1$
	ret

; a * b -> low 32 bits: al*bl in full plus (ah*bl + al*bh) << 16
	.globl __mulsi3
__mulsi3:
	push	de		; ah
	push	hl		; al
	push	bc		; bl
	ld	iy,#0
	add	iy,sp		; 0(iy) bl, 2 al, 4 ah, 8 bh
	ld	h,5(iy)
	ld	l,4(iy)
	ld	d,1(iy)
	ld	e,0(iy)
	call	m16lo		; ah*bl
	push	hl
	ld	l,2(iy)
	ld	h,3(iy)
	ld	e,8(iy)
	ld	d,9(iy)
	call	m16lo		; al*bh
	pop	de
	add	hl,de
	push	hl		; cross terms
	ld	c,2(iy)
	ld	b,3(iy)
	ld	e,0(iy)
	ld	d,1(iy)
	ld	hl,#0
	ld	a,#16
1$:	add	hl,hl		; de:hl = al * bl, one multiplier bit per step
	rl	e
	rl	d
	jr	nc,2$
	add	hl,bc
	jr	nc,2$
	inc	de
2$:	dec	a
	jr	nz,1$
	pop	bc
	ex	de,hl
	add	hl,bc
	pop	af
	pop	af
	pop	af
	ret

; unsigned divide, restoring, one quotient bit per step. The divisor is bc and 0(iy) of the caller's high word.
; A selects the result: 0 quotient, 1 remainder. Both operands below 2^16, divisor not zero, take a 16-bit loop.
	.globl __udivsi3
__udivsi3:
	xor	a,a
	jr	udm
	.globl __umodsi3
__umodsi3:
	ld	a,#1
udm:
	ld	iy,#2
	add	iy,sp		; 0(iy) is the divisor's high word
	push	af		; -3(iy) mode
	ld	a,0(iy)
	or	a,1(iy)
	jp	nz,big32		; a 32-bit divisor
	ld	a,b
	or	a,c
	jp	z,big32		; a zero divisor
	ld	a,d
	or	a,e
	jr	nz,6$
	ex	de,hl		; both below 2^16: one 16-bit division
	ld	hl,#0
	call	d16
	bit	0,-3(iy)
	jr	z,5$
	ex	de,hl
5$:	ld	hl,#0
	pop	af
	ret
6$:	push	hl		; a 16-bit divisor: two 16-bit steps, the high word first; -6(iy) holds the low word, then the quotient's high word
	ld	hl,#0
	call	d16
	ld	a,-6(iy)
	ld	-6(iy),e
	ld	e,a
	ld	a,-5(iy)
	ld	-5(iy),d
	ld	d,a
	call	d16
	bit	0,-3(iy)
	jr	z,7$
	ex	de,hl
	ld	hl,#0
	jr	8$
7$:	ld	l,-6(iy)
	ld	h,-5(iy)
8$:	pop	af
	pop	af
	ret

; de = hl:de / bc in 16 steps: the remainder in hl, the quotient shifted into de
d16:	ld	a,#16
1$:	sla	e
	rl	d
	adc	hl,hl
	jr	c,2$		; the remainder reached bit 16: it is above the divisor
	or	a,a
	sbc	hl,bc
	jr	nc,3$
	add	hl,bc
	jr	4$
2$:	or	a,a
	sbc	hl,bc
3$:	inc	e
4$:	dec	a
	jr	nz,1$
	ret

big32:	ld	a,b		; a zero divisor: all-ones quotient, the dividend as remainder
	or	a,c
	or	a,0(iy)
	or	a,1(iy)
	jr	nz,36$
	bit	0,-3(iy)
	jr	nz,37$
	ld	hl,#0xffff
	ld	de,#0xffff
	pop	af
	ret
37$:	ex	de,hl
	pop	af
	ret
36$:	ld	a,d		; a zero dividend gives 0 and 0
	or	a,e
	or	a,h
	or	a,l
	jr	nz,31$
	ld	d,h
	ld	e,l
	pop	af
	ret
31$:	push	af		; -6(iy) the number of aligning shifts
	push	af		; the divisor shifted: -8, -7, -10, -9(iy) bytes 0..3
	push	af
	push	af		; the quotient: -12, -11, -14, -13(iy)
	push	af
	xor	a,a
	ld	-6(iy),a
	ld	-12(iy),a
	ld	-11(iy),a
	ld	-14(iy),a
	ld	-13(iy),a
	ld	-8(iy),c
	ld	-7(iy),b
	ld	a,0(iy)
	ld	-10(iy),a
	ld	a,1(iy)
	ld	-9(iy),a
	call	sub32		; below the divisor: quotient 0, the dividend is the remainder
	call	add32
	jr	nc,41$
	jr	fin		; (add32 carries out exactly when sub32 borrowed)
41$:	bit	7,-9(iy)	; align the divisor to the dividend: shift while twice it still fits
	jr	nz,44$
	call	shl32d
	call	sub32
	push	af
	call	add32
	pop	af
	jr	c,43$
	inc	-6(iy)
	jr	41$
43$:	call	shr32d
44$:	call	shl32q		; one quotient bit per shift: restoring division
	call	sub32
	jr	c,46$
	set	0,-12(iy)
	jr	47$
46$:	call	add32
47$:	call	shr32d
	ld	a,-6(iy)
	sub	a,#1
	ld	-6(iy),a
	jr	nc,44$
fin:	bit	0,-3(iy)
	jr	z,48$
	ex	de,hl
	jr	49$
48$:	ld	e,-12(iy)
	ld	d,-11(iy)
	ld	l,-14(iy)
	ld	h,-13(iy)
49$:	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	pop	af
	ret

sub32:	ld	a,l
	sub	a,-8(iy)
	ld	l,a
	ld	a,h
	sbc	a,-7(iy)
	ld	h,a
	ld	a,e
	sbc	a,-10(iy)
	ld	e,a
	ld	a,d
	sbc	a,-9(iy)
	ld	d,a
	ret
add32:	ld	a,l
	add	a,-8(iy)
	ld	l,a
	ld	a,h
	adc	a,-7(iy)
	ld	h,a
	ld	a,e
	adc	a,-10(iy)
	ld	e,a
	ld	a,d
	adc	a,-9(iy)
	ld	d,a
	ret
shl32d:	sla	-8(iy)
	rl	-7(iy)
	rl	-10(iy)
	rl	-9(iy)
	ret
shr32d:	srl	-9(iy)
	rr	-10(iy)
	rr	-7(iy)
	rr	-8(iy)
	ret
shl32q:	sla	-12(iy)
	rl	-11(iy)
	rl	-14(iy)
	rl	-13(iy)
	ret

; signed: work on the magnitudes through the unsigned routine, then give the result its sign.
; dividend negate (de:hl), result negate (de low, hl high)
negdh:
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
	ret
negr:
	xor	a,a
	sub	a,e
	ld	e,a
	ld	a,#0
	sbc	a,d
	ld	d,a
	ld	a,#0
	sbc	a,l
	ld	l,a
	ld	a,#0
	sbc	a,h
	ld	h,a
	ret

; the byte kept in the frame: bit 7 the sign to give the result (quotient: xor of the signs; remainder: the dividend's),
; bit 0 set for a remainder
	.globl __divsi3
__divsi3:
	ld	iy,#2
	add	iy,sp
	ld	a,d
	xor	a,1(iy)
	and	a,#0x80
	jr	sdm
	.globl __modsi3
__modsi3:
	ld	iy,#2
	add	iy,sp
	ld	a,d
	and	a,#0x80
	or	a,#1
sdm:
	push	af		; sign and mode, A at -3(iy)
	push	af		; room for the divisor's magnitude, high word at -6(iy)
	bit	7,d
	call	nz,negdh
	bit	7,1(iy)
	jr	z,1$
	xor	a,a
	sub	a,c
	ld	c,a
	ld	a,#0
	sbc	a,b
	ld	b,a
	ld	a,#0
	sbc	a,0(iy)
	ld	-6(iy),a
	ld	a,#0
	sbc	a,1(iy)
	ld	-5(iy),a
	jr	2$
1$:	ld	a,0(iy)
	ld	-6(iy),a
	ld	a,1(iy)
	ld	-5(iy),a
2$:	bit	0,-3(iy)
	jr	nz,3$
	call	__udivsi3
	jr	4$
3$:	call	__umodsi3
4$:	pop	bc
	pop	bc
	bit	7,b
	ret	z
	jp	negr
