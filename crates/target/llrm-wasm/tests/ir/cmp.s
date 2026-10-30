	.section	.text.eq,"",@
	.globl	eq                              # -- Begin function eq
	.type	eq,@function
eq:                                     # @eq
	.functype	eq (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.eq  
	return
	end_function
                                        # -- End function
	.section	.text.slt,"",@
	.globl	slt                             # -- Begin function slt
	.type	slt,@function
slt:                                    # @slt
	.functype	slt (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.lt_s
	return
	end_function
                                        # -- End function
	.section	.text.ule,"",@
	.globl	ule                             # -- Begin function ule
	.type	ule,@function
ule:                                    # @ule
	.functype	ule (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.le_u
	return
	end_function
                                        # -- End function
	.section	.text.sel,"",@
	.globl	sel                             # -- Begin function sel
	.type	sel,@function
sel:                                    # @sel
	.functype	sel (i32, i32, i32, i32) -> (i32)
# %bb.0:
	local.get	2
	local.get	3
	local.get	0
	local.get	1
	i32.gt_u
	i32.select
	return
	end_function
                                        # -- End function
	.section	.text.eq64,"",@
	.globl	eq64                            # -- Begin function eq64
	.type	eq64,@function
eq64:                                   # @eq64
	.functype	eq64 (i64, i64) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i64.ne  
	return
	end_function
                                        # -- End function
	.section	.text.isz,"",@
	.globl	isz                             # -- Begin function isz
	.type	isz,@function
isz:                                    # @isz
	.functype	isz (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.eqz
	return
	end_function
                                        # -- End function
