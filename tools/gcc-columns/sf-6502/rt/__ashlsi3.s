; SImode shift left: rc2..rc5 shifted by the count in rc6 (0..31); hand helper, not codegen
	.importzp rc2, rc3, rc4, rc5, rc6
	.export ___ashlsi3
	.segment "CODE"
___ashlsi3:
	ldx rc6
	beq @d
@l:	asl rc2
	rol rc3
	rol rc4
	rol rc5
	dex
	bne @l
@d:	rts
