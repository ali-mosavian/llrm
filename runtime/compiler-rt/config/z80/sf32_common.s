; binary32 helpers shared by the sf32 members: round and pack, exponent bytes, NaN and the frame exit.
	.module sf32_common
	.area _CODE

	.globl pack32
	.globl done
	.globl retnan
	.globl inf
	.globl exps
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

retnan:	ld	hl,#0
	ld	de,#0x7fc0
done:	ex	de,hl		; the long result: low word in de, high word in hl
	ld	iy,#24
	add	iy,sp
	ld	sp,iy
	ret

