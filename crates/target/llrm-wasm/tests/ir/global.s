	.section	.text.rg,"",@
	.globl	rg                              # -- Begin function rg
	.type	rg,@function
rg:                                     # @rg
	.functype	rg () -> (i32)
# %bb.0:
	i32.const	0
	i32.load	g
	return
	end_function
                                        # -- End function
	.section	.text.wg,"",@
	.globl	wg                              # -- Begin function wg
	.type	wg,@function
wg:                                     # @wg
	.functype	wg (i32) -> ()
# %bb.0:
	i32.const	0
	local.get	0
	i32.store	g
	return
	end_function
                                        # -- End function
	.section	.text.rh,"",@
	.globl	rh                              # -- Begin function rh
	.type	rh,@function
rh:                                     # @rh
	.functype	rh () -> (i64)
# %bb.0:
	i32.const	0
	i64.load	h
	return
	end_function
                                        # -- End function
	.section	.text.ag,"",@
	.globl	ag                              # -- Begin function ag
	.type	ag,@function
ag:                                     # @ag
	.functype	ag () -> (i32)
# %bb.0:
	i32.const	g
	return
	end_function
                                        # -- End function
	.section	.text.ra2,"",@
	.globl	ra2                             # -- Begin function ra2
	.type	ra2,@function
ra2:                                    # @ra2
	.functype	ra2 () -> (i32)
# %bb.0:
	i32.const	8
	i32.load	arr
	return
	end_function
                                        # -- End function
	.section	.text.rai,"",@
	.globl	rai                             # -- Begin function rai
	.type	rai,@function
rai:                                    # @rai
	.functype	rai (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	2
	i32.shl 
	i32.const	arr
	i32.add 
	i32.load	0
	return
	end_function
                                        # -- End function
