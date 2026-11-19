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
; hl (a caller that wants both takes them there: the pair, `lower_divrem`); the remainder entry swaps them. Restoring division in two shapes. A divisor under 128
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

; hl / de, signed, quotient and remainder in one call: the quotient in de, the remainder in hl. The quotient is negative
; when the signs differ, the remainder takes the dividend's.
	.globl __divremhi4
__divremhi4:
	ld	a,h
	or	a,a
	push	af
	xor	a,d
	push	af
	call	abshlde
	call	__udivhi3
	pop	af
	jp	p,1$
	call	negde
1$:	pop	af
	ret	p
	xor	a
	sub	a,l
	ld	l,a
	sbc	a,a
	sub	a,h
	ld	h,a
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

; The first long is hl (low) and de (high), the count a byte in a; the result is hl (low) and de (high).
	.globl __ashlsi3
__ashlsi3:
	ld	b,a
	inc	b
	jr	2$
1$:	add	hl,hl
	rl	e
	rl	d
2$:	djnz	1$
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
	ret

; ---- 16 x 16 -> 32 multiply ---------------------------------------------------------------------------------------

; hl * de -> hl:de, the full 32-bit product of two unsigned words (hl low, de high, as a long result): one multiplier
; bit per step, 16 steps. `__mulsi3` computes two cross terms against zero for the same product.
	.globl __umulhisi3
__umulhisi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	ld	a,#16
1$:	add	hl,hl
	rl	e
	rl	d
	jr	nc,2$
	add	hl,bc
	jr	nc,2$
	inc	de
2$:	dec	a
	jr	nz,1$
	ret

; ---- long multiply -------------------------------------------------------------------------------

; The product of two longs, low 32 bits: a is hl (low) and de (high), b bc (low) and the word the caller stored on the
; stack (high); the result is hl (low) and de (high). Shift and add over b's bits in the shadow registers, until b is
; spent (a product by 100 takes 7 passes, not 32). The return address waits in iy and the caller's word stays where it was
; (it pops it).
	.globl __mulsi3
__mulsi3:
	pop	iy
	exx
	pop	de
	or	a,a
	sbc	hl,hl
	exx
	push	de
	ld	d,b
	ld	e,c
	ld	b,h
	ld	c,l
	sbc	hl,hl
	exx
	pop	bc
1$:	srl	d
	rr	e
	exx
	rr	d
	rr	e
	jr	nc,2$
	add	hl,bc
	exx
	adc	hl,bc
	exx
2$:	sla	c
	rl	b
	exx
	rl	c
	rl	b
	ld	a,d
	or	a,e
	exx
	or	a,d
	or	a,e
	exx
	jr	nz,1$
	exx
	push	hl
	exx
	pop	de
	ex	de,hl
	push	af
	jp	(iy)

; ---- long divide and remainder -------------------------------------------------------------------

; The dividend is hl (low) and de (high), the divisor bc (low) and the word the caller stored on the stack (high);
; the result is hl (low) and de (high). The divisor is aligned under the dividend and each step takes it off where
; it fits: one step per quotient bit, in the shadow registers, over a frame addressed through ix, which is kept.
; Mode bits in the frame: 0 remainder wanted, 1 signed, 2 quotient negative, 3 remainder negative.
	.globl __udivsi3
__udivsi3:
	xor	a
	jr	__udivmodsi4
	.globl __umodsi3
__umodsi3:
	ld	a,#1
	jr	__udivmodsi4
	.globl __divsi3
__divsi3:
	ld	a,#2
	jr	__udivmodsi4
	.globl __modsi3
__modsi3:
	ld	a,#3
	.globl __udivmodsi4
__udivmodsi4:
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
; The divisor is shifted left under the dividend (while its top bit is clear and it stays no greater), then
; each step subtracts it where it fits, a quotient bit, and shifts it right: one step per quotient bit, not per
; dividend bit. The dividend (remainder) is hl low and hl' high, the divisor de and de', the quotient bc and bc'
; (the routine's own shadow set), a counts the steps left.
loop:	ld	l,-8(ix)
	ld	h,-7(ix)
	ld	e,-4(ix)
	ld	d,-3(ix)
	exx
	ld	l,-6(ix)
	ld	h,-5(ix)
	ld	e,4(ix)
	ld	d,5(ix)
	ld	bc,#0
	exx
	ld	bc,#0
	xor	a,a
; a divisor of zero: the quotient is all ones, the remainder the dividend
	ld	a,e
	or	a,d
	exx
	or	a,e
	or	a,d
	exx
	jr	nz,udm_nz
	ld	bc,#0xFFFF
	exx
	ld	bc,#0xFFFF
	exx
	jr	udm_out
udm_nz:	xor	a,a
udm_al:	exx
	bit	7,d
	exx
	jr	nz,udm_lp
	sla	e
	rl	d
	exx
	rl	e
	rl	d
	exx
	or	a,a
	sbc	hl,de
	exx
	sbc	hl,de
	exx
	jr	c,udm_ov
	add	hl,de
	exx
	adc	hl,de
	exx
	inc	a
	jr	udm_al
; the divisor passed the dividend: back one place
udm_ov:	add	hl,de
	exx
	adc	hl,de
	srl	d
	rr	e
	exx
	rr	d
	rr	e
udm_lp:	or	a,a
	sbc	hl,de
	exx
	sbc	hl,de
	exx
	jr	c,udm_no
	scf
	jr	udm_q
udm_no:	add	hl,de
	exx
	adc	hl,de
	exx
	or	a,a
udm_q:	rl	c
	rl	b
	exx
	rl	c
	rl	b
	srl	d
	rr	e
	exx
	rr	d
	rr	e
	dec	a
	jp	p,udm_lp
udm_out:	ld	-12(ix),l
	ld	-11(ix),h
	ld	-8(ix),c
	ld	-7(ix),b
	exx
	ld	-10(ix),l
	ld	-9(ix),h
	ld	-6(ix),c
	ld	-5(ix),b
udm_done:
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
5$:	ld	l,-8(ix)
	ld	h,-7(ix)
	ld	e,-6(ix)
	ld	d,-5(ix)
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
7$:	ld	l,-12(ix)
	ld	h,-11(ix)
	ld	e,-10(ix)
	ld	d,-9(ix)
6$:	ld	sp,ix
	pop	ix
	ret

; ---- long quotient and remainder in one call, the speed family (`__udivremsi4`, `__divremsi4`) ---------------------
; The arguments of the others and the address of a long for the remainder on the stack above the divisor's high word;
; the quotient is returned (de low, hl high), the remainder stored through the address. The divisor aligned under the
; dividend, one step per quotient bit, as `__udivsi3` (its own copy of the steps: a program that pairs a divide and a
; remainder and also divides alone pays for both, the plain routines are as they were).
	.globl __udivremsi4
__udivremsi4:
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
	push	hl
	call	__dvr_core
	.globl __dvr_ep
__dvr_ep:
	ld	l,6(ix)
	ld	h,7(ix)
	ld	a,-12(ix)
	ld	(hl),a
	inc	hl
	ld	a,-11(ix)
	ld	(hl),a
	inc	hl
	ld	a,-10(ix)
	ld	(hl),a
	inc	hl
	ld	a,-9(ix)
	ld	(hl),a
	ld	l,-8(ix)
	ld	h,-7(ix)
	ld	e,-6(ix)
	ld	d,-5(ix)
	ld	sp,ix
	pop	ix
	ret
; signed: the magnitudes divided, the quotient negated if the signs differ, the remainder if the dividend is negative.
; -1(ix): bit 2 the quotient is negative, bit 3 the remainder.
	.globl __divremsi4
__divremsi4:
	push	ix
	ld	ix,#0
	add	ix,sp
	xor	a
	push	af
	push	bc
	push	de
	push	hl
	ld	hl,#0
	push	hl
	push	hl
	push	hl
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
	ld	a,#12
	ld	-1(ix),a
1$:	bit	7,5(ix)
	jr	z,2$
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
2$:	call	__dvr_core
	bit	3,-1(ix)
	jr	z,3$
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
3$:	bit	2,-1(ix)
	jp	z,__dvr_ep
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
	jp	__dvr_ep
	.globl __dvr_core
__dvr_core:
dvr_lp0:	ld	l,-8(ix)
	ld	h,-7(ix)
	ld	e,-4(ix)
	ld	d,-3(ix)
	exx
	ld	l,-6(ix)
	ld	h,-5(ix)
	ld	e,4(ix)
	ld	d,5(ix)
	ld	bc,#0
	exx
	ld	bc,#0
	xor	a,a
; a divisor of zero: the quotient is all ones, the remainder the dividend
	ld	a,e
	or	a,d
	exx
	or	a,e
	or	a,d
	exx
	jr	nz,dvr_nz
	ld	bc,#0xFFFF
	exx
	ld	bc,#0xFFFF
	exx
	jr	dvr_out
dvr_nz:	xor	a,a
dvr_al:	exx
	bit	7,d
	exx
	jr	nz,dvr_lp
	sla	e
	rl	d
	exx
	rl	e
	rl	d
	exx
	or	a,a
	sbc	hl,de
	exx
	sbc	hl,de
	exx
	jr	c,dvr_ov
	add	hl,de
	exx
	adc	hl,de
	exx
	inc	a
	jr	dvr_al
; the divisor passed the dividend: back one place
dvr_ov:	add	hl,de
	exx
	adc	hl,de
	srl	d
	rr	e
	exx
	rr	d
	rr	e
dvr_lp:	or	a,a
	sbc	hl,de
	exx
	sbc	hl,de
	exx
	jr	c,dvr_no
	scf
	jr	dvr_q
dvr_no:	add	hl,de
	exx
	adc	hl,de
	exx
	or	a,a
dvr_q:	rl	c
	rl	b
	exx
	rl	c
	rl	b
	srl	d
	rr	e
	exx
	rr	d
	rr	e
	dec	a
	jp	p,dvr_lp
dvr_out:	ld	-12(ix),l
	ld	-11(ix),h
	ld	-8(ix),c
	ld	-7(ix),b
	exx
	ld	-10(ix),l
	ld	-9(ix),h
	ld	-6(ix),c
	ld	-5(ix),b
	ret

; ---- long divide and remainder, the size family (`__s_`, chosen for a function tuned for size) --------------------
; The same arguments and results as `__udivsi3` and the others. The restoring division, one step per dividend bit
; (32), over registers alone: the dividend, and the quotient as its bits leave, is de and de', the remainder hl and
; hl', the divisor bc and bc' (each pair the low word, then the high word in the other set); a counts the steps. A
; remainder that carries out of its 32 bits is above the divisor. A divisor of zero gives all ones and the dividend.
	.globl __s_udivsi3
__s_udivsi3:
	call	sdu
	exx
	push	de
	exx
	pop	hl
	ex	de,hl
	ret
	.globl __s_umodsi3
__s_umodsi3:
	call	sdu
	exx
	push	hl
	exx
	pop	de
	ret
; the divisor's high word is on the stack above the two return addresses
	.globl __s_dvcore
__s_dvcore:
sdu:	exx
	ld	hl,#4
	add	hl,sp
	ld	c,(hl)
	inc	hl
	ld	b,(hl)
	exx
sdm:	ex	de,hl
	push	hl
	exx
	pop	de
	ld	hl,#0
	exx
	ld	hl,#0
	ld	a,#32
sdm_lp:	sla	e
	rl	d
	exx
	rl	e
	rl	d
	exx
	adc	hl,hl
	exx
	adc	hl,hl
	exx
	jr	c,sdm_t
	or	a,a
	sbc	hl,bc
	exx
	sbc	hl,bc
	exx
	jr	nc,sdm_q
	add	hl,bc
	exx
	adc	hl,bc
	exx
	jr	sdm_n
sdm_t:	or	a,a
	sbc	hl,bc
	exx
	sbc	hl,bc
	exx
sdm_q:	inc	e
sdm_n:	dec	a
	jr	nz,sdm_lp
	ret
; ---- long quotient and remainder in one call --------------------------------------------------------------------------
; The arguments of the others and the address of a long for the remainder on the stack above the divisor's high word
; (that is the stack above the return address, then the divisor's high word, then the address); the quotient is
; returned (de low, hl high), the remainder stored through the address. Over the same register-only core as the size
; family: a program that pairs a divide and a remainder pays for this one routine and the core, no frame routine.
	.globl __s_udivremsi4
__s_udivremsi4:
	call	sdu
	push	de
	push	hl
	ld	hl,#8
	add	hl,sp
	ld	c,(hl)
	inc	hl
	ld	b,(hl)
	pop	de
	ld	a,e
	ld	(bc),a
	inc	bc
	ld	a,d
	ld	(bc),a
	inc	bc
	exx
	push	hl
	push	de
	exx
	pop	hl
	pop	de
	ld	a,e
	ld	(bc),a
	inc	bc
	ld	a,d
	ld	(bc),a
	pop	de
	ex	de,hl
	ret
; signed: the magnitudes divided, the quotient negated if the signs differ, the remainder if the dividend is negative.
; -1(ix): bit 0 the quotient is negative, bit 1 the remainder is.
	.globl __s_divremsi4
__s_divremsi4:
	push	ix
	ld	ix,#0
	add	ix,sp
	xor	a
	push	af
	bit	7,d
	jr	z,1$
	xor	a
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
	ld	a,#3
	ld	-1(ix),a
1$:	bit	7,5(ix)
	jr	z,2$
	xor	a
	sub	a,c
	ld	c,a
	ld	a,#0
	sbc	a,b
	ld	b,a
	ld	a,#0
	sbc	a,4(ix)
	ld	4(ix),a
	ld	a,#0
	sbc	a,5(ix)
	ld	5(ix),a
	ld	a,-1(ix)
	xor	a,#1
	ld	-1(ix),a
2$:	exx
	ld	c,4(ix)
	ld	b,5(ix)
	exx
	call	sdm
	bit	1,-1(ix)
	jr	z,3$
	ld	c,l
	ld	b,h
	ld	hl,#0
	or	a,a
	sbc	hl,bc
	exx
	ld	c,l
	ld	b,h
	ld	hl,#0
	sbc	hl,bc
	exx
3$:	ld	c,6(ix)
	ld	b,7(ix)
	ld	a,l
	ld	(bc),a
	inc	bc
	ld	a,h
	ld	(bc),a
	exx
	ld	c,6(ix)
	ld	b,7(ix)
	inc	bc
	inc	bc
	ld	a,l
	ld	(bc),a
	inc	bc
	ld	a,h
	ld	(bc),a
	push	de
	exx
	pop	hl
	bit	0,-1(ix)
	jr	z,4$
	xor	a
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
4$:	ex	de,hl
	ld	sp,ix
	pop	ix
	ret
; a signed divide takes the magnitudes, divides, and gives the result the sign C gives it. -1(ix): bit 0 the
; remainder is wanted, bit 1 the result is negative.
	.globl __s_divsi3
__s_divsi3:
	xor	a
	jr	sds
	.globl __s_modsi3
__s_modsi3:
	ld	a,#1
sds:	push	ix
	ld	ix,#0
	add	ix,sp
	push	af
	bit	7,d
	jr	z,1$
	xor	a
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
	ld	a,-1(ix)
	xor	a,#2
	ld	-1(ix),a
1$:	bit	7,5(ix)
	jr	z,2$
	xor	a
	sub	a,c
	ld	c,a
	ld	a,#0
	sbc	a,b
	ld	b,a
	ld	a,#0
	sbc	a,4(ix)
	ld	4(ix),a
	ld	a,#0
	sbc	a,5(ix)
	ld	5(ix),a
	bit	0,-1(ix)
	jr	nz,2$
	ld	a,-1(ix)
	xor	a,#2
	ld	-1(ix),a
2$:	exx
	ld	c,4(ix)
	ld	b,5(ix)
	exx
	call	sdm
	bit	0,-1(ix)
	jr	nz,3$
	exx
	push	de
	exx
	pop	hl
	jr	4$
3$:	ex	de,hl
	exx
	push	hl
	exx
	pop	hl
4$:	bit	1,-1(ix)
	jr	z,5$
	xor	a
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
5$:	ex	de,hl
	ld	sp,ix
	pop	ix
	ret

; ---- 64-bit divide, remainder and shifts ----------------------------------------------------------
; A long long travels as four words, low first. The result is written through hl (the hidden pointer, also
; returned in de); the first operand's words are de, bc, 4(ix), 6(ix) and the second's the next four words on the
; stack (8(ix) up) or, for a shift, a count byte at 8(ix). The caller pops its arguments.

; The mode is in a: 0 quotient of unsigned, 1 remainder of unsigned, 2 quotient of signed, 3 remainder of signed;
; bit 4 both (`finpair64`).
; The size family (`__s_`, chosen for a function tuned for size) is the restoring division over a frame: the dividend shifts left through the remainder, 64 steps, the quotient bit
; entering at the bottom. Frame: -16..-9 quotient/dividend, -8..-1 remainder, -17 mode (bit 2 quotient negative,
; bit 3 remainder negative), -18 steps left, -19 carry out of the remainder, -21,-20 result pointer.
	.globl __s_udivmddi4
__s_udivmddi4:
	ld	a,#0x10
	jr	udivmoddi_s
	.globl __s_divmddi4
__s_divmddi4:
	ld	a,#0x12
	jr	udivmoddi_s
	.globl __s_udivdi3
__s_udivdi3:
	xor	a
	jr	udivmoddi_s
	.globl __s_umoddi3
__s_umoddi3:
	ld	a,#1
	jr	udivmoddi_s
	.globl __s_divdi3
__s_divdi3:
	ld	a,#2
	jr	udivmoddi_s
	.globl __s_moddi3
__s_moddi3:
	ld	a,#3
udivmoddi_s:
	push	ix
	ld	ix,#0
	add	ix,sp
	ld	iy,#-21
	add	iy,sp
	ld	sp,iy
	ld	-21(ix),l
	ld	-20(ix),h
	ld	-17(ix),a
	ld	-16(ix),e
	ld	-15(ix),d
	ld	-14(ix),c
	ld	-13(ix),b
	ld	a,4(ix)
	ld	-12(ix),a
	ld	a,5(ix)
	ld	-11(ix),a
	ld	a,6(ix)
	ld	-10(ix),a
	ld	a,7(ix)
	ld	-9(ix),a
	xor	a
	ld	b,#8
	push	ix
	pop	hl
	ld	de,#-8
	add	hl,de
1$:	ld	(hl),a
	inc	hl
	djnz	1$
	bit	1,-17(ix)
	jr	z,loop64s
	bit	7,-9(ix)
	jr	z,2$
	push	ix
	pop	hl
	ld	de,#-16
	add	hl,de
	call	neg64
	ld	a,-17(ix)
	xor	a,#12
	ld	-17(ix),a
2$:	bit	7,15(ix)
	jr	z,loop64s
	push	ix
	pop	hl
	ld	de,#8
	add	hl,de
	call	neg64
	ld	a,-17(ix)
	xor	a,#4
	ld	-17(ix),a
loop64s:
	.globl __s_slow
__s_slow:
d64_slow:
	ld	-18(ix),#64
3$:	push	ix
	pop	hl
	ld	de,#-16
	add	hl,de
	sla	(hl)
	ld	b,#15
4$:	inc	hl
	rl	(hl)
	djnz	4$
	sbc	a,a
	ld	-19(ix),a
	push	ix
	pop	hl
	ld	de,#8
	add	hl,de
	ex	de,hl
	push	ix
	pop	hl
	ld	bc,#-8
	add	hl,bc
	or	a,a
	ld	b,#8
5$:	ld	a,(hl)
	ex	de,hl
	sbc	a,(hl)
	ex	de,hl
	ld	(hl),a
	inc	hl
	inc	de
	djnz	5$
	jr	nc,7$
	ld	a,-19(ix)
	or	a,a
	jr	nz,7$
	ld	bc,#-8
	add	hl,bc
	ex	de,hl
	add	hl,bc
	ex	de,hl
	or	a,a
	ld	b,#8
6$:	ld	a,(de)
	adc	a,(hl)
	ld	(hl),a
	inc	hl
	inc	de
	djnz	6$
	jr	8$
7$:	inc	-16(ix)
8$:	dec	-18(ix)
	jp	nz,3$
fin64:	push	ix
	pop	hl
	ld	de,#-16
	add	hl,de
	bit	0,-17(ix)
	jr	z,9$
	ld	de,#8
	add	hl,de
	bit	3,-17(ix)
	jr	10$
9$:	bit	2,-17(ix)
10$:	jr	z,11$
	call	neg64
	ld	bc,#-8
	add	hl,bc
11$:	bit	4,-17(ix)
	jr	nz,finpair64
	ld	bc,#8
fincopy64:
	ld	e,-21(ix)
	ld	d,-20(ix)
	ldir
	ld	e,-21(ix)
	ld	d,-20(ix)
	ld	sp,ix
	pop	ix
	ret
; the pair, the quotient made signed and hl at it: the remainder made signed, the 16 bytes copied to the cell
finpair64:
	ld	de,#8
	add	hl,de
	bit	3,-17(ix)
	jr	z,1$
	call	neg64
	ld	bc,#-8
	add	hl,bc
1$:	ld	bc,#-8
	add	hl,bc
	ld	bc,#16
	jr	fincopy64

; Negates the eight bytes at hl in place; hl is left at their end.
neg64:
	ld	b,#8
	or	a,a
1$:	ld	a,#0
	sbc	a,(hl)
	ld	(hl),a
	inc	hl
	djnz	1$
	ret

; The speed family keeps the same frame and result; a divisor of 32 bits or less runs in registers, so the
; dividend's leading zeros cost a shift each and not a quotient step. The size family does not reach this code.
; The names differ from `__udivmodsi4` and `__divremsi4` within 9 characters: the link map cuts a symbol there.
; The pair (`__udivmddi4`, `__divmddi4`, mode bit 4): hl points at a cell of 16 bytes, the quotient in the first
; eight and the remainder in the last eight.
	.globl __udivmddi4
__udivmddi4:
	ld	a,#0x10
	jr	udivmoddi
	.globl __divmddi4
__divmddi4:
	ld	a,#0x12
	jr	udivmoddi
	.globl __udivdi3
__udivdi3:
	xor	a
	jr	udivmoddi
	.globl __umoddi3
__umoddi3:
	ld	a,#1
	jr	udivmoddi
	.globl __divdi3
__divdi3:
	ld	a,#2
	jr	udivmoddi
	.globl __moddi3
__moddi3:
	ld	a,#3
udivmoddi:
	push	ix
	ld	ix,#0
	add	ix,sp
	ld	iy,#-21
	add	iy,sp
	ld	sp,iy
	ld	-21(ix),l
	ld	-20(ix),h
	ld	-17(ix),a
	ld	-16(ix),e
	ld	-15(ix),d
	ld	-14(ix),c
	ld	-13(ix),b
	ld	a,4(ix)
	ld	-12(ix),a
	ld	a,5(ix)
	ld	-11(ix),a
	ld	a,6(ix)
	ld	-10(ix),a
	ld	a,7(ix)
	ld	-9(ix),a
	xor	a
	ld	b,#8
	push	ix
	pop	hl
	ld	de,#-8
	add	hl,de
1$:	ld	(hl),a
	inc	hl
	djnz	1$
	bit	1,-17(ix)
	jr	z,loop64
	bit	7,-9(ix)
	jr	z,2$
	push	ix
	pop	hl
	ld	de,#-16
	add	hl,de
	call	neg64
	ld	a,-17(ix)
	xor	a,#12
	ld	-17(ix),a
2$:	bit	7,15(ix)
	jr	z,loop64
	push	ix
	pop	hl
	ld	de,#8
	add	hl,de
	call	neg64
	ld	a,-17(ix)
	xor	a,#4
	ld	-17(ix),a
loop64:
	ld	a,12(ix)
	or	a,13(ix)
	or	a,14(ix)
	or	a,15(ix)
	jp	nz,d64_big
; a divisor of 32 bits or less: the dividend (hl de, then hl' de') shifts left through a
; 32-bit remainder (bc' low, bc high), the quotient bits entering below it; its leading zeros are skipped. The
; steps left are at -18(ix)
d64_small:
	ld	-18(ix),#64
	ld	l,-16(ix)
	ld	h,-15(ix)
	ld	e,-14(ix)
	ld	d,-13(ix)
	exx
	ld	l,-12(ix)
	ld	h,-11(ix)
	ld	e,-10(ix)
	ld	d,-9(ix)
	ld	bc,#0
	exx
	ld	bc,#0
d64_sk:	exx
	bit	7,d
	exx
	jr	nz,d64_st
	add	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	exx
	adc	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	exx
	dec	-18(ix)
	jr	nz,d64_sk
	jr	d64_fn
d64_st:	add	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	exx
	adc	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	rl	c
	rl	b
	exx
	rl	c
	rl	b
	jr	c,d64_ov
	push	bc
	exx
	push	bc
	ld	a,c
	sub	a,8(ix)
	ld	c,a
	ld	a,b
	sbc	a,9(ix)
	ld	b,a
	exx
	ld	a,c
	sbc	a,10(ix)
	ld	c,a
	ld	a,b
	sbc	a,11(ix)
	ld	b,a
	jr	c,d64_no
	pop	af
	pop	af
	inc	l
	jr	d64_nx
d64_no:	exx
	pop	bc
	exx
	pop	bc
d64_nx:	dec	-18(ix)
	jp	nz,d64_st
d64_fn:	ld	-16(ix),l
	ld	-15(ix),h
	ld	-14(ix),e
	ld	-13(ix),d
	ld	-6(ix),c
	ld	-5(ix),b
	exx
	ld	-12(ix),l
	ld	-11(ix),h
	ld	-10(ix),e
	ld	-9(ix),d
	ld	-8(ix),c
	ld	-7(ix),b
	jp	fin64
; the remainder passed 32 bits (a divisor over 2^31): it is the larger, so the divisor comes off whatever the borrow
d64_ov:	exx
	ld	a,c
	sub	a,8(ix)
	ld	c,a
	ld	a,b
	sbc	a,9(ix)
	ld	b,a
	exx
	ld	a,c
	sbc	a,10(ix)
	ld	c,a
	ld	a,b
	sbc	a,11(ix)
	ld	b,a
	inc	l
	jr	d64_nx
; a divisor of more than 32 bits: over a dividend of 32 bits or less the quotient is zero and the remainder the
; dividend; otherwise the restoring loop, one step per dividend bit
d64_big:
	ld	a,-12(ix)
	or	a,-11(ix)
	or	a,-10(ix)
	or	a,-9(ix)
	jp	nz,d64_slow
	ld	a,-16(ix)
	ld	-8(ix),a
	ld	-16(ix),#0
	ld	a,-15(ix)
	ld	-7(ix),a
	ld	-15(ix),#0
	ld	a,-14(ix)
	ld	-6(ix),a
	ld	-14(ix),#0
	ld	a,-13(ix)
	ld	-5(ix),a
	ld	-13(ix),#0
	jp	fin64

; A shift of a long long by the count byte at 8(ix), left, logical right or arithmetic right (a = 0, 1, 2): the
; operand is written to the result, shifted whole bytes with ldir/lddr, then the rest a bit at a time. Frame: -2,-1
; the result pointer, -3 mode, -4 fill byte, -5 bits left, -6 bytes left.
	.globl __ashldi3
__ashldi3:
	xor	a
	jr	shiftdi
	.globl __lshrdi3
__lshrdi3:
	ld	a,#1
	jr	shiftdi
	.globl __ashrdi3
__ashrdi3:
	ld	a,#2
shiftdi:
	push	ix
	ld	ix,#0
	add	ix,sp
	push	hl
	ld	(hl),e
	inc	hl
	ld	(hl),d
	inc	hl
	ld	(hl),c
	inc	hl
	ld	(hl),b
	inc	hl
	ld	e,4(ix)
	ld	(hl),e
	inc	hl
	ld	e,5(ix)
	ld	(hl),e
	inc	hl
	ld	e,6(ix)
	ld	(hl),e
	inc	hl
	ld	e,7(ix)
	ld	(hl),e
	ld	d,a
	cp	a,#2
	ld	a,#0
	jr	nz,1$
	ld	a,e
	rla
	sbc	a,a
1$:	ld	e,a
	push	de
	ld	a,8(ix)
	and	a,#7
	ld	b,a
	ld	a,8(ix)
	rrca
	rrca
	rrca
	and	a,#0x1F
	ld	c,a
	push	bc
2$:	ld	a,-6(ix)
	or	a,a
	jr	z,5$
	dec	-6(ix)
	ld	l,-2(ix)
	ld	h,-1(ix)
	ld	a,-3(ix)
	or	a,a
	jr	nz,3$
	ld	de,#7
	add	hl,de
	ld	e,l
	ld	d,h
	dec	hl
	ld	bc,#7
	lddr
	xor	a
	ld	(de),a
	jr	2$
3$:	ld	e,l
	ld	d,h
	inc	hl
	ld	bc,#7
	ldir
	ld	a,-4(ix)
	ld	(de),a
	jr	2$
5$:	ld	a,-5(ix)
	or	a,a
	jr	z,9$
6$:	ld	l,-2(ix)
	ld	h,-1(ix)
	ld	a,-3(ix)
	or	a,a
	jr	nz,7$
	sla	(hl)
	ld	b,#7
12$:	inc	hl
	rl	(hl)
	djnz	12$
	jr	8$
7$:	ld	de,#7
	add	hl,de
	dec	a
	jr	nz,10$
	srl	(hl)
	jr	11$
10$:	sra	(hl)
11$:	ld	b,#7
13$:	dec	hl
	rr	(hl)
	djnz	13$
8$:	dec	-5(ix)
	jr	nz,6$
9$:	ld	e,-2(ix)
	ld	d,-1(ix)
	ld	sp,ix
	pop	ix
	ret

; The same shifts for a function tuned for size: the operand is written to the result, then shifted one bit at a time,
; the count byte at 8(ix) times (a = 0, 1, 2 as above; no whole-byte moves, so a long count is slow). Frame: -2,-1
; the result pointer, -3 mode.
	.globl __s_ashldi3
__s_ashldi3:
	xor	a
	jr	sshiftdi
	.globl __s_lshrdi3
__s_lshrdi3:
	ld	a,#1
	jr	sshiftdi
	.globl __s_ashrdi3
__s_ashrdi3:
	ld	a,#2
sshiftdi:
	push	ix
	ld	ix,#0
	add	ix,sp
	push	hl
	push	af
	ld	(hl),e
	inc	hl
	ld	(hl),d
	inc	hl
	ld	(hl),c
	inc	hl
	ld	(hl),b
	inc	hl
	ex	de,hl
	push	ix
	pop	hl
	ld	bc,#4
	add	hl,bc
	ldir
1$:	ld	a,8(ix)
	or	a,a
	jr	z,9$
	dec	a
	ld	8(ix),a
	ld	l,-2(ix)
	ld	h,-1(ix)
	ld	b,#7
	ld	a,-3(ix)
	or	a,a
	jr	nz,3$
	sla	(hl)
2$:	inc	hl
	rl	(hl)
	djnz	2$
	jr	1$
3$:	ld	de,#7
	add	hl,de
	dec	a
	jr	nz,4$
	srl	(hl)
	jr	5$
4$:	sra	(hl)
5$:	dec	hl
	rr	(hl)
	djnz	5$
	jr	1$
9$:	ld	e,-2(ix)
	ld	d,-1(ix)
	ld	sp,ix
	pop	ix
	ret

; ---- the compiler's library calls -----------------------------------------------------------------
; Reserved `__` names: a program may define its own strlen, memcpy, memmove or memset and none of them clashes.
; tests/programs/libc/ has the C library entries over these.

; The length of the string at hl -> de. cpir scans from a count of 0 (the 65536 bytes it can take) for the zero byte;
; the count is then minus the bytes scanned, the zero included, and the length is its complement.
	.globl __strlen
__strlen:
	xor	a
	ld	b,a
	ld	c,a
	cpir
	ld	a,c
	cpl
	ld	e,a
	ld	a,b
	cpl
	ld	d,a
	ret

; memcpy (dst hl, src de, n bc) -> dst in de; the areas do not overlap.
	.globl __memcpy
__memcpy:
	push	hl
	ex	de,hl
	ld	a,b
	or	a,c
	jr	z,1$
	ldir
1$:
	pop	de
	ret

; memmove (dst hl, src de, n bc) -> dst in de: forward when dst is below src, else from the ends.
	.globl __memmove
__memmove:
	push	hl
	ld	a,b
	or	a,c
	jr	z,2$
	ld	a,l
	sub	a,e
	ld	a,h
	sbc	a,d
	jr	c,3$
	ex	de,hl
	add	hl,bc
	dec	hl
	ex	de,hl
	add	hl,bc
	dec	hl
	ex	de,hl
	lddr
	jr	2$
3$:
	ex	de,hl
	ldir
2$:
	pop	de
	ret

; memset (dst hl, value a, n de) -> dst in de: the first byte, then ldir copies it forward.
	.globl __memseta
__memseta:
	push	hl
	ld	b,a
	ld	a,d
	or	a,e
	jr	z,4$
	ld	a,b
	ld	(hl),a
	dec	de
	ld	a,d
	or	a,e
	jr	z,4$
	ld	c,e
	ld	b,d
	ld	e,l
	ld	d,h
	inc	de
	ldir
4$:
	pop	de
	ret

; A call through a pointer: the address is in iy.
	.globl __call_iy
__call_iy:
	jp (iy)

; Ends the last routine: measure.py sizes a routine up to the next symbol.
	.globl __rt_end
__rt_end:
