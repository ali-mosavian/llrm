	.section	.text.sum,"",@
	.globl	sum                             # -- Begin function sum
	.type	sum,@function
sum:                                    # @sum
	.functype	sum (i32, i32) -> (i32)
	.local  	i32, i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	2
	local.get	2
	local.get	0
	i32.store	12
	i32.const	0
	local.set	3
	local.get	2
	local.get	3
	i32.store	4
	local.get	2
	local.get	1
	i32.store	8
	local.get	2
	local.get	3
	i32.store	0
.LBB0_1:                                # =>This Inner Loop Header: Depth=1
	block   	
	loop    	                                # label1:
	local.get	2
	i32.load	0
	local.get	2
	i32.load	12
	i32.ge_s
	br_if   	1                               # 1: down to label0
# %bb.2:                                #   in Loop: Header=BB0_1 Depth=1
	local.get	2
	i32.load	8
	local.set	4
	local.get	2
	local.get	4
	i32.const	4
	i32.add 
	i32.store	8
	local.get	2
	local.get	4
	i32.load	0
	local.get	2
	i32.load	4
	i32.add 
	i32.store	4
# %bb.3:                                #   in Loop: Header=BB0_1 Depth=1
	local.get	2
	local.get	2
	i32.load	0
	i32.const	1
	i32.add 
	i32.store	0
	br      	0                               # 0: up to label1
.LBB0_4:
	end_loop
	end_block                               # label0:
	local.get	2
	i32.load	4
	return
	end_function
                                        # -- End function
	.section	.text.wrap,"",@
	.globl	wrap                            # -- Begin function wrap
	.type	wrap,@function
wrap:                                   # @wrap
	.functype	wrap (i32, i32) -> (i32)
	.local  	i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	2
	local.get	2
	global.set	__stack_pointer
	local.get	2
	local.get	0
	i32.store	12
	local.get	2
	local.get	1
	i32.store	8
	local.get	2
	local.get	2
	i32.load	12
	local.get	2
	i32.load	8
	call	forward
	i32.store	4
	local.get	2
	i32.load	4
	local.set	3
	local.get	2
	i32.const	16
	i32.add 
	global.set	__stack_pointer
	local.get	3
	return
	end_function
                                        # -- End function
	.section	.text.copied,"",@
	.globl	copied                          # -- Begin function copied
	.type	copied,@function
copied:                                 # @copied
	.functype	copied (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	2
	local.get	2
	local.get	0
	i32.store	12
	local.get	2
	i32.const	0
	i32.store	0
	local.get	2
	local.get	1
	i32.store	8
	local.get	2
	local.get	2
	i32.load	8
	i32.store	4
	local.get	2
	i32.load	8
	local.set	3
	i32.const	4
	local.set	4
	local.get	2
	local.get	3
	local.get	4
	i32.add 
	i32.store	8
	local.get	2
	local.get	3
	i32.load	0
	local.get	2
	i32.load	0
	i32.add 
	i32.store	0
	local.get	2
	i32.load	4
	local.set	5
	local.get	2
	local.get	5
	local.get	4
	i32.add 
	i32.store	4
	local.get	2
	local.get	5
	i32.load	0
	local.get	2
	i32.load	0
	i32.add 
	i32.store	0
	local.get	2
	i32.load	0
	return
	end_function
                                        # -- End function
