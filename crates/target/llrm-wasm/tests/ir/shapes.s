	.section	.text.tri,"",@
	.globl	tri
	.type	tri,@function
tri:
	.functype	tri (i32) -> (i32)
	block
	local.get	0
	i32.const	4
	i32.lt_s
	br_if   	0
	local.get	0
	call	side
	end_block
	local.get	0
	i32.const	1
	i32.add
	return
	end_function
	.section	.text.ifelse,"",@
	.globl	ifelse
	.type	ifelse,@function
ifelse:
	.functype	ifelse (i32, i32) -> (i32)
	.local  	i32
	block
	block
	local.get	0
	local.get	1
	i32.ge_s
	br_if   	0
	local.get	0
	i32.const	1
	i32.shl
	local.set	2
	br      	1
	end_block
	local.get	1
	i32.const	-7
	i32.add
	local.set	2
	end_block
	local.get	2
	i32.const	1
	i32.add
	return
	end_function
	.section	.text.chain,"",@
	.globl	chain
	.type	chain,@function
chain:
	.functype	chain (i32) -> (i32)
	.local  	i32
	block
	block
	local.get	0
	i32.const	1
	i32.ne
	br_if   	0
	i32.const	10
	local.set	1
	br      	1
	end_block
	block
	local.get	0
	i32.const	2
	i32.ne
	br_if   	0
	i32.const	20
	local.set	1
	br      	1
	end_block
	block
	local.get	0
	i32.const	3
	i32.ne
	br_if   	0
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
	.section	.text.andor,"",@
	.globl	andor
	.type	andor,@function
andor:
	.functype	andor (i32, i32, i32) -> (i32)
	.local  	i32
	block
	block
	block
	block
	local.get	0
	i32.eqz
	br_if   	0
	local.get	1
	br_if   	1
	end_block
	local.get	2
	i32.eqz
	br_if   	1
	end_block
	i32.const	1
	local.set	3
	br      	1
	end_block
	i32.const	2
	local.set	3
	end_block
	local.get	3
	return
	end_function
	.section	.text.nested_if,"",@
	.globl	nested_if
	.type	nested_if,@function
nested_if:
	.functype	nested_if (i32, i32) -> (i32)
	.local  	i32
	block
	block
	local.get	0
	i32.eqz
	br_if   	0
	block
	local.get	1
	i32.eqz
	br_if   	0
	i32.const	1
	local.set	2
	br      	2
	end_block
	i32.const	2
	local.set	2
	br      	1
	end_block
	block
	local.get	1
	i32.eqz
	br_if   	0
	i32.const	3
	local.set	2
	br      	1
	end_block
	i32.const	4
	local.set	2
	end_block
	local.get	2
	return
	end_function
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
	.section	.text.dowhile,"",@
	.globl	dowhile
	.type	dowhile,@function
dowhile:
	.functype	dowhile (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32
	i32.const	0
	local.set	1
	local.get	0
	local.set	2
	loop
	local.get	2
	local.set	3
	local.get	1
	call	get
	i32.add
	local.set	4
	local.get	3
	i32.const	-1
	i32.add
	local.set	5
	local.get	5
	i32.const	0
	i32.gt_s
	local.set	6
	local.get	4
	local.set	1
	local.get	5
	local.set	2
	local.get	6
	br_if   	0
	end_loop
	local.get	4
	return
	end_function
	.section	.text.forloop,"",@
	.globl	forloop
	.type	forloop,@function
forloop:
	.functype	forloop (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32
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
	local.get	5
	local.get	4
	local.get	4
	i32.mul
	i32.add
	local.set	6
	local.get	4
	i32.const	1
	i32.add
	local.set	7
	local.get	6
	local.set	2
	local.get	7
	local.set	3
	br      	0
	end_loop
	end_block
	local.get	5
	return
	end_function
	.section	.text.breaks,"",@
	.globl	breaks
	.type	breaks,@function
breaks:
	.functype	breaks (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32
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
	block
	call	get
	i32.const	5
	i32.ne
	br_if   	0
	br      	2
	end_block
	local.get	5
	local.get	4
	i32.add
	local.set	6
	local.get	4
	i32.const	1
	i32.add
	local.set	7
	local.get	6
	local.set	2
	local.get	7
	local.set	3
	br      	0
	end_loop
	end_block
	local.get	5
	return
	end_function
	.section	.text.conts,"",@
	.globl	conts
	.type	conts,@function
conts:
	.functype	conts (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32
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
	block
	block
	local.get	4
	i32.const	1
	i32.and
	i32.eqz
	br_if   	0
	local.get	5
	local.set	6
	br      	1
	end_block
	local.get	5
	local.get	4
	i32.add
	local.set	6
	end_block
	local.get	6
	local.set	7
	local.get	4
	i32.const	1
	i32.add
	local.set	8
	local.get	7
	local.set	2
	local.get	8
	local.set	3
	br      	0
	end_loop
	end_block
	local.get	5
	return
	end_function
	.section	.text.nested_loops,"",@
	.globl	nested_loops
	.type	nested_loops,@function
nested_loops:
	.functype	nested_loops (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32
	i32.const	0
	local.set	2
	local.get	2
	local.set	3
	local.get	2
	local.set	4
	block
	loop
	local.get	4
	local.set	5
	local.get	3
	local.set	6
	local.get	5
	local.get	0
	i32.ge_s
	br_if   	1
	i32.const	0
	local.set	7
	local.get	6
	local.set	8
	local.get	7
	local.set	9
	block
	loop
	local.get	9
	local.set	10
	local.get	8
	local.set	11
	local.get	10
	local.get	1
	i32.ge_s
	br_if   	1
	local.get	11
	local.get	5
	local.get	10
	i32.xor
	i32.add
	local.set	12
	local.get	10
	i32.const	1
	i32.add
	local.set	13
	local.get	12
	local.set	8
	local.get	13
	local.set	9
	br      	0
	end_loop
	end_block
	local.get	5
	i32.const	1
	i32.add
	local.set	14
	local.get	11
	local.set	3
	local.get	14
	local.set	4
	br      	0
	end_loop
	end_block
	local.get	6
	return
	end_function
	.section	.text.loop_in_if,"",@
	.globl	loop_in_if
	.type	loop_in_if,@function
loop_in_if:
	.functype	loop_in_if (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32
	i32.const	0
	local.set	2
	block
	local.get	0
	i32.eqz
	br_if   	0
	i32.const	0
	local.set	3
	local.get	3
	local.set	4
	local.get	3
	local.set	5
	block
	loop
	local.get	5
	local.set	6
	local.get	4
	local.set	7
	local.get	6
	local.get	1
	i32.ge_s
	br_if   	1
	local.get	7
	call	get
	i32.add
	local.set	8
	local.get	6
	i32.const	1
	i32.add
	local.set	9
	local.get	8
	local.set	4
	local.get	9
	local.set	5
	br      	0
	end_loop
	end_block
	local.get	7
	local.set	2
	end_block
	local.get	2
	return
	end_function
	.section	.text.if_in_loop,"",@
	.globl	if_in_loop
	.type	if_in_loop,@function
if_in_loop:
	.functype	if_in_loop (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32
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
	block
	block
	call	get
	i32.eqz
	br_if   	0
	local.get	5
	i32.const	1
	i32.add
	local.set	6
	br      	1
	end_block
	local.get	5
	i32.const	2
	i32.add
	local.set	6
	end_block
	local.get	6
	local.set	7
	local.get	4
	i32.const	1
	i32.add
	local.set	8
	local.get	7
	local.set	2
	local.get	8
	local.set	3
	br      	0
	end_loop
	end_block
	local.get	5
	return
	end_function
	.section	.text.calls_in_loop,"",@
	.globl	calls_in_loop
	.type	calls_in_loop,@function
calls_in_loop:
	.functype	calls_in_loop (i32) -> ()
	.local  	i32, i32, i32
	local.get	0
	local.set	1
	block
	loop
	local.get	1
	local.set	2
	local.get	2
	i32.const	-1
	i32.add
	local.set	3
	local.get	2
	i32.eqz
	br_if   	1
	local.get	3
	call	side
	local.get	3
	local.set	1
	br      	0
	end_loop
	end_block
	return
	end_function
	.section	.text.early,"",@
	.globl	early
	.type	early,@function
early:
	.functype	early (i32) -> (i32)
	.local  	i32
	block
	loop
	block
	call	get
	local.get	0
	i32.ne
	br_if   	0
	i32.const	1
	local.set	1
	br      	2
	end_block
	block
	call	get
	i32.const	100
	i32.lt_s
	br_if   	0
	i32.const	2
	local.set	1
	br      	2
	end_block
	br      	0
	end_loop
	end_block
	local.get	1
	return
	end_function
