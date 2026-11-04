; unsigned 16%16: rc2:rc3 = rc2:rc3 % rc4:rc5
	.importzp rc2, rc3, rc6, rc7
	.import ___udivhi3
	.export ___umodhi3
	.segment "CODE"
___umodhi3:
	jsr ___udivhi3
	lda rc6
	sta rc2
	lda rc7
	sta rc3
	rts
