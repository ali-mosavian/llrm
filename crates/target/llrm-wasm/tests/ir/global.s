	.section	.text.rg,"",@
	.globl	rg
	.type	rg,@function
rg:
	.functype	rg () -> (i32)
	i32.const	0
	i32.load	g
	return
	end_function
	.section	.text.wg,"",@
	.globl	wg
	.type	wg,@function
wg:
	.functype	wg (i32) -> ()
	i32.const	0
	local.get	0
	i32.store	g
	return
	end_function
	.section	.text.rh,"",@
	.globl	rh
	.type	rh,@function
rh:
	.functype	rh () -> (i64)
	i32.const	0
	i64.load	h
	return
	end_function
	.section	.text.ag,"",@
	.globl	ag
	.type	ag,@function
ag:
	.functype	ag () -> (i32)
	i32.const	g
	return
	end_function
	.section	.text.ra2,"",@
	.globl	ra2
	.type	ra2,@function
ra2:
	.functype	ra2 () -> (i32)
	i32.const	8
	i32.load	arr
	return
	end_function
	.section	.text.rai,"",@
	.globl	rai
	.type	rai,@function
rai:
	.functype	rai (i32) -> (i32)
	local.get	0
	i32.const	2
	i32.shl
	i32.const	arr
	i32.add
	i32.load	0
	return
	end_function
