	.section	.text.f,"",@
	.globl	f
	.type	f,@function
f:
.Lfunc_begin0:
	.functype	f (i32) -> (i32)
	.local  	i32, i32, i32
	global.get	__stack_pointer
	local.set	1
	i32.const	0
	local.set	2
	try
	loop
	local.get	2
	local.set	3
	local.get	3
	local.get	0
	i32.ge_s
	br_if   	1
	local.get	3
	call	g
	local.set	2
	br      	0
	end_loop
.Ltmp0:
	catch_all
	local.get	1
	global.set	__stack_pointer
	i32.const	-1
	return
	end_try
	local.get	3
	return
	end_function
.Lfunc_end0:
