_either:
.Leither.0:
	sub sp, sp, #16
	stp x29, x30, [sp]
	add x29, sp, #0
	cbz w0, .Leither.2
	cbz w1, .Leither.2
.Leither.1:
	bl _no
.Leither.3:
	ldp x29, x30, [sp]
	add sp, sp, #16
	ret x30
.Leither.2:
	bl _yes
	b .Leither.3
_both:
.Lboth.0:
	sub sp, sp, #16
	stp x29, x30, [sp]
	add x29, sp, #0
	subs wzr, w0, #7
	b.ge .Lboth.1
	subs wzr, w1, #9
	b.le .Lboth.1
	subs wzr, w2, #3
	b.ne .Lboth.2
.Lboth.1:
	bl _no
.Lboth.3:
	ldp x29, x30, [sp]
	add sp, sp, #16
	ret x30
.Lboth.2:
	bl _yes
	b .Lboth.3
_into_phi:
.Linto_phi.0:
	subs wzr, w0, #10
	csinc w10, wzr, wzr, hs
	subs wzr, w1, #20
	csinc w9, wzr, wzr, ls
	orr w9, w10, w9, lsl #0
	ands wzr, w9, #0x1
	movz w10, #1, lsl #0
	movz w9, #2, lsl #0
	csel w0, w10, w9, ne
	ret x30
