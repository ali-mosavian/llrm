; 16x16->16: rc2:rc3 = rc2:rc3 * rc4:rc5
	.importzp rc2, rc3, rc4, rc5, rc6, rc7
	.export ___mulhi3
	.segment "CODE"
___mulhi3:
	lda #0
	sta rc6
	sta rc7
	ldx #16
@l:	lsr rc5
	ror rc4
	bcc @s
	clc
	lda rc6
	adc rc2
	sta rc6
	lda rc7
	adc rc3
	sta rc7
@s:	asl rc2
	rol rc3
	dex
	bne @l
	lda rc6
	sta rc2
	lda rc7
	sta rc3
	rts
