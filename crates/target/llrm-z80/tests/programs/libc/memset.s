	.module memset
	.globl _memset, __memseta
	.area _CODE
_memset:
	ld	a,e
	ld	d,b
	ld	e,c
	jp	__memseta
