; signed 16%16: rc2:rc3 = rc2:rc3 % rc4:rc5, sign of the dividend
	.importzp rc2, rc3, rc4, rc5, rc6, rc7, rc8
	.import ___udivhi3
	.export ___modhi3
	.segment "CODE"
___modhi3:
	lda rc3
	sta rc8             ; sign of the remainder in bit 7
	bpl @a
	sec
	lda #0
	sbc rc2
	sta rc2
	lda #0
	sbc rc3
	sta rc3
@a:	lda rc5
	bpl @b
	sec
	lda #0
	sbc rc4
	sta rc4
	lda #0
	sbc rc5
	sta rc5
@b:	jsr ___udivhi3
	lda rc6
	sta rc2
	lda rc7
	sta rc3
	lda rc8
	bpl @r
	sec
	lda #0
	sbc rc2
	sta rc2
	lda #0
	sbc rc3
	sta rc3
@r:	rts
