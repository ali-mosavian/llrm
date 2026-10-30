	.section	.text.callv,"",@
	.globl	callv                           # -- Begin function callv
	.type	callv,@function
callv:                                  # @callv
	.functype	callv () -> ()
# %bb.0:
	call	v0
	return
	end_function
                                        # -- End function
	.section	.text.calli,"",@
	.globl	calli                           # -- Begin function calli
	.type	calli,@function
calli:                                  # @calli
	.functype	calli () -> (i32)
# %bb.0:
	call	i0
	return
	end_function
                                        # -- End function
	.section	.text.call2,"",@
	.globl	call2                           # -- Begin function call2
	.type	call2,@function
call2:                                  # @call2
	.functype	call2 (i32, i32) -> (i32)
# %bb.0:
	local.get	1
	local.get	0
	call	i2
	return
	end_function
                                        # -- End function
	.section	.text.call64,"",@
	.globl	call64                          # -- Begin function call64
	.type	call64,@function
call64:                                 # @call64
	.functype	call64 (i64) -> (i64)
# %bb.0:
	local.get	0
	call	l1
	return
	end_function
                                        # -- End function
	.section	.text.twice,"",@
	.globl	twice                           # -- Begin function twice
	.type	twice,@function
twice:                                  # @twice
	.functype	twice (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	1
	call	i2
	local.get	0
	call	i2
	return
	end_function
                                        # -- End function
	.section	.text.callp,"",@
	.globl	callp                           # -- Begin function callp
	.type	callp,@function
callp:                                  # @callp
	.functype	callp (i32) -> ()
# %bb.0:
	local.get	0
	i32.const	42
	call	vp
	return
	end_function
                                        # -- End function
	.section	.text.tail,"",@
	.globl	tail                            # -- Begin function tail
	.type	tail,@function
tail:                                   # @tail
	.functype	tail (i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	0
	call	i2
	i32.const	1
	i32.add 
	return
	end_function
                                        # -- End function
