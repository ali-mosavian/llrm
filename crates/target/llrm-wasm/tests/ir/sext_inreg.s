	.section	.text.narrow,"",@
	.globl	narrow
	.type	narrow,@function
narrow:
	.functype	narrow (i32) -> (i32)
	local.get	0
	i32.extend8_s
	return
	end_function
	.section	.text.narrow_wide,"",@
	.globl	narrow_wide
	.type	narrow_wide,@function
narrow_wide:
	.functype	narrow_wide (i32) -> (i32)
	local.get	0
	i32.extend8_s
	return
	end_function
