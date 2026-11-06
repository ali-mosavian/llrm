; Indirect call through iy (hl carries the first word argument).
	.module call_iy
	.globl	___call_iy
	.area	_CODE
___call_iy:
	jp	(iy)
