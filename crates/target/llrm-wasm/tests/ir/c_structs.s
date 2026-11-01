	.section	.text.g,"",@
	.globl	g
	.type	g,@function
g:
	.functype	g (i32, i32, i32, i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32
	global.get	__stack_pointer
	i32.const	384
	i32.sub
	local.set	5
	local.get	5
	global.set	__stack_pointer
	local.get	5
	local.get	0
	i32.store8	383
	local.get	5
	local.get	2
	i32.load	8
	i32.store	248
	local.get	5
	local.get	2
	i64.load	0:p2align=2
	i64.store	240
	local.get	5
	local.get	2
	i32.load	8
	i32.store	232
	local.get	5
	local.get	2
	i64.load	0:p2align=2
	i64.store	224
	local.get	5
	i32.const	368
	i32.add
	local.get	5
	i32.const	240
	i32.add
	local.get	5
	i32.const	224
	i32.add
	call	f12
	local.get	5
	local.get	4
	i64.load	8
	i64.store	216
	local.get	5
	local.get	4
	i64.load	0
	i64.store	208
	local.get	5
	i32.const	352
	i32.add
	local.get	5
	i32.const	208
	i32.add
	call	fm
	i32.const	100
	local.set	6
	local.get	5
	i32.const	108
	i32.add
	local.get	3
	local.get	6
	memory.copy	0, 0
	i32.const	3
	local.set	7
	local.get	5
	i32.const	252
	i32.add
	local.get	5
	i32.const	108
	i32.add
	local.get	7
	call	big
	local.get	5
	i32.load8_u	383
	call	f1
	local.set	8
	local.get	5
	local.get	1
	i32.load8_u	2
	i32.store8	106
	local.get	5
	local.get	1
	i32.load16_u	0:p2align=0
	i32.store16	104
	local.get	8
	local.get	5
	i32.const	104
	i32.add
	call	f3
	i32.add
	local.get	5
	i32.load	372
	i32.add
	local.set	9
	local.get	5
	i32.load	260
	local.set	10
	local.get	5
	i32.const	4
	i32.add
	local.get	3
	local.get	6
	memory.copy	0, 0
	local.get	9
	local.get	5
	i32.const	4
	i32.add
	local.get	10
	call	f100
	i32.add
	local.get	5
	f64.load	352
	i32.trunc_sat_f64_s
	i32.add
	local.set	11
	local.get	5
	i32.const	384
	i32.add
	global.set	__stack_pointer
	local.get	11
	return
	end_function
	.section	.text.mk,"",@
	.globl	mk
	.type	mk,@function
mk:
	.functype	mk (i32, i32) -> ()
	.local  	i32
	global.get	__stack_pointer
	i32.const	16
	i32.sub
	local.set	2
	local.get	2
	local.get	1
	i32.store	12
	local.get	0
	local.get	2
	i32.load	12
	i32.store	0
	local.get	0
	local.get	2
	i32.load	12
	i32.const	1
	i32.add
	i32.store	4
	local.get	0
	local.get	2
	i32.load	12
	i32.const	2
	i32.add
	i32.store	8
	return
	end_function
	.section	.text.takes,"",@
	.globl	takes
	.type	takes,@function
takes:
	.functype	takes (i32) -> (i32)
	local.get	0
	i32.load	96
	return
	end_function
	.section	.text.pass,"",@
	.globl	pass
	.type	pass,@function
pass:
	.functype	pass (i32, i32) -> ()
	local.get	1
	i32.const	5
	i32.store	4
	local.get	0
	local.get	1
	i32.const	100
	memory.copy	0, 0
	return
	end_function
