; Runtime for the Z80 target: what the compiler calls for multiply, divide, remainder and shifts by a variable.
; The convention is calling.toml's: a byte argument in the first free of a l h e d c b, a word in hl, de, bc; a
; byte result in a, a word in de. A routine may change a, bc, de, hl, iy and the flags; it keeps ix and sp.
; Division by zero gives the all-ones quotient and the dividend as remainder (the restoring loops, unchanged).
; Signed division truncates toward zero and the remainder has the dividend's sign.
	.module z80_rt
	.area _CODE

; ---- multiply: the low half of the product -------------------------------------------------------

; a * l -> a. Adds the multiplicand once per set bit of l and stops when l is spent.
	.globl __mulqi3
__mulqi3:
	ld	e,a
	xor	a
	jr	1$
2$:	add	a,e
3$:	sla	e
1$:	srl	l
	jr	c,2$
	jr	nz,3$
	ret

; hl * de -> de, the same way over 16 bits: hl is the multiplicand, de the multiplier.
	.globl __mulhi3
__mulhi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	jr	1$
2$:	add	hl,bc
3$:	sla	c
	rl	b
1$:	srl	d
	rr	e
	jr	c,2$
	ld	a,d
	or	a,e
	jr	nz,3$
	ex	de,hl
	ret

; ---- unsigned divide and remainder ---------------------------------------------------------------

; a / l -> a (__udivqi3), a % l -> a (__umodqi3): restoring division, one quotient bit per step.
	.globl __udivqi3
__udivqi3:
	call	__umodqi3
	ld	a,c
	ret

	.globl __umodqi3
__umodqi3:
	ld	e,l
	ld	c,a
	ld	b,#8
	xor	a
1$:	sla	c
	rla
	jr	c,2$
	cp	a,e
	jr	c,3$
2$:	sub	a,e
	inc	c
3$:	djnz	1$
	ret

; hl / de -> de (__udivhi3), hl % de -> de (__umodhi3). The division leaves the quotient in de and the remainder in
; hl; the remainder entry swaps them. Restoring division in two shapes. A divisor under 128
; keeps the remainder in a: 16 steps, the quotient shifting into hl as the dividend shifts out. A larger divisor
; leaves a quotient of at most 9 bits, so the 7 high bits of the dividend are the first remainder and 9 steps follow
; (the remainder is then at most 16 bits, never more), the dividend's low byte in a shifting out as the quotient
; shifts in.
	.globl __udivhi3
__udivhi3:
	ld	a,e
	and	a,#0x80
	or	a,d
	jr	nz,4$
	ld	b,#16
	xor	a
1$:	add	hl,hl
	rla
	cp	a,e
	jr	c,2$
	sub	a,e
	inc	l
2$:	djnz	1$
	ex	de,hl
	ld	l,a
	ld	h,#0
	ret
4$:	ld	b,#9
	ld	a,l
	ld	l,h
	ld	h,#0
	rr	l
5$:	adc	hl,hl
	sbc	hl,de
	jr	nc,6$
	add	hl,de
6$:	ccf
	rla
	djnz	5$
	rl	b
	ld	d,b
	ld	e,a
	ret

	.globl __umodhi3
__umodhi3:
	call	__udivhi3
	ex	de,hl
	ret

; ---- signed divide and remainder: the unsigned routines on magnitudes ----------------------------

; a / l and a % l, signed bytes. The quotient is negative when the signs differ, the remainder takes the dividend's.
	.globl __divqi3
__divqi3:
	ld	b,a
	xor	a,l
	ld	d,a
	ld	a,b
	call	absal
	call	__udivqi3
	bit	7,d
	ret	z
	neg
	ret

	.globl __modqi3
__modqi3:
	ld	d,a
	call	absal
	call	__umodqi3
	bit	7,d
	ret	z
	neg
	ret

; a = |a|, l = |l|; keeps d and h.
	.globl absal
absal:	or	a,a
	jp	p,1$
	neg
1$:	bit	7,l
	ret	z
	ld	b,a
	xor	a
	sub	a,l
	ld	l,a
	ld	a,b
	ret

; hl / de and hl % de, signed words.
	.globl __divhi3
__divhi3:
	ld	a,h
	xor	a,d
	push	af
	call	abshlde
	call	__udivhi3
	pop	af
	ret	p
	.globl negde
negde:	xor	a
	sub	a,e
	ld	e,a
	sbc	a,a
	sub	a,d
	ld	d,a
	ret

	.globl __modhi3
__modhi3:
	ld	a,h
	or	a,a
	push	af
	call	abshlde
	call	__umodhi3
	pop	af
	ret	p
	jr	negde

; hl = |hl|, de = |de|.
	.globl abshlde
abshlde:
	bit	7,h
	jr	z,1$
	xor	a
	sub	a,l
	ld	l,a
	sbc	a,a
	sub	a,h
	ld	h,a
1$:	bit	7,d
	ret	z
	jr	negde

; ---- shifts by a variable amount -----------------------------------------------------------------

; a << l, a >> l (logical), a >> l (arithmetic).
	.globl __ashlqi3
__ashlqi3:
	ld	b,l
	inc	b
	jr	2$
1$:	add	a,a
2$:	djnz	1$
	ret

	.globl __lshrqi3
__lshrqi3:
	ld	b,l
	inc	b
	jr	2$
1$:	srl	a
2$:	djnz	1$
	ret

	.globl __ashrqi3
__ashrqi3:
	ld	b,l
	inc	b
	jr	2$
1$:	sra	a
2$:	djnz	1$
	ret

; hl << a, hl >> a (logical), hl >> a (arithmetic) -> de.
	.globl __ashlhi3
__ashlhi3:
	ld	b,a
	inc	b
	jr	2$
1$:	add	hl,hl
2$:	djnz	1$
	ex	de,hl
	ret

	.globl __lshrhi3
__lshrhi3:
	ld	b,a
	inc	b
	jr	2$
1$:	srl	h
	rr	l
2$:	djnz	1$
	ex	de,hl
	ret

	.globl __ashrhi3
__ashrhi3:
	ld	b,a
	inc	b
	jr	2$
1$:	sra	h
	rr	l
2$:	djnz	1$
	ex	de,hl
	ret

; The first long is hl (low) and de (high), the count a byte in a; the result is de (low) and hl (high).
	.globl __ashlsi3
__ashlsi3:
	ld	b,a
	inc	b
	jr	2$
1$:	add	hl,hl
	rl	e
	rl	d
2$:	djnz	1$
	ex	de,hl
	ret

	.globl __lshrsi3
__lshrsi3:
	ld	b,a
	inc	b
	jr	2$
1$:	srl	d
	rr	e
	rr	h
	rr	l
2$:	djnz	1$
	ex	de,hl
	ret

	.globl __ashrsi3
__ashrsi3:
	ld	b,a
	inc	b
	jr	2$
1$:	sra	d
	rr	e
	rr	h
	rr	l
2$:	djnz	1$
	ex	de,hl
	ret

; ---- long divide and remainder -------------------------------------------------------------------

; The dividend is hl (low) and de (high), the divisor bc (low) and the word the caller stored on the stack (high);
; the result is de (low) and hl (high). Restoring division over a frame addressed through ix, which is kept: 32 steps,
; the quotient shifting into the dividend's place as the remainder shifts in. Mode bits in the frame: 0 remainder
; wanted, 1 signed, 2 quotient negative, 3 remainder negative.
	.globl __udivsi3
__udivsi3:
	xor	a
	jr	udivmodsi
	.globl __umodsi3
__umodsi3:
	ld	a,#1
	jr	udivmodsi
	.globl __divsi3
__divsi3:
	ld	a,#2
	jr	udivmodsi
	.globl __modsi3
__modsi3:
	ld	a,#3
udivmodsi:
	push	ix
	ld	ix,#0
	add	ix,sp
	push	af
	push	bc
	push	de
	push	hl
	ld	hl,#0
	push	hl
	push	hl
	bit	1,-1(ix)
	jr	z,loop
	bit	7,-5(ix)
	jr	z,1$
	xor	a
	sub	a,-8(ix)
	ld	-8(ix),a
	ld	a,#0
	sbc	a,-7(ix)
	ld	-7(ix),a
	ld	a,#0
	sbc	a,-6(ix)
	ld	-6(ix),a
	ld	a,#0
	sbc	a,-5(ix)
	ld	-5(ix),a
	ld	a,-1(ix)
	xor	a,#12
	ld	-1(ix),a
1$:	bit	7,5(ix)
	jr	z,loop
	xor	a
	sub	a,-4(ix)
	ld	-4(ix),a
	ld	a,#0
	sbc	a,-3(ix)
	ld	-3(ix),a
	ld	a,#0
	sbc	a,4(ix)
	ld	4(ix),a
	ld	a,#0
	sbc	a,5(ix)
	ld	5(ix),a
	ld	a,-1(ix)
	xor	a,#4
	ld	-1(ix),a
loop:	ld	b,#32
1$:	sla	-8(ix)
	rl	-7(ix)
	rl	-6(ix)
	rl	-5(ix)
	rl	-12(ix)
	rl	-11(ix)
	rl	-10(ix)
	rl	-9(ix)
	sbc	a,a
	ld	c,a
	ld	a,-12(ix)
	sub	a,-4(ix)
	ld	l,a
	ld	a,-11(ix)
	sbc	a,-3(ix)
	ld	h,a
	ld	a,-10(ix)
	sbc	a,4(ix)
	ld	e,a
	ld	a,-9(ix)
	sbc	a,5(ix)
	ld	d,a
	jr	nc,2$
	ld	a,c
	or	a,a
	jr	z,3$
2$:	ld	-12(ix),l
	ld	-11(ix),h
	ld	-10(ix),e
	ld	-9(ix),d
	inc	-8(ix)
3$:	djnz	1$
	bit	0,-1(ix)
	jr	nz,4$
	bit	2,-1(ix)
	jr	z,5$
	xor	a
	sub	a,-8(ix)
	ld	-8(ix),a
	ld	a,#0
	sbc	a,-7(ix)
	ld	-7(ix),a
	ld	a,#0
	sbc	a,-6(ix)
	ld	-6(ix),a
	ld	a,#0
	sbc	a,-5(ix)
	ld	-5(ix),a
5$:	ld	e,-8(ix)
	ld	d,-7(ix)
	ld	l,-6(ix)
	ld	h,-5(ix)
	jr	6$
4$:	bit	3,-1(ix)
	jr	z,7$
	xor	a
	sub	a,-12(ix)
	ld	-12(ix),a
	ld	a,#0
	sbc	a,-11(ix)
	ld	-11(ix),a
	ld	a,#0
	sbc	a,-10(ix)
	ld	-10(ix),a
	ld	a,#0
	sbc	a,-9(ix)
	ld	-9(ix),a
7$:	ld	e,-12(ix)
	ld	d,-11(ix)
	ld	l,-10(ix)
	ld	h,-9(ix)
6$:	ld	sp,ix
	pop	ix
	ret
