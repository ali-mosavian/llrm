	.section	.text.st,"",@
	.globl	st                              # -- Begin function st
	.type	st,@function
st:                                     # @st
	.functype	st (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i32.store	0
	return
	end_function
                                        # -- End function
