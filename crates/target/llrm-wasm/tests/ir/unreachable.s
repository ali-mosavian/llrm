	.section	.text.f,"",@
	.globl	f                               # -- Begin function f
	.type	f,@function
f:                                      # @f
	.functype	f (i32) -> (i32)
# %bb.0:                                # %entry
	block   	
	local.get	0
	br_if   	0                               # 0: down to label0
# %bb.1:                                # %bad
	i32.const	11
	call	die
	unreachable
.LBB0_2:                                # %ok
	end_block                               # label0:
	local.get	0
	return
	end_function
                                        # -- End function
	.section	.text.g,"",@
	.globl	g                               # -- Begin function g
	.type	g,@function
g:                                      # @g
	.functype	g (i32) -> (i32)
# %bb.0:                                # %entry
	i32.const	11
	call	die
# %bb.1:                                # %bad
	unreachable
	end_function
                                        # -- End function
