	.section	.text.f,"",@
	.globl	f
	.type	f,@function
f:
.Lfunc_begin0:
	.functype	f (i32) -> (i32)
	.local  	i32
	global.get	__stack_pointer
	local.set	1
	try
	local.get	0
	call	g
	i32.const	2
	call	g
.Ltmp0:
	catch_all
	local.get	1
	global.set	__stack_pointer
	try
	i32.const	3
	call	g
.Ltmp1:
	catch_all
	local.get	1
	global.set	__stack_pointer
	i32.const	4
	call	g
	i32.const	3
	return
	end_try
	i32.const	2
	return
	end_try
	i32.const	1
	return
	end_function
.Lfunc_end0:
