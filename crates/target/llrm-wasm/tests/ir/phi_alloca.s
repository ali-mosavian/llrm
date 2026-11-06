	.section	.text.f,"",@
	.globl	f
	.type	f,@function
f:
	.functype	f (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32
	global.get	__stack_pointer
	i32.const	64
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	call	use
	local.get	1
	local.set	2
	loop
	local.get	2
	local.set	3
	local.get	3
	i32.load	0
	local.set	4
	local.get	3
	i32.const	4
	i32.add
	local.set	5
	local.get	4
	i32.const	5
	i32.eq
	local.set	6
	local.get	5
	local.set	2
	local.get	6
	br_if   	0
	end_loop
	local.get	1
	i32.const	64
	i32.add
	global.set	__stack_pointer
	local.get	4
	return
	end_function
