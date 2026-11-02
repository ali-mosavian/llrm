| IEEE-754 binary32 for the m68k: round to nearest even, denormals, inf, NaN.
| Hand written; no libgcc source is copied (libgcc is only a binary comparison column).
| libgcc's ABI: arguments on the stack, result in d0, d0-d1/a0-a1 scratch.
| Base body 68000/68010; --defsym M68020=1 selects bfffo and the 64-bit mulu.l/divu.l (68020/030/040);
| add --defsym M68060=1 for the 68060, which traps on the 64-bit forms: mul and div keep the 16-bit cores.
| Packing: sign | (e - 1) << 23 plus the 24-bit significand m (implicit bit at 23),
| so a rounding carry moves into the exponent and reaches inf by itself.
| Rounding: m with a 32-bit guard word (round bit 31, sticky below).

	.text
.ifdef M68020
.ifndef M68060
WIDE = 1
.endif
.endif

	.globl	__negsf2
	.type	__negsf2,@function
__negsf2:
	move.l	4(%sp),%d0
	bchg	#31,%d0
	rts
	.size	__negsf2,.-__negsf2

| ---- compare: -1/0/1; unordered gives 1 (cmp eq ne lt le) or -1 (gt ge) ----

	.globl	__gtsf2
	.type	__gtsf2,@function
	.globl	__gesf2
	.type	__gesf2,@function
__gtsf2:
__gesf2:
	moveq	#-1,%d0
	bra.s	.Lcmp
	.size	__gtsf2,.-__gtsf2
	.size	__gesf2,.-__gesf2

	.globl	__cmpsf2
	.type	__cmpsf2,@function
	.globl	__eqsf2
	.type	__eqsf2,@function
	.globl	__nesf2
	.type	__nesf2,@function
	.globl	__ltsf2
	.type	__ltsf2,@function
	.globl	__lesf2
	.type	__lesf2,@function
__cmpsf2:
__eqsf2:
__nesf2:
__ltsf2:
__lesf2:
	moveq	#1,%d0
.Lcmp:
	move.l	%d2,%a0
	move.l	%d0,%a1			| the unordered result
	move.l	4(%sp),%d1		| a
	move.l	8(%sp),%d2		| b
	move.l	%d1,%d0
	add.l	%d0,%d0
	cmp.l	#0xFF000000,%d0
	bhi.s	.Lcunord
	move.l	%d2,%d0
	add.l	%d0,%d0
	cmp.l	#0xFF000000,%d0
	bhi.s	.Lcunord
	move.l	%d1,%d0
	or.l	%d2,%d0
	add.l	%d0,%d0
	beq.s	.Lcret			| +-0 == +-0
	move.l	%d1,%d0
	eor.l	%d2,%d0
	bmi.s	.Lcdiff
	move.l	%d1,%d0
	sub.l	%d2,%d1			| same sign: |a| - |b| has the sign of a - b for a >= 0
	beq.s	.Lceq
	eor.l	%d0,%d1			| N: a < b
.Lcdiff:
	moveq	#1,%d0
	tst.l	%d1
	bpl.s	.Lcret
	moveq	#-1,%d0
	bra.s	.Lcret
.Lceq:
	moveq	#0,%d0
.Lcret:
	move.l	%a0,%d2
	rts
.Lcunord:
	move.l	%a1,%d0
	move.l	%a0,%d2
	rts
	.size	__cmpsf2,.-__cmpsf2
	.size	__eqsf2,.-__eqsf2
	.size	__nesf2,.-__nesf2
	.size	__ltsf2,.-__ltsf2
	.size	__lesf2,.-__lesf2

	.globl	__unordsf2
	.type	__unordsf2,@function
__unordsf2:
	move.l	#0xFF000000,%d1
	move.l	4(%sp),%d0
	add.l	%d0,%d0
	cmp.l	%d1,%d0
	bhi.s	1f
	move.l	8(%sp),%d0
	add.l	%d0,%d0
	cmp.l	%d1,%d0
	bhi.s	1f
	moveq	#0,%d0
	rts
1:	moveq	#1,%d0
	rts
	.size	__unordsf2,.-__unordsf2

| ---- add, sub ----

	.globl	__subsf3
	.type	__subsf3,@function
__subsf3:
	move.l	8(%sp),%d1
	bchg	#31,%d1
	bra.s	.Ladd
	.size	__subsf3,.-__subsf3

	.globl	__addsf3
	.type	__addsf3,@function
__addsf3:
	move.l	8(%sp),%d1
.Ladd:
	move.l	4(%sp),%d0
	move.l	%d0,%a0
	move.l	%d1,%a1
	add.l	%d0,%d0
	add.l	%d1,%d1
	cmp.l	%d1,%d0
	bcc.s	1f
	exg	%d0,%d1
	exg	%a0,%a1
1:	cmp.l	#0xFF000000,%d0		| a0 = big, a1 = small, d0 = |big| << 1, d1 = |small| << 1
	bcc.s	.Laspec
	sub.l	%d1,%d0
	cmp.l	#0x1A000000,%d0
	bcs.s	.Lanear
	move.l	%a0,%d0			| exponents 26 or more apart: small is under a quarter ulp
	rts

.Laspec:				| big is inf (Z) or NaN
	bne.s	.Laqnan
	cmp.l	%d0,%d1
	bne.s	.Labig
	move.l	%a0,%d0
	move.l	%a1,%d1
	eor.l	%d0,%d1
	bpl.s	.Labig
	move.l	#0x7FC00000,%d0		| inf - inf
	rts
.Laqnan:				| a if a is NaN, else b (big), quietened
	move.l	4(%sp),%d0
	move.l	%d0,%d1
	add.l	%d1,%d1
	cmp.l	#0xFF000000,%d1
	bhi.s	1f
	move.l	%a0,%d0
1:	bset	#22,%d0
	rts
.Labig:
	move.l	%a0,%d0
	rts

.Lanear:				| d0 = (|big| - |small|) << 1, d1 = |small| << 1
	move.l	%d2,-(%sp)
	move.l	%d3,-(%sp)
	add.l	%d1,%d0
	swap	%d0
	lsr.w	#8,%d0			| fa
	beq	.Laden
	move.l	%a1,%d3
	move.l	%a0,%d2
	eor.l	%d3,%d2
	move.l	%d2,%a1			| bit 31: the signs differ
	and.l	#0x7FFFFF,%d3
	swap	%d1
	lsr.w	#8,%d1			| fb
	beq.s	.Lab0
	bset	#23,%d3			| mb
.Lab1:
	sub.w	%d1,%d0			| n = fa - fb, 0..26
	move.l	%d3,%d2
	lsr.l	%d0,%d3
	ror.l	%d0,%d2
	eor.l	%d3,%d2			| guard: the bits shifted out, top aligned
	move.l	%a0,%d0
	move.l	%d0,%d1
	and.l	#0x7FFFFF,%d0
	bset	#23,%d0			| ma
	sub.l	%d0,%d1			| E = sign | (fa - 1) << 23
	exg	%d1,%a1
	tst.l	%d1
	bmi.s	.Lasub
	add.l	%d3,%d0
	btst	#24,%d0
	bne.s	.Lacarry
.Laround:				| d0 = m, d2 = guard, a1 = E
	add.l	%d2,%d2
	bcc.s	1f
	beq.s	.Latie
	addq.l	#1,%d0
1:	add.l	%a1,%d0
.Laret:
	move.l	(%sp)+,%d3
	move.l	(%sp)+,%d2
	rts
.Latie:
	addq.l	#1,%d0
	bclr	#0,%d0
	bra.s	1b

.Lab0:					| small is denormal: e = 1, no implicit bit
	moveq	#1,%d1
	bra.s	.Lab1

.Lacarry:
	lsr.l	#1,%d0
	roxr.l	#1,%d2			| guard bit 0 is clear: n <= 26
	adda.l	#0x800000,%a1
	move.l	%a1,%d1
	add.l	%d1,%d1
	cmp.l	#0xFE000000,%d1
	bcs.s	.Laround
	move.l	%a1,%d0			| fa + 1 = 255
	add.l	#0x800000,%d0
	bra.s	.Laret

.Lasub:
	neg.l	%d2
	subx.l	%d3,%d0			| Z: the difference is 0
	beq.s	.Lazero
	btst	#23,%d0
	bne.s	.Laround
	move.l	%a1,%d1
1:	move.l	%d1,%d3
	add.l	%d3,%d3
	beq.s	2f			| e = 1: denormal, exact
	sub.l	#0x800000,%d1
	add.l	%d2,%d2
	addx.l	%d0,%d0
	btst	#23,%d0
	beq.s	1b
2:	move.l	%d1,%a1
	bra.s	.Laround
.Lazero:
	moveq	#0,%d0
	bra.s	.Laret

.Laden:					| both exponent fields 0: the integer sum is exact
	move.l	%a0,%d0
	move.l	%a1,%d1
	move.l	%d1,%d2
	eor.l	%d0,%d2
	bclr	#31,%d1
	tst.l	%d2
	bmi.s	1f
	add.l	%d1,%d0
	bra.s	.Laret
1:	sub.l	%d1,%d0
	move.l	%d0,%d1
	add.l	%d1,%d1
	bne.s	.Laret
	moveq	#0,%d0
	bra.s	.Laret
	.size	__addsf3,.-__addsf3

| ---- mul, div: shared unpacking ----
| In: d0 = |a| << 1, d1 = |b| << 1 (exponent fields nonzero or the operand normalized below).
| Out: d2 = fa, d3 = fb (a denormal gets 1 - shift and bit 24 set), branches to \spec on 0, inf, NaN.

	.macro	fields spec, aok, bok
	move.l	%d0,%d2
	swap	%d2
	lsr.w	#8,%d2
	beq	8f
	cmp.w	#255,%d2
	beq	\spec
\aok:
	move.l	%d1,%d3
	swap	%d3
	lsr.w	#8,%d3
	beq	9f
	cmp.w	#255,%d3
	beq	\spec
\bok:
	.endm

	.macro	denorm spec, aok, bok
8:	tst.l	%d0
	beq	\spec
.ifdef M68020
	bfffo	%d0{#0:#0},%d2
	subq.l	#7,%d2
	lsl.l	%d2,%d0
	neg.w	%d2
	addq.w	#1,%d2
.else
	moveq	#1,%d2
1:	subq.w	#1,%d2
	add.l	%d0,%d0
	btst	#24,%d0
	beq.s	1b
.endif
	bra	\aok
9:	tst.l	%d1
	beq	\spec
.ifdef M68020
	bfffo	%d1{#0:#0},%d3
	subq.l	#7,%d3
	lsl.l	%d3,%d1
	neg.w	%d3
	addq.w	#1,%d3
.else
	moveq	#1,%d3
1:	subq.w	#1,%d3
	add.l	%d1,%d1
	btst	#24,%d1
	beq.s	1b
.endif
	bra	\bok
	.endm

	.macro	entry
	movem.l	%d2-%d4,-(%sp)
	move.l	16(%sp),%d0
	move.l	20(%sp),%d1
	move.l	%d0,%d2
	eor.l	%d1,%d2
	and.l	#0x80000000,%d2
	move.l	%d2,%a1			| sign
	add.l	%d0,%d0
	add.l	%d1,%d1
	.endm

| ---- mul ----

	.globl	__mulsf3
	.type	__mulsf3,@function
__mulsf3:
	entry
	fields	.Lmspec, .Lma1, .Lmb1
	add.w	%d3,%d2
	sub.w	#128,%d2		| e - 1 before the product's carry
	cmp.w	#253,%d2
	bcc	.Lmrange
.Lmgo:
	move.w	%d2,%a0
	and.l	#0xFFFFFF,%d0
	bset	#24,%d0			| ma << 1
	lsl.l	#7,%d1
	bset	#31,%d1			| mb << 8
.ifdef WIDE
	mulu.l	%d1,%d4:%d0
.else
	move.l	%d0,%d2
	swap	%d2			| ah, 9 bits
	move.l	%d1,%d3
	swap	%d3			| bh
	move.w	%d3,%d4
	mulu.w	%d2,%d4			| ah * bh
	mulu.w	%d1,%d2			| ah * bl
	mulu.w	%d0,%d3			| al * bh
	mulu.w	%d1,%d0			| al * bl
	add.l	%d2,%d3
	bcc.s	1f
	add.l	#0x10000,%d4
1:	swap	%d3
	moveq	#0,%d2
	move.w	%d3,%d2
	clr.w	%d3
	add.l	%d3,%d0
	addx.l	%d2,%d4
.endif
	move.l	%d0,%d1			| guard
	move.l	%d4,%d0			| 2^23 <= m < 2^25
	move.l	%a0,%d2
	btst	#24,%d0
	beq.s	1f
	lsr.l	#1,%d0
	roxr.l	#1,%d1			| guard bit 0 is clear
	addq.l	#1,%d2
1:	cmp.w	#254,%d2
	bcc.s	.Lslow
.Lpack:					| d0 = m, d1 = guard, d2 = e - 1 in 0..253 (upper word 0), a1 = sign
	add.l	%d1,%d1
	bcc.s	1f
	beq.s	2f
	addq.l	#1,%d0
1:	lsl.w	#7,%d2
	swap	%d2			| (e - 1) << 23
	add.l	%a1,%d2
	add.l	%d2,%d0
	movem.l	(%sp)+,%d2-%d4
	rts
2:	addq.l	#1,%d0
	bclr	#0,%d0
	bra.s	1b

.Lslow:					| e - 1 outside 0..253
	cmp.w	#254,%d2
	bge.s	.Linf
	neg.w	%d2			| k = 1 - e: denormal shift of d0:d1, sticky
	cmp.w	#25,%d2
	bge.s	.Lzero
	move.l	%d1,%d3
	lsr.l	%d2,%d1
	ror.l	%d2,%d3
	eor.l	%d1,%d3			| guard bits shifted out
	sne	%d3
	or.b	%d3,%d1
	move.l	%d0,%d3
	lsr.l	%d2,%d0
	ror.l	%d2,%d3
	eor.l	%d0,%d3			| m's low k bits, top aligned (k <= 24: above the low byte)
	or.l	%d3,%d1
	moveq	#0,%d2
	bra.s	.Lpack
.Linf:
	move.l	%a1,%d0
	or.l	#0x7F800000,%d0
	bra.s	.Lret
.Lzero:
	move.l	%a1,%d0
	bra.s	.Lret
.Lmnan:
	move.l	#0x7FC00000,%d0
	bra.s	.Lret
.Lqnan1:
	move.l	%d1,%d0
.Lqnan0:
	bset	#22,%d0
.Lret:
	movem.l	(%sp)+,%d2-%d4
	rts

.Lmrange:				| fa + fb - 128 outside 0..252
	cmp.w	#254,%d2
	bge.s	.Linf
	cmp.w	#-26,%d2
	ble.s	.Lzero
	bra	.Lmgo

.Lmspec:				| a or b is 0, inf or NaN
	bsr.s	.Lspecin
	bhi.s	.Lqnan1
	tst.l	%d2
	beq.s	1f
	tst.l	%d3
	bne.s	.Linf			| inf * nonzero
1:	cmp.l	%d4,%d2
	beq.s	.Lmnan			| inf * 0
	cmp.l	%d4,%d3
	beq.s	.Lmnan
	bra.s	.Lzero

| d0 = a, d1 = b, d2 = |a| << 1, d3 = |b| << 1, d4 = inf << 1; returns to the caller's caller if a is NaN,
| else with the flags of |b| <=> inf.
.Lspecin:
	move.l	20(%sp),%d0
	move.l	24(%sp),%d1
	move.l	%d0,%d2
	add.l	%d2,%d2
	move.l	%d1,%d3
	add.l	%d3,%d3
	move.l	#0xFF000000,%d4
	cmp.l	%d4,%d2
	bls.s	1f
	addq.l	#4,%sp
	bra.s	.Lqnan0
1:	cmp.l	%d4,%d3
	rts

	denorm	.Lmspec, .Lma1, .Lmb1
	.size	__mulsf3,.-__mulsf3

| ---- div ----

	.globl	__divsf3
	.type	__divsf3,@function
__divsf3:
	entry
	fields	.Ldspec, .Lda1, .Ldb1
	sub.w	%d3,%d2
	add.w	#125,%d2		| e - 1 when ma < mb
	lsl.l	#7,%d0
	bset	#31,%d0			| ma << 8
	lsl.l	#7,%d1
	bset	#31,%d1			| mb << 8
	cmp.l	%d1,%d0
	bcs.s	1f
	lsr.l	#1,%d0
	addq.w	#1,%d2
1:	cmp.w	#254,%d2
	bcc	.Ldrange
.Ldgo:					| d0 = U < d1 = V: q = U * 2^32 / V, 2^31 <= q < 2^32
	move.w	%d2,%a0
.ifdef WIDE
	move.l	%d0,%d4
	moveq	#0,%d0
	divu.l	%d1,%d4:%d0		| d0 = q, d4 = remainder
.else
	move.l	%d1,%d3
	swap	%d3			| v1, the divisor's top half (v0 has a zero low byte)
	move.l	%d0,%d4
	swap	%d4
	cmp.w	%d3,%d4
	bcc.s	.Ldov1
	move.l	%d0,%d4
	divu.w	%d3,%d4			| remainder : qhat
	move.w	%d4,%d0
	move.w	%d4,%d2
	mulu.w	%d1,%d2
	clr.w	%d4
	sub.l	%d2,%d4			| r = rem << 16 - qhat * v0
	bcc.s	2f
1:	subq.w	#1,%d0
	add.l	%d1,%d4
	bcc.s	1b
2:	swap	%d0			| q1 in the upper word
	move.l	%d4,%d2
	swap	%d2
	cmp.w	%d3,%d2
	bcc.s	.Ldov2
	divu.w	%d3,%d4
	move.w	%d4,%d0
	move.w	%d4,%d2
	mulu.w	%d1,%d2
	clr.w	%d4
	sub.l	%d2,%d4
	bcc.s	.Ldq
3:	subq.l	#1,%d0
	add.l	%d1,%d4
	bcc.s	3b
	bra.s	.Ldq
.Ldov1:					| top halves equal: qhat = 0xFFFF, r = (U - V) << 16 + V
	move.l	%d0,%d4
	move.w	#0xFFFF,%d0
	sub.l	%d1,%d4
	swap	%d4
	clr.w	%d4
	add.l	%d1,%d4
	bcs.s	2b
	bra.s	1b
.Ldov2:
	move.w	#0xFFFF,%d0
	sub.l	%d1,%d4
	swap	%d4
	clr.w	%d4
	add.l	%d1,%d4
	bcs.s	.Ldq
	bra.s	3b
.endif
.Ldq:					| d0 = q, d4 = remainder
	move.l	%d0,%d1
	lsr.l	#8,%d0			| m
	ror.l	#8,%d1
	eor.l	%d0,%d1			| guard = q << 24
	tst.l	%d4
	beq.s	1f
	addq.l	#1,%d1			| sticky
1:	move.l	%a0,%d2
	cmp.w	#254,%d2
	bcc	.Lslow
	bra	.Lpack

.Ldrange:				| e - 1 outside 0..253
	cmp.w	#254,%d2
	bge	.Linf
	cmp.w	#-25,%d2
	ble	.Lzero
	bra	.Ldgo

.Ldspec:				| a or b is 0, inf or NaN
	bsr	.Lspecin
	bhi	.Lqnan1
	cmp.l	%d4,%d2
	bne.s	1f
	cmp.l	%d4,%d3			| inf / x
	beq	.Lmnan
	bra	.Linf
1:	cmp.l	%d4,%d3
	beq	.Lzero			| x / inf
	tst.l	%d3
	bne	.Lzero			| 0 / x
	tst.l	%d2
	beq	.Lmnan			| 0 / 0
	bra	.Linf			| x / 0

	denorm	.Ldspec, .Lda1, .Ldb1
	.size	__divsf3,.-__divsf3

| ---- conversions: int to float rounds to nearest even; float to int truncates, saturates, NaN gives 0 ----

	.globl	__floatunsisf
	.type	__floatunsisf,@function
__floatunsisf:
	move.l	4(%sp),%d0
	beq.s	.Lirt
	sub.l	%a0,%a0			| sign
	bra.s	.Lint
	.size	__floatunsisf,.-__floatunsisf

	.globl	__floatsisf
	.type	__floatsisf,@function
__floatsisf:
	move.l	4(%sp),%d0
	beq.s	.Lirt
	move.l	%d0,%a0			| sign
	bpl.s	.Lint
	neg.l	%d0
.Lint:					| d0 = |x| != 0: top-align it, field - 1 = 157 - shift
.ifdef M68020
	bfffo	%d0{#0:#0},%d1
	lsl.l	%d1,%d0
	neg.w	%d1
	add.w	#157,%d1
	ext.l	%d1
	lsl.w	#7,%d1
.else
	move.l	#157<<7,%d1
	cmp.l	#0x10000,%d0
	bcc.s	1f
	swap	%d0
	sub.w	#16<<7,%d1
1:	cmp.l	#0x1000000,%d0
	bcc.s	2f
	lsl.l	#8,%d0
	sub.w	#8<<7,%d1
2:	tst.l	%d0
	bmi.s	4f
3:	sub.w	#1<<7,%d1
	add.l	%d0,%d0
	bpl.s	3b
4:
.endif
	swap	%d1			| (field - 1) << 23
	move.l	%d0,%a1
	lsr.l	#8,%d0
	add.l	%d1,%d0
	move.l	%a1,%d1
	add.b	%d1,%d1			| C: round bit, Z: no sticky
	bcc.s	6f
	bne.s	5f
	btst	#0,%d0
	beq.s	6f
5:	addq.l	#1,%d0
6:	move.l	%a0,%d1
	bpl.s	.Lirt
	bset	#31,%d0
.Lirt:
	rts
	.size	__floatsisf,.-__floatsisf

	.globl	__fixunssfsi
	.type	__fixunssfsi,@function
__fixunssfsi:
	move.l	4(%sp),%d0
	bmi	.Lfz			| negative (and -NaN): 0
	move.l	%d0,%d1
	swap	%d1
	lsr.w	#7,%d1
	sub.w	#127,%d1		| e
	blt.s	.Lfz
	cmp.w	#32,%d1
	bge.s	1f
	and.l	#0x7FFFFF,%d0
	bset	#23,%d0
	sub.w	#23,%d1
	bge.s	.Lfl
	neg.w	%d1
	lsr.l	%d1,%d0
	rts
1:	cmp.l	#0x7F800000,%d0
	bhi.s	.Lfz			| NaN
	moveq	#-1,%d0
	rts
	.size	__fixunssfsi,.-__fixunssfsi

	.globl	__fixsfsi
	.type	__fixsfsi,@function
__fixsfsi:
	move.l	4(%sp),%d0
	move.l	%d0,%a0			| sign
	move.l	%d0,%d1
	add.l	%d1,%d1
	cmp.l	#0xFF000000,%d1
	bhi.s	.Lfz			| NaN
	swap	%d1
	lsr.w	#8,%d1
	sub.w	#127,%d1		| e
	blt.s	.Lfz
	cmp.w	#31,%d1
	bge.s	1f
	and.l	#0x7FFFFF,%d0
	bset	#23,%d0
	sub.w	#23,%d1
	bge.s	2f
	neg.w	%d1
	lsr.l	%d1,%d0
	bra.s	3f
2:	lsl.l	%d1,%d0
3:	move.l	%a0,%d1
	bpl.s	4f
	neg.l	%d0
4:	rts
1:	move.l	#0x7FFFFFFF,%d0		| saturate: 2^31 and up
	move.l	%a0,%d1
	bpl.s	4b
	addq.l	#1,%d0
	rts
.Lfl:
	lsl.l	%d1,%d0
	rts
.Lfz:
	moveq	#0,%d0
	rts
	.size	__fixsfsi,.-__fixsfsi
