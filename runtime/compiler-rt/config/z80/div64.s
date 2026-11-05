; Hand written. void __sf_div64(void): the f64 quotient step, the SF_DIV64_Z80 hook of lib/fp/f64/div.c. Reads sf_d64_ma and sf_d64_mb (uint64_t, little
; endian), leaves q = floor(ma * 2^56 / mb) in sf_d64_q and the remainder in sf_d64_r: 57 steps of "q = q << 1 | (r >= mb); if r >= mb: r -= mb;
; r <<= 1", as the C loop does. The operands go through these cells, so the routine does not depend on the compiler's argument layout.
; r lives in hl, de, hl', de' (low word first); mb stays in memory. A step subtracts mb word by word and adds it back when the subtract borrowed;
; the quotient bit rolls into a, with a set bit as the sentinel that tells when a byte is full (the first byte holds one bit, 57 = 1 + 7 * 8); a'
; counts the stored bytes. Scratch: a, bc, de, hl, iy and the alternate set.

	.module div64
	.area _DATA
_sf_d64_ma::	.ds 8
_sf_d64_mb::	.ds 8
_sf_d64_q::	.ds 8
_sf_d64_r::	.ds 8
	.area _CODE
	.globl __sf_div64
__sf_div64::
	ld	hl,(_sf_d64_ma)
	ld	de,(_sf_d64_ma+2)
	exx
	ld	hl,(_sf_d64_ma+4)
	ld	de,(_sf_d64_ma+6)
	exx
	ld	iy,#_sf_d64_q+7
	ld	a,#8
	ex	af,af'
	ld	a,#0x80
1$:	or	a,a		; r - mb, a word at a time, the borrow ripples through
	ld	bc,(_sf_d64_mb)
	sbc	hl,bc
	ex	de,hl
	ld	bc,(_sf_d64_mb+2)
	sbc	hl,bc
	ex	de,hl
	exx
	ld	bc,(_sf_d64_mb+4)
	sbc	hl,bc
	ex	de,hl
	ld	bc,(_sf_d64_mb+6)
	sbc	hl,bc
	ex	de,hl
	exx
	jr	nc,2$		; no borrow: r >= mb, the quotient bit is 1
	ld	bc,(_sf_d64_mb)	; r < mb: add mb back, the bit is 0
	add	hl,bc
	ex	de,hl
	ld	bc,(_sf_d64_mb+2)
	adc	hl,bc
	ex	de,hl
	exx
	ld	bc,(_sf_d64_mb+4)
	adc	hl,bc
	ex	de,hl
	ld	bc,(_sf_d64_mb+6)
	adc	hl,bc
	ex	de,hl
	exx
	or	a,a
	jr	3$
2$:	scf
3$:	rla			; the bit in, the sentinel out
	ld	c,#0
	rl	c		; c = 1 when the byte is full
	add	hl,hl		; r <<= 1
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	exx
	adc	hl,hl
	ex	de,hl
	adc	hl,hl
	ex	de,hl
	exx
	bit	0,c
	jr	z,1$
	ld	0(iy),a		; a byte of q, high one first
	dec	iy
	ex	af,af'
	dec	a
	jr	z,4$
	ex	af,af'
	ld	a,#1
	jr	1$
4$:	ld	(_sf_d64_r),hl
	ld	(_sf_d64_r+2),de
	exx
	ld	(_sf_d64_r+4),hl
	ld	(_sf_d64_r+6),de
	exx
	ret
