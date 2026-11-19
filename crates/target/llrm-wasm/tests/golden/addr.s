	.section	.text.addr,"",@
	.globl	addr                            # -- Begin function addr
	.type	addr,@function
addr:                                   # @addr
	.functype	addr () -> (i32)
	.local  	i32, i32
# %bb.0:
	i32.const	0
	local.set	0
	local.get	0
	i32.load	64
	local.set	1
	local.get	1
	return
	end_function
                                        # -- End function
