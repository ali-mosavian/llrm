; unsigned 16/16: quotient rc2:rc3 = rc2:rc3 / rc4:rc5, remainder rc6:rc7
	.importzp rc2, rc3, rc4, rc5, rc6, rc7
	.export ___udivhi3
	.segment "CODE"
___udivhi3:
	lda #0
	sta rc6
	sta rc7
	ldx #16
@l:	asl rc2
	rol rc3
	rol rc6
	rol rc7
	sec
	lda rc6
	sbc rc4
	tay
	lda rc7
	sbc rc5
	bcc @s
	sty rc6
	sta rc7
	inc rc2
@s:	dex
	bne @l
	rts
