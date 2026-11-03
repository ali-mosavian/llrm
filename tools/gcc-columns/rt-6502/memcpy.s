; void *memcpy (void *d, const void *s, unsigned n): d rc2:rc3, s rc4:rc5, n rc6:rc7; returns d
	.importzp rc2, rc3, rc4, rc5, rc6, rc7, rc8, rc9
	.export _memcpy
	.segment "CODE"
_memcpy:
	lda rc2
	sta rc8
	lda rc3
	sta rc9
	ldy #0
@l:	lda rc6
	ora rc7
	beq @d
	lda (rc4),y
	sta (rc8),y
	iny
	bne @n
	inc rc5
	inc rc9
@n:	lda rc6
	bne @m
	dec rc7
@m:	dec rc6
	jmp @l
@d:	rts
