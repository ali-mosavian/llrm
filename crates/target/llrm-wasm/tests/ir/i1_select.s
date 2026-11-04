	.section	.text.both,"",@
	.globl	both
	.type	both,@function
both:
	.functype	both (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.and
	return
	end_function
	.section	.text.both_widened,"",@
	.globl	both_widened
	.type	both_widened,@function
both_widened:
	.functype	both_widened (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.and
	i32.const	1
	i32.and
	return
	end_function
