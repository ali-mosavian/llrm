; binary32 add, subtract, multiply and divide for the Z80 (sdasz80), round to nearest even, denormals, inf, NaN. The llrm-z80 convention:
; first float in HL (low) and DE (high), second float's low word in BC and its high word on the stack above the return
; address, popped by the caller; result low word in DE... as a long: HL low, DE high is the argument layout, and the
; result comes back as a long: low word in DE, high word in HL. Changes A BC DE HL IY and the flags; keeps IX and SP.
; Every NaN result is the quiet NaN 0x7fc00000. The work is done in a 24-byte frame addressed through IY:
;   0..3 a   4..7 b   8 ea  9 eb  10 sign of a / of the result  11 sign of b  12 b's sign mask / na
;   13 quotient group count  14..15 product low bytes  20 nb  21 operation  22..23 mul/div exponent
; Results are rounded from a 32-bit block in d:e:h:l (pack32): leading one at bit 31, l the guard byte, sticky in its bit 0.
	.module sf32_float
	.area _CODE

	.globl __floatsisf
	.globl __floatunsisf
	.globl pack32
	.globl done
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

retzero32:
	ld	hl,#0
	ld	d,h
	ld	e,l
	jp	done

