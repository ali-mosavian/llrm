; void *memset (void *p, int c, unsigned n): p rc2:rc3, c rc4, n rc6:rc7; returns p
	.importzp rc2, rc3, rc4, rc6, rc7, rc8, rc9
	.export _memset
	.segment "CODE"
_memset:
	lda rc2
	sta rc8
	lda rc3
	sta rc9
	ldy #0
@l:	lda rc6
	ora rc7
	beq @d
	lda rc4
	sta (rc8),y
	iny
	bne @n
	inc rc9
@n:	lda rc6
	bne @m
	dec rc7
@m:	dec rc6
	jmp @l
@d:	rts
