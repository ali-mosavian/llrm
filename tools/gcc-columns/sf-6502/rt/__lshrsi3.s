; SImode shift logical right: rc2..rc5 shifted by the count in rc6 (0..31); hand helper, not codegen
	.importzp rc2, rc3, rc4, rc5, rc6
	.export ___lshrsi3
	.segment "CODE"
___lshrsi3:
	ldx rc6
	beq @d
@l:	lsr rc5
	ror rc4
	ror rc3
	ror rc2
	dex
	bne @l
@d:	rts
