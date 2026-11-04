; Indirect call through hl.
	.module call_hl
	.globl	___call_hl
	.area	_CODE
___call_hl:
	jp	(hl)
