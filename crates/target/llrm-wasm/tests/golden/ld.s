	.section	.text.ld,"",@
	.globl	ld                              # -- Begin function ld
	.type	ld,@function
ld:                                     # @ld
	.functype	ld (i32) -> (i32)
	.local  	i32
# %bb.0:
	local.get	0
	i32.load	0
	local.set	1
	local.get	1
	return
	end_function
                                        # -- End function
