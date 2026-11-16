; Hand written. void __sf_mul64(void): the f64 mantissa product, the SF_MUL64X64 hook of lib/fp/f64/mul64.h. Reads sf_m64_a and sf_m64_b (uint64_t, little
; endian), leaves the 128-bit product in sf_m64_lo (low half) and sf_m64_hi: sixteen 16x16 products (__umulhisi3), each added into the result at its
; word position, the carry rippling up byte by byte. The operands go through these cells, so the routine does not depend on the compiler's argument layout.
; ix walks the result words, iy the words of a; the alternate set holds the pointer into b and the inner count (__umulhisi3 keeps it).
; Scratch: a, bc, de, hl, iy and the alternate set; ix is kept.

	.module mul64
	.area _DATA
_sf_m64_a::	.ds 8
_sf_m64_b::	.ds 8
_sf_m64_lo::	.ds 8
_sf_m64_hi::	.ds 8
_sf_m64_t:	.ds 4
_sf_m64_i:	.ds 1
	.area _CODE
	.globl __sf_mul64
	.globl __umulhisi3
__sf_mul64::
	push	ix
	ld	hl,#_sf_m64_lo
	ld	de,#_sf_m64_lo+1
	ld	bc,#15
	ld	(hl),#0
	ldir			; the result starts at zero
	ld	iy,#_sf_m64_a
	ld	ix,#_sf_m64_lo
	ld	a,#4
	ld	(_sf_m64_i),a
1$:	exx
	ld	hl,#_sf_m64_b
	ld	b,#4
	exx
2$:	ld	l,0(iy)
	ld	h,1(iy)		; hl = a word
	exx
	ld	e,(hl)
	inc	hl
	ld	d,(hl)
	inc	hl
	push	de
	exx
	pop	de		; de = b word
	call	__umulhisi3	; de:hl = the product
	ld	(_sf_m64_t),hl
	ld	(_sf_m64_t+2),de
	push	ix
	pop	hl
	ld	de,#_sf_m64_t
	ld	b,#4
	or	a,a
3$:	ld	a,(de)		; four bytes of the product into the result
	adc	a,(hl)
	ld	(hl),a
	inc	de
	inc	hl
	djnz	3$
	jr	nc,5$
4$:	inc	(hl)		; the carry ripples up; the product is below 2^128, so it ends inside the result
	jr	nz,5$
	inc	hl
	jr	4$
5$:	inc	ix
	inc	ix
	exx
	dec	b
	exx
	jr	nz,2$
	ld	bc,#-6		; the next word of a starts one word above the last row
	add	ix,bc
	inc	iy
	inc	iy
	ld	hl,#_sf_m64_i
	dec	(hl)
	jr	nz,1$
	pop	ix
	ret
