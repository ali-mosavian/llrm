| IEEE-754 binary64 for the m68k: add, sub, mul, div; round to nearest even, denormals, inf, NaN as sf64.c.
| Hand written; no libgcc source is copied (libgcc is only a binary comparison column).
| libgcc's ABI: arguments on the stack (a hi, a lo, b hi, b lo), result in d0:d1, d0-d1/a0-a1 scratch.
| Base body 68000/68010; --defsym M68020=1 selects bfffo and the 64-bit mulu.l/divu.l (68020/030/040);
| add --defsym M68060=1 for the 68060, which traps on the 64-bit forms: mul and div keep the 16-bit cores.
| Mantissas are top-aligned in 64 bits (implicit bit 63, m << 11); packing takes m >> 11 with the implicit bit at 52,
| a 32-bit guard word (round bit 31, sticky below) and e - 1, so a rounding carry moves into the exponent and reaches inf.

	.text
.ifdef M68020
.ifndef M68060
WIDE = 1
.endif
.endif

SAVED = 40				| movem.l %d2-%d7/%a2-%a5

| hi:lo <<= 11 (t scratch)
	.macro	shl11 hi, lo, t
	lsl.l	#8,\hi
	lsl.l	#3,\hi
	move.l	\lo,\t
	rol.l	#8,\t
	rol.l	#3,\t
	move.l	\t,\lo
	and.w	#0xF800,\lo
	eor.l	\lo,\t
	or.l	\t,\hi
	.endm

| \e = the exponent field of \hi
	.macro	expo hi, e
	move.l	\hi,\e
	add.l	\e,\e
	swap	\e
	lsr.w	#5,\e
	.endm

| Z set if hi:lo (exponent field 0) is zero
	.macro	mant0 hi, lo, t
	move.l	\hi,\t
	and.l	#0xFFFFF,\t
	or.l	\lo,\t
	.endm

| ---- add, sub ----

	.globl	__subdf3
	.type	__subdf3,@function
__subdf3:
	move.l	4(%sp),%d0
	move.l	12(%sp),%d1
	bchg	#31,%d1
	bra.s	.Ladd
	.size	__subdf3,.-__subdf3

	.globl	__adddf3
	.type	__adddf3,@function
__adddf3:
	move.l	4(%sp),%d0
	move.l	12(%sp),%d1
.Ladd:					| d0 = a hi, d1 = b hi (negated for sub); exponents 55 or more apart return the bigger
	move.l	%d0,%a0
	move.l	%d1,%a1
	add.l	%d0,%d0
	add.l	%d1,%d1
	cmp.l	%d1,%d0
	bcs.s	1f
	cmp.l	#0xFFE00000,%d0
	bcc.s	.Laslow
	sub.l	%d1,%d0
	cmp.l	#55<<21,%d0
	bcs.s	.Laslow
	move.l	%a0,%d0
	move.l	8(%sp),%d1
	rts
1:	cmp.l	#0xFFE00000,%d1
	bcc.s	.Laslow
	sub.l	%d0,%d1
	cmp.l	#55<<21,%d1
	bcs.s	.Laslow
	move.l	%a1,%d0
	move.l	16(%sp),%d1
	rts

.Laslow:
	movem.l	%d2-%d7,-(%sp)
	move.l	%a0,%d0
	move.l	32(%sp),%d1
	move.l	%a1,%d2
	move.l	40(%sp),%d3
	move.l	%d0,%d4
	add.l	%d4,%d4
	move.l	%d2,%d5
	add.l	%d5,%d5
	cmp.l	%d5,%d4
	bne.s	1f
	cmp.l	%d3,%d1
1:	bcc.s	2f
	exg	%d0,%d2
	exg	%d1,%d3
	exg	%d4,%d5
2:	cmp.l	#0xFFE00000,%d4		| d0:d1 = big, d2:d3 = small, d4/d5 = their hi words << 1
	bcc	.Laspec
	move.l	%d5,%d6
	or.l	%d3,%d6
	beq	.Lasz			| small is 0
	move.l	%d0,%a0			| sign
	move.l	%d0,%d6
	eor.l	%d2,%d6
	move.l	%d6,%a1			| bit 31: subtract
	swap	%d4
	lsr.w	#5,%d4			| ea
	beq	.Laden
	and.l	#0xFFFFF,%d0
	bset	#20,%d0
	and.l	#0xFFFFF,%d2
	swap	%d5
	lsr.w	#5,%d5
	beq.s	3f
	bset	#20,%d2
	bra.s	4f
3:	moveq	#1,%d5
4:	sub.w	%d4,%d5
	neg.w	%d5			| ea - eb, 0..54
	moveq	#0,%d6			| guard
	cmp.w	#32,%d5
	bcs.s	5f
	move.l	%d3,%d6
	move.l	%d2,%d3
	moveq	#0,%d2
	sub.w	#32,%d5
	move.l	%d6,%d7
	lsr.l	%d5,%d6
	ror.l	%d5,%d7
	eor.l	%d6,%d7
	sne	%d7
	neg.b	%d7
	or.b	%d7,%d6			| sticky
5:	move.l	%d3,%d7
	lsr.l	%d5,%d3
	ror.l	%d5,%d7
	eor.l	%d3,%d7
	or.l	%d7,%d6
	move.l	%d2,%d7
	lsr.l	%d5,%d2
	ror.l	%d5,%d7
	eor.l	%d2,%d7
	or.l	%d7,%d3
	move.l	%a1,%d7
	bmi.s	.Lasub
	add.l	%d3,%d1
	addx.l	%d2,%d0
	btst	#21,%d0
	beq.s	.Laround
	lsr.l	#1,%d0
	roxr.l	#1,%d1
	roxr.l	#1,%d6
	bcc.s	6f
	or.b	#1,%d6
6:	addq.w	#1,%d4
	cmp.w	#2047,%d4
	beq.s	.Lainf
.Laround:				| d0:d1 = m (implicit bit 52, or a denormal with ea = 1), d6 = guard, d4.w = ea, a0 = sign
	add.l	%d6,%d6
	bcc.s	1f
	beq.s	3f
	addq.l	#1,%d1
	bcc.s	1f
	addq.l	#1,%d0
1:	subq.w	#1,%d4
	swap	%d4
	clr.w	%d4
	lsl.l	#4,%d4
	add.l	%d4,%d0
	move.l	%a0,%d7
	bpl.s	.Laret
	bset	#31,%d0
.Laret:
	movem.l	(%sp)+,%d2-%d7
	rts
3:	addq.l	#1,%d1			| tie: to even
	bcc.s	4f
	addq.l	#1,%d0
4:	bclr	#0,%d1
	bra.s	1b

.Lainf:
	move.l	%a0,%d0
	and.l	#0x80000000,%d0
	or.l	#0x7FF00000,%d0
	moveq	#0,%d1
	bra.s	.Laret

.Lasub:
	neg.l	%d6
	subx.l	%d3,%d1
	subx.l	%d2,%d0
	btst	#20,%d0
	bne.s	.Laround
	move.l	%d0,%d7
	or.l	%d1,%d7
	or.l	%d6,%d7
	beq.s	.Lazero
1:	cmp.w	#1,%d4			| renormalize; a denormal stops at ea = 1
	beq.s	.Laround
	subq.w	#1,%d4
	add.l	%d6,%d6
	addx.l	%d1,%d1
	addx.l	%d0,%d0
	btst	#20,%d0
	beq.s	1b
	bra.s	.Laround
.Lazero:
	moveq	#0,%d0
	moveq	#0,%d1
	bra.s	.Laret

.Laden:					| both exponent fields 0: the integer sum is exact
	bclr	#31,%d2
	move.l	%a1,%d7
	bmi.s	1f
	add.l	%d3,%d1
	addx.l	%d2,%d0
	bra.s	.Laret
1:	sub.l	%d3,%d1
	subx.l	%d2,%d0
	move.l	%d0,%d7
	add.l	%d7,%d7
	or.l	%d1,%d7
	bne.s	.Laret
	moveq	#0,%d0
	bra.s	.Laret

.Lasz:					| small is 0: big, or a & b when both are 0
	move.l	%d4,%d6
	or.l	%d1,%d6
	bne.s	.Laret
	and.l	%d2,%d0
	bra.s	.Laret

.Laspec:				| big is inf or NaN
	bhi.s	1f
	tst.l	%d1
	bne.s	1f
	cmp.l	%d4,%d5			| inf: small inf of the other sign gives NaN
	bne	.Laret
	move.l	%d0,%d6
	eor.l	%d2,%d6
	bpl	.Laret
	move.l	#0x7FF80000,%d0
	moveq	#0,%d1
	bra	.Laret
1:	move.l	28(%sp),%d0		| a if a is NaN, else b, quietened
	move.l	32(%sp),%d1
	move.l	%d0,%d6
	add.l	%d6,%d6
	cmp.l	#0xFFE00000,%d6
	bhi.s	2f
	bcs.s	3f
	tst.l	%d1
	bne.s	2f
3:	move.l	%a1,%d0
	move.l	40(%sp),%d1
2:	bset	#19,%d0
	bra	.Laret
	.size	__adddf3,.-__adddf3

| ---- shared tails of mul and div ----

| Pack: d0:d1 = m (implicit bit 52), d3 = guard, d4.w = e - 1, a3 = sign; restores and returns.
.Lpack:
	cmp.w	#2046,%d4
	bcc.s	.Lpslow
.Lpround:
	add.l	%d3,%d3			| C: round bit, Z: no sticky
	bcc.s	1f
	beq.s	3f
	addq.l	#1,%d1
	bcc.s	1f
	addq.l	#1,%d0
1:	swap	%d4
	clr.w	%d4
	lsl.l	#4,%d4			| (e - 1) << 20
	add.l	%d4,%d0
	add.l	%a3,%d0
.Lret:
	movem.l	(%sp)+,%d2-%d7/%a2-%a5
	rts
3:	addq.l	#1,%d1			| tie: to even
	bcc.s	4f
	addq.l	#1,%d0
4:	bclr	#0,%d1
	bra.s	1b

.Lpslow:				| e - 1 outside 0..2045
	tst.w	%d4
	bpl.s	.Linf
	neg.w	%d4			| k = 1 - e: shift m:guard right with sticky
	cmp.w	#54,%d4
	bcc.s	.Lzero
	cmp.w	#32,%d4
	bcs.s	1f
	tst.l	%d3
	beq.s	5f
	moveq	#1,%d3			| sticky below the new round bit
5:	or.l	%d1,%d3
	move.l	%d0,%d1
	moveq	#0,%d0
	sub.w	#32,%d4
1:	move.l	%d3,%d5
	lsr.l	%d4,%d3
	ror.l	%d4,%d5
	eor.l	%d3,%d5			| guard bits shifted out
	sne	%d5
	neg.b	%d5
	or.b	%d5,%d3
	move.l	%d1,%d5
	lsr.l	%d4,%d1
	ror.l	%d4,%d5
	eor.l	%d1,%d5
	or.l	%d5,%d3
	move.l	%d0,%d5
	lsr.l	%d4,%d0
	ror.l	%d4,%d5
	eor.l	%d0,%d5
	or.l	%d5,%d1
	moveq	#0,%d4
	bra.s	.Lpround

.Linf:
	move.l	%a3,%d0
	or.l	#0x7FF00000,%d0
	moveq	#0,%d1
	bra.s	.Lret
.Lzero:
	move.l	%a3,%d0
	moveq	#0,%d1
	bra.s	.Lret
.Lqnan:
	move.l	#0x7FF80000,%d0
	moveq	#0,%d1
	bra.s	.Lret

| NaN check: d0:d1 = a, d2:d3 = b. A NaN operand (a first) is quietened and returned to the caller's caller.
.Lnan:
	move.l	%d0,%d4
	add.l	%d4,%d4
	cmp.l	#0xFFE00000,%d4
	bhi.s	1f
	bcs.s	2f
	tst.l	%d1
	bne.s	1f
2:	move.l	%d2,%d4
	add.l	%d4,%d4
	cmp.l	#0xFFE00000,%d4
	bhi.s	3f
	bcs.s	4f
	tst.l	%d3
	bne.s	3f
4:	rts
3:	move.l	%d2,%d0
	move.l	%d3,%d1
1:	bset	#19,%d0
	addq.l	#4,%sp
	bra	.Lret

| Denormal: d0:d1 = the 52-bit field (nonzero) top-aligned in place; d6.w = 12 - shift, the exponent of the m << 11 form.
| Uses d7 (and a0 on the 68020).
.Lnorm:
	and.l	#0xFFFFF,%d0
	moveq	#12,%d6
	tst.l	%d0
	bne.s	1f
	move.l	%d1,%d0
	moveq	#0,%d1
	moveq	#-20,%d6
1:
.ifdef M68020
	move.l	%d5,%a0
	bfffo	%d0{#0:#0},%d5
	sub.w	%d5,%d6
	lsl.l	%d5,%d0
	move.l	%d1,%d7
	rol.l	%d5,%d7
	lsl.l	%d5,%d1
	eor.l	%d1,%d7
	or.l	%d7,%d0
	move.l	%a0,%d5
.else
	cmp.l	#0x10000,%d0
	bcc.s	2f
	swap	%d0
	swap	%d1
	move.w	%d1,%d0
	clr.w	%d1
	sub.w	#16,%d6
2:	cmp.l	#0x1000000,%d0
	bcc.s	3f
	lsl.l	#8,%d0
	rol.l	#8,%d1
	move.b	%d1,%d0
	clr.b	%d1
	subq.w	#8,%d6
	bra.s	3f
4:	add.l	%d1,%d1
	addx.l	%d0,%d0
	subq.w	#1,%d6
3:	tst.l	%d0
	bpl.s	4b
.endif
	rts

| ---- mul ----

| \x * \y -> \hi:\x, \y kept
	.macro	mul32 x, y, hi, t1, t2
.ifdef WIDE
	mulu.l	\y,\hi:\x
.else
	move.l	\x,\t1
	swap	\t1			| xh
	move.l	\y,\t2
	swap	\t2			| yh
	move.w	\t2,\hi
	mulu.w	\t1,\hi			| xh * yh
	mulu.w	\y,\t1			| xh * yl
	mulu.w	\x,\t2			| xl * yh
	mulu.w	\y,\x			| xl * yl
	add.l	\t1,\t2
	bcc.s	1f
	add.l	#0x10000,\hi
1:	swap	\t2
	moveq	#0,\t1
	move.w	\t2,\t1
	clr.w	\t2
	add.l	\t2,\x
	addx.l	\t1,\hi
.endif
	.endm

	.globl	__muldf3
	.type	__muldf3,@function
__muldf3:
	movem.l	%d2-%d7/%a2-%a5,-(%sp)
	movem.l	SAVED+4(%sp),%d0-%d3
	move.l	%d0,%d4
	eor.l	%d2,%d4
	and.l	#0x80000000,%d4
	move.l	%d4,%a3			| sign
	expo	%d0,%d4
	beq	.Lmodd
	cmp.w	#0x7FF,%d4
	beq	.Lmodd
	expo	%d2,%d5
	beq	.Lmodd
	cmp.w	#0x7FF,%d5
	beq	.Lmodd
	shl11	%d0,%d1,%d6
	bset	#31,%d0			| A = ma << 11
	and.l	#0xFFFFF,%d2
	bset	#20,%d2
	add.l	%d3,%d3
	addx.l	%d2,%d2			| B = mb << 1: the product's m lands in its top 64 bits
.Lmgo:					| d0:d1 = A, d2:d3 = B, d4.w = ea, d5.w = eb (the m << 11 form)
	add.w	%d5,%d4
	sub.w	#1024,%d4		| e - 1 if the product's top bit is 116
	move.w	%d4,%a4
	move.l	%d0,%d4
	mul32	%d4,%d3,%d5,%d6,%d7	| AH * BL
	move.l	%d5,%a0
	move.l	%d4,%a1
	move.l	%d1,%d4
	mul32	%d4,%d3,%d5,%d6,%d7	| AL * BL
	move.l	%d4,%a2			| P0
	move.l	%a0,%d4
	add.l	%a1,%d5
	bcc.s	1f
	addq.l	#1,%d4			| d4:d5 = P2:P1 so far
1:	mul32	%d1,%d2,%d3,%d6,%d7	| AL * BH
	add.l	%d1,%d5
	moveq	#0,%d6
	addx.l	%d6,%d3
	add.l	%d3,%d4
	moveq	#0,%d3
	addx.l	%d3,%d3			| P3 so far
	mul32	%d0,%d2,%d1,%d6,%d7	| AH * BH
	add.l	%d0,%d4
	addx.l	%d3,%d1
	move.l	%d1,%d0			| P3, top bit 20 or 21
	move.l	%d4,%d1			| P2
	move.l	%d5,%d3			| P1: guard
	move.l	%a2,%d5
	move.l	%a4,%d4
	btst	#21,%d0
	beq.s	2f
	lsr.l	#1,%d0
	roxr.l	#1,%d1
	roxr.l	#1,%d3
	bcc.s	1f
	moveq	#1,%d5
1:	addq.w	#1,%d4
2:	tst.l	%d5
	beq	.Lpack
	or.b	#1,%d3
	bra	.Lpack

.Lmodd:					| an exponent field is 0 or 2047
	bsr	.Lnan
	expo	%d0,%d4
	expo	%d2,%d5
	cmp.w	#0x7FF,%d4
	bne.s	1f
	tst.w	%d5			| inf * x
	bne	.Linf
	mant0	%d2,%d3,%d6
	beq	.Lqnan
	bra	.Linf
1:	cmp.w	#0x7FF,%d5
	bne.s	2f
	tst.w	%d4			| x * inf
	bne	.Linf
	mant0	%d0,%d1,%d6
	beq	.Lqnan
	bra	.Linf
2:	tst.w	%d4
	bne.s	3f
	mant0	%d0,%d1,%d6
	beq	.Lzero
3:	tst.w	%d5
	bne.s	4f
	mant0	%d2,%d3,%d6
	beq	.Lzero
4:	tst.w	%d4			| no zero, inf or NaN: denormals
	bne.s	5f
	bsr	.Lnorm
	move.w	%d6,%d4
	bra.s	6f
5:	shl11	%d0,%d1,%d6
	bset	#31,%d0
6:	exg	%d0,%d2
	exg	%d1,%d3
	tst.w	%d5
	bne.s	7f
	bsr	.Lnorm
	move.w	%d6,%d5
	moveq	#9,%d6
8:	lsr.l	#1,%d0
	roxr.l	#1,%d1
	dbra	%d6,8b			| m << 1
	bra.s	9f
7:	and.l	#0xFFFFF,%d0
	bset	#20,%d0
	add.l	%d1,%d1
	addx.l	%d0,%d0
9:	exg	%d0,%d2
	exg	%d1,%d3
	bra	.Lmgo
	.size	__muldf3,.-__muldf3

| ---- div ----

	.globl	__divdf3
	.type	__divdf3,@function
__divdf3:
	movem.l	%d2-%d7/%a2-%a5,-(%sp)
	movem.l	SAVED+4(%sp),%d0-%d3
	move.l	%d0,%d4
	eor.l	%d2,%d4
	and.l	#0x80000000,%d4
	move.l	%d4,%a3			| sign
	expo	%d0,%d4
	beq	.Ldodd
	cmp.w	#0x7FF,%d4
	beq	.Ldodd
	expo	%d2,%d5
	beq	.Ldodd
	cmp.w	#0x7FF,%d5
	beq	.Ldodd
	shl11	%d0,%d1,%d6
	bset	#31,%d0			| U
	shl11	%d2,%d3,%d6
	bset	#31,%d2			| V
.Ldgo:					| d0:d1 = U, d2:d3 = V, d4.w = ea, d5.w = eb
	sub.w	%d5,%d4
	add.w	#1021,%d4		| e - 1 when U < V
	cmp.l	%d2,%d0
	bne.s	1f
	cmp.l	%d3,%d1
1:	bcs.s	2f
	lsr.l	#1,%d0
	roxr.l	#1,%d1
	addq.w	#1,%d4
2:	move.w	%d4,%a4
.ifdef WIDE
| Two 32-bit digits: q = (R1:R0) / V1, R = R << 32 - q * V, corrected by adding V back.
	.macro	digit
	cmp.l	%d2,%d0
	bcc.s	3f			| R1 = V1
	divu.l	%d2,%d0:%d1		| d0 = r, d1 = q
	move.l	%d1,%d7
	move.l	%d3,%d5
	mulu.l	%d7,%d4:%d5		| q * V0
	moveq	#0,%d1
	sub.l	%d5,%d1
	subx.l	%d4,%d0
	bcc.s	2f
1:	subq.l	#1,%d7
	add.l	%d3,%d1
	addx.l	%d2,%d0
	bcc.s	1b
	bra.s	2f
3:	moveq	#-1,%d7			| q = 2^32 - 1: R = (R0 - V0) << 32 + V
	move.l	%d1,%d0
	sub.l	%d3,%d0
	move.l	%d3,%d1
	add.l	%d2,%d0
	bcc.s	1b
2:
	.endm
	digit
	move.l	%d7,%a5
	digit
.else
| Four 16-bit digits: q = (R3:R2) / V3, R = R << 16 - q * (V2:V1:V0), corrected by adding V back.
	move.l	%d2,%a0			| V hi
	move.l	%d3,%a1			| V lo
	clr.w	%d2
	move.l	%d2,%a2			| V3 << 16
	swap	%d2			| V3
	.macro	digit
	cmp.l	%a2,%d0
	bcc.s	3f			| R3 = V3
	divu.w	%d2,%d0			| r : q
	move.w	%d0,%d7
	swap	%d1
	move.w	%d1,%d0			| r : R1
	clr.w	%d1			| R0 : 0
	move.l	%a1,%d3
	mulu.w	%d7,%d3			| q * V0
	move.l	%a1,%d4
	swap	%d4
	mulu.w	%d7,%d4			| q * V1
	move.l	%a0,%d5
	mulu.w	%d7,%d5			| q * V2
	moveq	#0,%d6
	swap	%d4
	move.w	%d4,%d6
	clr.w	%d4
	add.l	%d4,%d3
	addx.l	%d6,%d5			| d5:d3 = q * (V2:V1:V0)
	sub.l	%d3,%d1
	subx.l	%d5,%d0
	bcc.s	2f
1:	subq.w	#1,%d7
	add.l	%a1,%d1
	move.l	%a0,%d3
	addx.l	%d3,%d0
	bcc.s	1b
	bra.s	2f
3:	move.w	#-1,%d7			| q = 2^16 - 1: R = (R - V) << 16 + V
	sub.l	%a1,%d1
	move.l	%a0,%d3
	subx.l	%d3,%d0
	swap	%d0
	swap	%d1
	move.w	%d1,%d0
	clr.w	%d1
	add.l	%a1,%d1
	move.l	%a0,%d3
	addx.l	%d3,%d0
	bcc.s	1b
2:
	.endm
	digit
	swap	%d7
	digit
	move.l	%d7,%a5
	digit
	swap	%d7
	digit
.endif
	or.l	%d0,%d1			| remainder: sticky
	sne	%d5
	move.l	%a5,%d0
	move.l	%d7,%d1			| q, implicit bit 63
	move.l	%d1,%d3
	swap	%d3
	clr.w	%d3
	lsl.l	#5,%d3			| guard
	or.b	%d5,%d3
	lsr.l	#8,%d1
	lsr.l	#3,%d1
	move.l	%d0,%d6
	swap	%d6
	clr.w	%d6
	lsl.l	#5,%d6
	or.l	%d6,%d1
	lsr.l	#8,%d0
	lsr.l	#3,%d0
	move.l	%a4,%d4
	bra	.Lpack

.Ldodd:					| an exponent field is 0 or 2047
	bsr	.Lnan
	expo	%d0,%d4
	expo	%d2,%d5
	cmp.w	#0x7FF,%d4
	bne.s	1f
	cmp.w	#0x7FF,%d5
	beq	.Lqnan			| inf / inf
	bra	.Linf			| inf / x
1:	cmp.w	#0x7FF,%d5
	beq	.Lzero			| x / inf
	tst.w	%d5
	bne.s	2f
	mant0	%d2,%d3,%d6
	bne.s	2f
	tst.w	%d4			| x / 0
	bne	.Linf
	mant0	%d0,%d1,%d6
	beq	.Lqnan			| 0 / 0
	bra	.Linf
2:	tst.w	%d4
	bne.s	3f
	mant0	%d0,%d1,%d6
	beq	.Lzero			| 0 / x
	bsr	.Lnorm
	move.w	%d6,%d4
	bra.s	4f
3:	shl11	%d0,%d1,%d6
	bset	#31,%d0
4:	exg	%d0,%d2
	exg	%d1,%d3
	tst.w	%d5
	bne.s	5f
	bsr	.Lnorm
	move.w	%d6,%d5
	bra.s	6f
5:	shl11	%d0,%d1,%d6
	bset	#31,%d0
6:	exg	%d0,%d2
	exg	%d1,%d3
	bra	.Ldgo
	.size	__divdf3,.-__divdf3
