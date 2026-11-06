	.section	.text.dense,"",@
	.globl	dense
	.type	dense,@function
dense:
	.functype	dense (i32) -> (i32)
	.local  	i32
	local.get	0
	drop
	local.get	0
	i32.const	5
	i32.gt_u
	drop
	block
	block
	block
	block
	block
	block
	block
	block
	local.get	0
	br_table 	{0, 1, 2, 3, 4, 5, 6}
	end_block
	i32.const	5
	local.set	1
	br      	6
	end_block
	i32.const	7
	local.set	1
	br      	5
	end_block
	i32.const	9
	local.set	1
	br      	4
	end_block
	i32.const	11
	local.set	1
	br      	3
	end_block
	i32.const	13
	local.set	1
	br      	2
	end_block
	call	get
	local.set	1
	br      	1
	end_block
	i32.const	-1
	local.set	1
	end_block
	local.get	1
	return
	end_function
	.section	.text.fall,"",@
	.globl	fall
	.type	fall,@function
fall:
	.functype	fall (i32) -> (i32)
	.local  	i32, i32, i32
	i32.const	0
	local.set	1
	local.get	0
	drop
	local.get	0
	i32.const	10
	i32.gt_u
	drop
	local.get	1
	local.set	2
	block
	block
	block
	block
	block
	block
	block
	block
	local.get	0
	br_table 	{0, 1, 2, 6, 6, 6, 6, 3, 6, 4, 5, 6}
	end_block
	i32.const	1
	local.set	2
	end_block
	local.get	2
	i32.const	2
	i32.add
	local.set	3
	br      	5
	end_block
	i32.const	4
	local.set	3
	br      	4
	end_block
	i32.const	8
	local.set	3
	br      	3
	end_block
	i32.const	16
	local.set	3
	br      	2
	end_block
	i32.const	32
	local.set	3
	br      	1
	end_block
	i32.const	64
	local.set	3
	end_block
	local.get	3
	return
	end_function
