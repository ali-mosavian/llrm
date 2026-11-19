; call through a pointer: the caller stores it in __icv and does jsr __icall (the 6502 has no indirect jsr)
	.export __icall, __icv
	.segment "BSS"
__icv:	.res 2
	.segment "CODE"
__icall:
	jmp (__icv)
