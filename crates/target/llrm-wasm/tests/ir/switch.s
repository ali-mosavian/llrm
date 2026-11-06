	.section	.text.small,"",@
	.globl	small
	.type	small,@function
small:
	.functype	small (i32) -> (i32)
	.local  	i32, i32
	local.get	0
	i32.const	-1
	i32.add
	local.set	1
	local.get	1
	i32.const	1
	i32.gt_u
	drop
	block
	block
	block
	block
	local.get	1
	br_table 	{0, 1, 2}
	end_block
	i32.const	10
	local.set	2
	br      	2
	end_block
	i32.const	20
	local.set	2
	br      	1
	end_block
	i32.const	0
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.comp,"",@
	.globl	comp
	.type	comp,@function
comp:
	.functype	comp (i32) -> (i32)
	.local  	i32
	local.get	0
	i32.const	2
	i32.gt_u
	drop
	block
	block
	block
	block
	block
	local.get	0
	br_table 	{0, 1, 2, 3}
	end_block
	i32.const	10
	local.set	1
	br      	3
	end_block
	i32.const	20
	local.set	1
	br      	2
	end_block
	i32.const	30
	local.set	1
	br      	1
	end_block
	i32.const	0
	local.set	1
	end_block
	local.get	1
	return
	end_function
	.section	.text.sparse,"",@
	.globl	sparse
	.type	sparse,@function
sparse:
	.functype	sparse (i32) -> (i32)
	.local  	i32, i32
	call	get
	local.set	1
	block
	block
	block
	block
	block
	block
	local.get	1
	i32.const	1
	i32.eq
	br_if   	0
	local.get	1
	i32.const	100
	i32.eq
	br_if   	1
	local.get	1
	i32.const	1000
	i32.eq
	br_if   	2
	local.get	1
	i32.const	10000
	i32.eq
	br_if   	3
	br      	4
	end_block
	i32.const	10
	local.set	2
	br      	4
	end_block
	i32.const	20
	local.set	2
	br      	3
	end_block
	i32.const	30
	local.set	2
	br      	2
	end_block
	i32.const	40
	local.set	2
	br      	1
	end_block
	i32.const	0
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.merge,"",@
	.globl	merge
	.type	merge,@function
merge:
	.functype	merge (i32) -> (i32)
	.local  	i32, i32
	call	get
	local.set	1
	local.get	1
	i32.const	3
	i32.gt_u
	drop
	block
	block
	block
	block
	block
	block
	local.get	1
	br_table 	{0, 1, 2, 3, 4}
	end_block
	i32.const	1
	local.set	2
	br      	4
	end_block
	i32.const	2
	local.set	2
	br      	3
	end_block
	i32.const	3
	local.set	2
	br      	2
	end_block
	local.get	0
	local.set	2
	br      	1
	end_block
	i32.const	9
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.loop,"",@
	.globl	loop
	.type	loop,@function
loop:
	.functype	loop (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32, i32
	i32.const	0
	local.set	1
	local.get	1
	local.set	2
	local.get	1
	local.set	3
	block
	loop
	local.get	3
	local.set	4
	local.get	2
	local.set	5
	local.get	4
	local.get	0
	i32.ge_s
	br_if   	1
	local.get	4
	i32.const	3
	i32.and
	local.set	6
	local.get	6
	i32.const	2
	i32.gt_u
	drop
	block
	block
	block
	block
	block
	local.get	6
	br_table 	{0, 1, 2, 3}
	end_block
	local.get	5
	i32.const	1
	i32.add
	local.set	7
	br      	3
	end_block
	local.get	5
	i32.const	2
	i32.add
	local.set	7
	br      	2
	end_block
	local.get	5
	i32.const	3
	i32.add
	local.set	7
	br      	1
	end_block
	local.get	5
	i32.const	4
	i32.add
	local.set	7
	end_block
	local.get	7
	local.set	8
	local.get	4
	i32.const	1
	i32.add
	local.set	9
	local.get	8
	local.set	2
	local.get	9
	local.set	3
	br      	0
	end_loop
	end_block
	local.get	5
	return
	end_function
	.section	.text.calls,"",@
	.globl	calls
	.type	calls,@function
calls:
	.functype	calls (i32) -> ()
	.local  	i32
	call	get
	local.set	1
	local.get	1
	i32.const	3
	i32.gt_u
	drop
	block
	block
	block
	block
	block
	block
	local.get	1
	br_table 	{0, 1, 2, 3, 4}
	end_block
	i32.const	0
	call	put
	br      	4
	end_block
	i32.const	1
	call	put
	br      	3
	end_block
	i32.const	2
	call	put
	end_block
	i32.const	3
	call	put
	br      	1
	end_block
	i32.const	9
	call	put
	end_block
	return
	end_function
	.section	.text.two,"",@
	.globl	two
	.type	two,@function
two:
	.functype	two (i32) -> (i32)
	.local  	i32, i32
	call	get
	i32.const	-5
	i32.add
	local.set	1
	local.get	1
	i32.const	1
	i32.gt_u
	drop
	block
	block
	block
	block
	local.get	1
	br_table 	{0, 1, 2}
	end_block
	i32.const	1
	local.set	2
	br      	2
	end_block
	i32.const	2
	local.set	2
	br      	1
	end_block
	i32.const	0
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.one,"",@
	.globl	one
	.type	one,@function
one:
	.functype	one (i32) -> (i32)
	.local  	i32
	block
	block
	call	get
	i32.const	5
	i32.ne
	br_if   	0
	i32.const	1
	local.set	1
	br      	1
	end_block
	i32.const	0
	local.set	1
	end_block
	local.get	1
	return
	end_function
	.section	.text.nodef,"",@
	.globl	nodef
	.type	nodef,@function
nodef:
	.functype	nodef (i32) -> (i32)
	.local  	i32, i32
	call	get
	local.set	1
	local.get	1
	i32.const	5
	i32.gt_u
	drop
	block
	block
	block
	block
	local.get	1
	br_table 	{0, 0, 0, 0, 1, 1, 2}
	end_block
	i32.const	7
	local.set	2
	br      	2
	end_block
	i32.const	8
	local.set	2
	br      	1
	end_block
	i32.const	0
	local.set	2
	end_block
	local.get	2
	return
	end_function
