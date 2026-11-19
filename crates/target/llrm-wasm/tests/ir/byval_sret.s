	.section	.text.swap,"",@
	.globl	swap
	.type	swap,@function
swap:
	.functype	swap (i32, i32) -> ()
	.local  	i64, i64
	local.get	1
	i64.load	0
	local.set	2
	local.get	1
	i64.load	8
	local.set	3
	local.get	0
	local.get	2
	i64.store	8
	local.get	0
	local.get	3
	i64.store	0
	return
	end_function
	.section	.text.use,"",@
	.globl	use
	.type	use,@function
use:
	.functype	use (i64, i64) -> (i64)
	.local  	i32, i64
	global.get	__stack_pointer
	i32.const	48
	i32.sub
	local.set	2
	local.get	2
	global.set	__stack_pointer
	local.get	2
	local.get	0
	i64.store	32
	local.get	2
	local.get	1
	i64.store	40
	local.get	2
	local.get	2
	i64.load	40
	i64.store	8
	local.get	2
	local.get	2
	i64.load	32
	i64.store	0
	local.get	2
	i32.const	16
	i32.add
	local.get	2
	call	swap
	local.get	2
	i64.load	16
	local.set	3
	local.get	2
	i32.const	48
	i32.add
	global.set	__stack_pointer
	local.get	3
	return
	end_function
