	.section	.text.whiles,"",@
	.globl	whiles
	.type	whiles,@function
whiles:
	.functype	whiles (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32
	i32.const	0
	local.set	1
	local.get	0
	local.set	2
	block
	loop
	local.get	2
	local.set	3
	local.get	1
	local.set	4
	local.get	3
	i32.const	1
	i32.lt_s
	br_if   	1
	local.get	4
	local.get	3
	i32.add
	local.set	5
	local.get	3
	i32.const	-1
	i32.add
	local.set	6
	local.get	5
	local.set	1
	local.get	6
	local.set	2
	br      	0
	end_loop
	end_block
	local.get	4
	return
	end_function
