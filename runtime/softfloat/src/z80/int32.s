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
	ld	a,d
	or	a,e
	jr	nz,10$
	ld	a,0(iy)
	or	a,1(iy)
	jr	nz,10$
	ld	a,b
	or	a,c
	jr	z,10$		; a zero divisor takes the long loop: its quotient has all 32 bits set
	ex	de,hl		; de = dividend, hl = remainder
	ld	hl,#0
	ld	a,#16
1$:	sla	e
	rl	d
	adc	hl,hl
	jr	c,2$
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
	bit	0,-3(iy)
	jr	z,5$
	ex	de,hl
5$:	ld	hl,#0
	pop	af
	ret
10$:	push	hl		; -6(iy) the dividend, low word first in memory
	push	de		; -8(iy)
	push	af		; -10(iy) the step count
	ld	-10(iy),#32
	ld	hl,#0
	ld	de,#0
11$:	sla	-6(iy)
	rl	-5(iy)
	rl	-8(iy)
	rl	-7(iy)
	adc	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	jr	c,14$		; the remainder reached bit 32: it is above the divisor
	or	a,a
	sbc	hl,bc
	ld	a,e
	sbc	a,0(iy)
	ld	e,a
	ld	a,d
	sbc	a,1(iy)
	ld	d,a
	jr	nc,15$
	add	hl,bc		; too far: put it back
	ld	a,e
	adc	a,0(iy)
	ld	e,a
	ld	a,d
	adc	a,1(iy)
	ld	d,a
	jr	16$
14$:	or	a,a
	sbc	hl,bc
	ld	a,e
	sbc	a,0(iy)
	ld	e,a
	ld	a,d
	sbc	a,1(iy)
	ld	d,a
15$:	set	0,-6(iy)
16$:	dec	-10(iy)
	jr	nz,11$
	bit	0,-3(iy)
	jr	z,17$
	ex	de,hl
	jr	18$
17$:	ld	e,-6(iy)
	ld	d,-5(iy)
	ld	l,-8(iy)
	ld	h,-7(iy)
18$:	pop	af
	pop	af
	pop	af
	pop	af
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
