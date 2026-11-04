_fib:
.Lfib.0:
	sub sp, sp, #32
	stp x19, x20, [sp]
	stp x29, x30, [sp, #16]
	add x29, sp, #16
	subs wzr, w0, #2
	b.lt .Lfib.e4
.Lfib.1:
	orr x20, xzr, x0, lsl #0
	movz w19, #0, lsl #0
.Lfib.2:
	sub w0, w20, #1
	bl _fib
	orr x9, xzr, x0, lsl #0
	sub w0, w20, #2
	add w19, w9, w19, lsl #0
	subs wzr, w20, #4
	b.lo .Lfib.3
.Lfib.e6:
	orr x20, xzr, x0, lsl #0
	b .Lfib.2
.Lfib.3:
	add w0, w0, w19, lsl #0
	ldp x19, x20, [sp]
	ldp x29, x30, [sp, #16]
	add sp, sp, #32
	ret x30
.Lfib.e4:
	movz w19, #0, lsl #0
	b .Lfib.3
_main:
.Lmain.0:
	sub sp, sp, #64
	stp x19, x20, [sp, #16]
	str x21, [sp, #32]
	stp x29, x30, [sp, #48]
	add x29, sp, #48
	movz w0, #0, lsl #0
	bl _fib
	str x0, [sp]
	adrp x9, _.str@PAGE
	add x19, x9, _.str@PAGEOFF
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #1, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #2, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #3, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #4, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #5, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #6, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #7, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #8, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #9, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w20, #10, lsl #0
	orr x0, xzr, x20, lsl #0
	bl _fib
	orr x21, xzr, x0, lsl #0
	str x21, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #11, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #12, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #13, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	movz w0, #14, lsl #0
	bl _fib
	str x0, [sp]
	orr x0, xzr, x19, lsl #0
	bl _printf
	orr x0, xzr, x20, lsl #0
	bl _putchar
	sub w0, w21, #55
	ldp x19, x20, [sp, #16]
	ldr x21, [sp, #32]
	ldp x29, x30, [sp, #48]
	add sp, sp, #64
	ret x30
