; unsigned/signed 8x8->8: rc2 = rc2 * rc3
	.importzp rc2, rc3, rc4
	.export ___mulqi3
	.segment "CODE"
___mulqi3:
	lda #0
	ldx #8
@l:	lsr rc3
	bcc @s
	clc
	adc rc2
@s:	asl rc2
	dex
	bne @l
	sta rc2
	rts
