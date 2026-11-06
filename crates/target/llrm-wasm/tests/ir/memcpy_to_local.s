	.section	.text.f,"",@
	.globl	f
	.type	f,@function
f:
	.functype	f (i32) -> ()
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	112
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	i32.const	100
	local.set	2
	local.get	1
	i32.const	12
	i32.add
	local.get	0
	local.get	2
	memory.copy	0, 0
	local.get	1
	i32.const	12
	i32.add
	call	use
	local.get	1
	i32.const	112
	i32.add
	global.set	__stack_pointer
	return
	end_function
