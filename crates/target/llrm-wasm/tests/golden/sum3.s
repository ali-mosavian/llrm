	.section	.text.sum3,"",@
	.globl	sum3                            # -- Begin function sum3
	.type	sum3,@function
sum3:                                   # @sum3
	.functype	sum3 (i32, i32, i32) -> (i32)
	.local  	i32, i32
# %bb.0:
	local.get	0
	local.get	1
	i32.add 
	local.set	3
	local.get	3
	local.get	2
	i32.add 
	local.set	4
	local.get	4
	return
	end_function
                                        # -- End function
