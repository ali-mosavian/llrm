	.section	.text.g,"",@
	.globl	g                               # -- Begin function g
	.type	g,@function
g:                                      # @g
	.functype	g (i32, i32) -> (i32)
# %bb.0:
	local.get	1
	local.get	0
	i32.load8_u	0
	i32.store8	0
	local.get	1
	i32.load8_s	0
	return
	end_function
                                        # -- End function
