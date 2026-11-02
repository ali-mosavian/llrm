.text
.globl __adddf3
__adddf3:
	fmove.d 4(%sp),%fp0
	fadd.d 12(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __addsf3
__addsf3:
	fmove.s 4(%sp),%fp0
	fadd.s 8(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __subdf3
__subdf3:
	fmove.d 4(%sp),%fp0
	fsub.d 12(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __subsf3
__subsf3:
	fmove.s 4(%sp),%fp0
	fsub.s 8(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __muldf3
__muldf3:
	fmove.d 4(%sp),%fp0
	fmul.d 12(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __mulsf3
__mulsf3:
	fmove.s 4(%sp),%fp0
	fmul.s 8(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __divdf3
__divdf3:
	fmove.d 4(%sp),%fp0
	fdiv.d 12(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __divsf3
__divsf3:
	fmove.s 4(%sp),%fp0
	fdiv.s 8(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __extendsfdf2
__extendsfdf2:
	fmove.s 4(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __truncdfsf2
__truncdfsf2:
	fmove.d 4(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __floatsidf
__floatsidf:
	fmove.l 4(%sp),%fp0
	fmove.d %fp0,-(%sp)
	movem.l (%sp)+,%d0-%d1
	rts
.globl __floatsisf
__floatsisf:
	fmove.l 4(%sp),%fp0
	fmove.s %fp0,-(%sp)
	move.l (%sp)+,%d0
	rts
.globl __fixdfsi
__fixdfsi:
	fmove.d 4(%sp),%fp0
	fintrz.x %fp0,%fp0
	fmove.l %fp0,%d0
	rts
.globl __eqdf2
__eqdf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __nedf2
__nedf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __ltdf2
__ltdf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __ledf2
__ledf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __gtdf2
__gtdf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __gedf2
__gedf2:
	fmove.d 4(%sp),%fp0
	fcmp.d 12(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __eqsf2
__eqsf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __nesf2
__nesf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __ltsf2
__ltsf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __lesf2
__lesf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __gtsf2
__gtsf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
.globl __gesf2
__gesf2:
	fmove.s 4(%sp),%fp0
	fcmp.s 8(%sp),%fp0
	fblt 1f
	fbgt 2f
	moveq #0,%d0
	bra 3f
1:	moveq #-1,%d0
	bra 3f
2:	moveq #1,%d0
3:
	rts
