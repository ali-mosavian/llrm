	.section	.text.f1,"",@
	.globl	f1
	.type	f1,@function
f1:
	.functype	f1 (i32) -> (i32)
	.local  	i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	local.get	1
	i32.load	12
	i32.store	8
	local.get	1
	i32.load	8
	i32.const	1
	i32.add
	return
	end_function
	.section	.text.f2,"",@
	.globl	f2
	.type	f2,@function
f2:
	.functype	f2 (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	32
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	local.get	0
	i32.store	28
	local.get	1
	local.get	1
	i32.load	28
	i32.store	0
	local.get	1
	call	get
	i32.store	4
	local.get	1
	i32.load	0
	local.get	1
	i32.load	4
	i32.add
	local.set	2
	local.get	1
	i32.const	32
	i32.add
	global.set	__stack_pointer
	local.get	2
	return
	end_function
	.section	.text.addr_taken,"",@
	.globl	addr_taken
	.type	addr_taken,@function
addr_taken:
	.functype	addr_taken (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	local.get	1
	i32.load	12
	i32.store	8
	local.get	1
	i32.const	8
	i32.add
	call	take
	local.get	1
	i32.load	8
	local.set	2
	local.get	1
	i32.const	16
	i32.add
	global.set	__stack_pointer
	local.get	2
	return
	end_function
	.section	.text.arr_local,"",@
	.globl	arr_local
	.type	arr_local,@function
arr_local:
	.functype	arr_local (i32) -> (i32)
	.local  	i32, i32, i32
	global.get	__stack_pointer
	i32.const	64
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	60
	local.get	1
	i32.const	0
	i32.store	12
	block
	loop
	local.get	1
	i32.load	12
	i32.const	9
	i32.gt_s
	br_if   	1
	local.get	1
	i32.load	12
	local.set	2
	local.get	2
	local.get	1
	i32.load	60
	i32.mul
	local.set	3
	local.get	2
	i32.const	2
	i32.shl
	local.get	1
	i32.const	16
	i32.add
	i32.add
	local.get	3
	i32.store	0
	local.get	1
	local.get	1
	i32.load	12
	i32.const	1
	i32.add
	i32.store	12
	br      	0
	end_loop
	end_block
	local.get	1
	i32.load	28
	local.get	1
	i32.load	52
	i32.add
	return
	end_function
	.section	.text.char_local,"",@
	.globl	char_local
	.type	char_local,@function
char_local:
	.functype	char_local (i32) -> (i32)
	.local  	i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	local.get	1
	i32.load	12
	i32.store8	11
	local.get	1
	local.get	1
	i32.load	12
	i32.const	1
	i32.add
	i32.store16	8
	local.get	1
	i32.load8_s	11
	local.get	1
	i32.load16_s	8
	i32.add
	return
	end_function
	.section	.text.ll_local,"",@
	.globl	ll_local
	.type	ll_local,@function
ll_local:
	.functype	ll_local (i64) -> (i64)
	.local  	i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i64.store	8
	local.get	1
	local.get	1
	i64.load	8
	i64.const	3
	i64.mul
	i64.store	0
	local.get	1
	i64.load	0
	i64.const	1
	i64.add
	return
	end_function
	.section	.text.struct_local,"",@
	.globl	struct_local
	.type	struct_local,@function
struct_local:
	.functype	struct_local (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	local.get	1
	i32.load	12
	i32.store	4
	local.get	1
	call	get
	i32.store	8
	local.get	1
	i32.load	4
	local.get	1
	i32.load	8
	i32.add
	local.set	2
	local.get	1
	i32.const	16
	i32.add
	global.set	__stack_pointer
	local.get	2
	return
	end_function
	.section	.text.struct_q,"",@
	.globl	struct_q
	.type	struct_q,@function
struct_q:
	.functype	struct_q (i32) -> (i32)
	.local  	i32
	global.get	__stack_pointer
	i32.const	32
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	28
	local.get	1
	local.get	1
	i32.load	28
	i32.store8	8
	local.get	1
	i32.const	2
	i32.store16	10
	local.get	1
	local.get	1
	i32.load	28
	i32.store	12
	local.get	1
	local.get	1
	i64.load32_s	28
	i64.store	16
	local.get	1
	i32.load8_s	8
	local.get	1
	i32.load16_s	10
	i32.add
	local.get	1
	i32.load	12
	i32.add
	local.get	1
	i32.load	16
	i32.add
	return
	end_function
	.section	.text.nested_calls,"",@
	.globl	nested_calls
	.type	nested_calls,@function
nested_calls:
	.functype	nested_calls (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	call	get
	i32.store	8
	local.get	1
	call	get
	i32.store	4
	local.get	1
	i32.load	8
	local.get	1
	i32.load	4
	i32.add
	local.get	1
	i32.load	12
	i32.add
	call	put
	local.get	1
	i32.load	8
	local.get	1
	i32.load	4
	i32.sub
	local.set	2
	local.get	1
	i32.const	16
	i32.add
	global.set	__stack_pointer
	local.get	2
	return
	end_function
	.section	.text.globals,"",@
	.globl	globals
	.type	globals,@function
globals:
	.functype	globals (i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	12
	local.get	1
	i32.load	12
	local.set	2
	i32.const	0
	local.set	3
	local.get	3
	local.get	2
	i32.store	g1
	local.get	3
	i32.load	g1
	i32.const	1
	i32.add
	local.set	4
	i32.const	12
	local.set	5
	local.get	5
	local.get	4
	i32.store	garr
	local.get	5
	i32.load	garr
	local.set	6
	i32.const	4
	local.set	7
	local.get	7
	local.get	6
	i32.store	gp
	local.get	7
	i32.load	gp
	local.get	3
	i32.load	g1
	i32.add
	return
	end_function
	.section	.text.ptr_arith,"",@
	.globl	ptr_arith
	.type	ptr_arith,@function
ptr_arith:
	.functype	ptr_arith (i32, i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	2
	local.get	2
	local.get	0
	i32.store	12
	local.get	2
	local.get	1
	i32.store	8
	i32.const	0
	local.set	3
	local.get	2
	local.get	3
	i32.store	4
	local.get	2
	local.get	3
	i32.store	0
	block
	loop
	local.get	2
	i32.load	0
	local.get	2
	i32.load	8
	i32.ge_s
	br_if   	1
	local.get	2
	local.get	2
	i32.load	12
	local.get	2
	i32.load	0
	i32.const	2
	i32.shl
	i32.add
	i32.load	0
	local.get	2
	i32.load	4
	i32.add
	i32.store	4
	local.get	2
	local.get	2
	i32.load	0
	i32.const	1
	i32.add
	i32.store	0
	br      	0
	end_loop
	end_block
	local.get	2
	i32.load	4
	return
	end_function
	.section	.text.bytes,"",@
	.globl	bytes
	.type	bytes,@function
bytes:
	.functype	bytes (i32, i32) -> ()
	.local  	i32, i32, i32
	global.get	__stack_pointer
	i32.const	48
	i32.sub
	local.set	2
	local.get	2
	global.set	__stack_pointer
	local.get	2
	local.get	0
	i32.store	44
	local.get	2
	local.get	1
	i32.store	40
	local.get	2
	i32.const	0
	i32.store	12
	block
	loop
	local.get	2
	i32.load	12
	i32.const	15
	i32.gt_s
	br_if   	1
	local.get	2
	i32.load	12
	local.set	3
	local.get	3
	local.get	2
	i32.const	16
	i32.add
	i32.add
	local.get	3
	i32.store8	0
	local.get	2
	local.get	2
	i32.load	12
	i32.const	1
	i32.add
	i32.store	12
	br      	0
	end_loop
	end_block
	local.get	2
	i32.load	40
	local.set	4
	local.get	2
	i32.const	16
	i32.add
	local.get	4
	call	takep
	local.get	2
	i32.const	48
	i32.add
	global.set	__stack_pointer
	return
	end_function
	.section	.text.cond_locals,"",@
	.globl	cond_locals
	.type	cond_locals,@function
cond_locals:
	.functype	cond_locals (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	local.get	0
	i32.store	12
	block
	block
	local.get	1
	i32.load	12
	i32.eqz
	br_if   	0
	local.get	1
	call	get
	i32.store	4
	local.get	1
	local.get	1
	i32.load	4
	i32.store	8
	br      	1
	end_block
	local.get	1
	local.get	1
	i32.load	12
	i32.const	1
	i32.add
	i32.store	0
	local.get	1
	local.get	1
	i32.load	0
	i32.store	8
	end_block
	local.get	1
	i32.load	8
	local.set	2
	local.get	1
	i32.const	16
	i32.add
	global.set	__stack_pointer
	local.get	2
	return
	end_function
	.section	.text.loop_locals,"",@
	.globl	loop_locals
	.type	loop_locals,@function
loop_locals:
	.functype	loop_locals (i32) -> (i32)
	.local  	i32, i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	1
	local.get	1
	local.get	0
	i32.store	12
	i32.const	0
	local.set	2
	local.get	1
	local.get	2
	i32.store	8
	local.get	1
	local.get	2
	i32.store	4
	block
	loop
	local.get	1
	i32.load	4
	local.get	1
	i32.load	12
	i32.ge_s
	br_if   	1
	local.get	1
	local.get	1
	i32.load	4
	i32.const	1
	i32.shl
	i32.store	0
	local.get	1
	local.get	1
	i32.load	0
	local.get	1
	i32.load	8
	i32.add
	i32.store	8
	local.get	1
	local.get	1
	i32.load	4
	i32.const	1
	i32.add
	i32.store	4
	br      	0
	end_loop
	end_block
	local.get	1
	i32.load	8
	return
	end_function
