; void *memmove (void *d, const void *s, unsigned n): forward when d < s, else backward; returns d
	.importzp rc2, rc3, rc4, rc5, rc6, rc7, rc8, rc9, rc10, rc11
	.import _memcpy
	.export _memmove
	.segment "CODE"
_memmove:
	lda rc2
	cmp rc4
	lda rc3
	sbc rc5
	bcs @b
	jmp _memcpy
@b:	clc
	lda rc2
	adc rc6
	sta rc8
	lda rc3
	adc rc7
	sta rc9
	clc
	lda rc4
	adc rc6
	sta rc10
	lda rc5
	adc rc7
	sta rc11
	ldy #0
@l:	lda rc6
	ora rc7
	beq @d
	lda rc10
	bne @a
	dec rc11
@a:	dec rc10
	lda rc8
	bne @c
	dec rc9
@c:	dec rc8
	lda (rc10),y
	sta (rc8),y
	lda rc6
	bne @m
	dec rc7
@m:	dec rc6
	jmp @l
@d:	rts
