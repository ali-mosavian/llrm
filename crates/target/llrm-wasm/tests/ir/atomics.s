	.section	.text.ld,"",@
	.globl	ld                              # -- Begin function ld
	.type	ld,@function
ld:                                     # @ld
	.functype	ld () -> (i32)
# %bb.0:
	i32.const	0
	i32.load	shown
	return
	end_function
                                        # -- End function
	.section	.text.st,"",@
	.globl	st                              # -- Begin function st
	.type	st,@function
st:                                     # @st
	.functype	st (i32) -> ()
# %bb.0:
	i32.const	0
	local.get	0
	i32.store	shown
	return
	end_function
                                        # -- End function
	.section	.text.ldv,"",@
	.globl	ldv                             # -- Begin function ldv
	.type	ldv,@function
ldv:                                    # @ldv
	.functype	ldv (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.load	0
	return
	end_function
                                        # -- End function
