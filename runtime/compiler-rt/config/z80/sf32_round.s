; binary32 round-to-nearest-even and repack (rnd32: exponent in c, mantissa block in d:e:h:l, leading one at bit 31, l the guard byte) and the frame exit (done).
; Apart from sf32_common so that int->float, whose exponent is always in range, links 53 B and not the denormal, inf and NaN code.
	.module sf32_round
	.area _CODE

	.globl rnd32
	.globl done
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
	ld	d,#0x80		; the mantissa wrapped: 1.0 at the next exponent (255 packs as inf)
	inc	c
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
done:	ex	de,hl		; the long result: low word in de, high word in hl
	ld	iy,#24
	add	iy,sp
	ld	sp,iy
	ret
