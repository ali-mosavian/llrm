	.section	.text.f,"",@
	.globl	f
	.type	f,@function
f:
.Lfunc_begin0:
	.functype	f (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32
	global.get	__stack_pointer
	local.set	1
	i32.const	0
	local.set	2
	loop
	local.get	2
	local.set	3
	local.get	3
	call	g
	try
	i32.const	1
	call	g
.Ltmp0:
	catch_all
	local.get	1
	global.set	__stack_pointer
	i32.const	2
	return
	end_try
	i32.const	2
	call	g
	local.get	3
	i32.const	1
	i32.add
	local.set	4
	local.get	4
	local.get	0
	i32.lt_s
	local.set	5
	local.get	4
	local.set	2
	local.get	5
	br_if   	0
	end_loop
	i32.const	1
	return
	end_function
.Lfunc_end0:
