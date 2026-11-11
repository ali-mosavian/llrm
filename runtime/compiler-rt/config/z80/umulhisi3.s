; 16x16 -> 32 for the Z80 (sdasz80), the llrm-z80 convention; its own member: soft-float multiply links it without int32.s.
	.module umulhisi3
	.area _CODE

; hl * de -> de:hl, the full 32-bit product of two 16-bit values (de low word, hl high word, as a long result): one multiplier bit per step.
; The 32x32 -> 64 of the soft-float library is four of these; __mulsi3 would compute two cross terms against zero.
	.globl __umulhisi3
__umulhisi3:
	ld	b,h
	ld	c,l
	ld	hl,#0
	ld	a,#16
1$:	add	hl,hl		; de:hl = bc * multiplier(de), one multiplier bit per step
	rl	e
	rl	d
	jr	nc,2$
	add	hl,bc
	jr	nc,2$
	inc	de
2$:	dec	a
	jr	nz,1$
	ex	de,hl
	ret
