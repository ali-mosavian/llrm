	.section	.text.shifts,"",@
	.globl	shifts                          # -- Begin function shifts
	.type	shifts,@function
shifts:                                 # @shifts
	.functype	shifts (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.shl 
	local.get	1
	i32.shr_u
	i32.const	3
	i32.shr_s
	return
	end_function
                                        # -- End function
	.section	.text.shl,"",@
	.globl	shl                             # -- Begin function shl
	.type	shl,@function
shl:                                    # @shl
	.functype	shl (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.shl 
	return
	end_function
                                        # -- End function
	.section	.text.shr_s,"",@
	.globl	shr_s                           # -- Begin function shr_s
	.type	shr_s,@function
shr_s:                                  # @shr_s
	.functype	shr_s (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.shr_s
	return
	end_function
                                        # -- End function
	.section	.text.shr_u,"",@
	.globl	shr_u                           # -- Begin function shr_u
	.type	shr_u,@function
shr_u:                                  # @shr_u
	.functype	shr_u (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.shr_u
	return
	end_function
                                        # -- End function
	.section	.text.shl64,"",@
	.globl	shl64                           # -- Begin function shl64
	.type	shl64,@function
shl64:                                  # @shl64
	.functype	shl64 (i64, i64) -> (i64)
# %bb.0:
	local.get	0
	local.get	1
	i64.shl 
	return
	end_function
                                        # -- End function
	.section	.text.shr_s64,"",@
	.globl	shr_s64                         # -- Begin function shr_s64
	.type	shr_s64,@function
shr_s64:                                # @shr_s64
	.functype	shr_s64 (i64) -> (i64)
# %bb.0:
	local.get	0
	i64.const	7
	i64.shr_s
	return
	end_function
                                        # -- End function
