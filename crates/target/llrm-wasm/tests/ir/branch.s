	.section	.text.diamond,"",@
	.globl	diamond
	.type	diamond,@function
diamond:
	.functype	diamond (i32, i32) -> (i32)
	.local  	i32
	block
	block
	local.get	0
	local.get	1
	i32.ge_s
	br_if   	0
	local.get	0
	i32.const	1
	i32.add
	local.set	2
	br      	1
	end_block
	local.get	1
	i32.const	-1
	i32.add
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.skip,"",@
	.globl	skip
	.type	skip,@function
skip:
	.functype	skip (i32) -> ()
	block
	local.get	0
	i32.eqz
	br_if   	0
	call	side
	end_block
	return
	end_function
	.section	.text.count,"",@
	.globl	count
	.type	count,@function
count:
	.functype	count (i32) -> (i32)
	.local  	i32, i32, i32
	i32.const	0
	local.set	1
	loop
	local.get	1
	i32.const	1
	i32.add
	local.set	2
	local.get	2
	local.get	0
	i32.lt_u
	local.set	3
	local.get	2
	local.set	1
	local.get	3
	br_if   	0
	end_loop
	local.get	2
	return
	end_function
