	.section	.text.plain,"",@
	.globl	plain
	.type	plain,@function
plain:
	.functype	plain (i32) -> (i32)
	local.get	0
	call	g
	i32.const	1
	return
	end_function
