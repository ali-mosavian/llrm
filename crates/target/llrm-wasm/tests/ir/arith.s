	.section	.text.add,"",@
	.globl	add                             # -- Begin function add
	.type	add,@function
add:                                    # @add
	.functype	add (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.add 
	return
	end_function
                                        # -- End function
	.section	.text.sub,"",@
	.globl	sub                             # -- Begin function sub
	.type	sub,@function
sub:                                    # @sub
	.functype	sub (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.sub 
	return
	end_function
                                        # -- End function
	.section	.text.mul,"",@
	.globl	mul                             # -- Begin function mul
	.type	mul,@function
mul:                                    # @mul
	.functype	mul (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.mul 
	return
	end_function
                                        # -- End function
	.section	.text.sdiv,"",@
	.globl	sdiv                            # -- Begin function sdiv
	.type	sdiv,@function
sdiv:                                   # @sdiv
	.functype	sdiv (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.div_s
	return
	end_function
                                        # -- End function
	.section	.text.udiv,"",@
	.globl	udiv                            # -- Begin function udiv
	.type	udiv,@function
udiv:                                   # @udiv
	.functype	udiv (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.div_u
	return
	end_function
                                        # -- End function
	.section	.text.srem,"",@
	.globl	srem                            # -- Begin function srem
	.type	srem,@function
srem:                                   # @srem
	.functype	srem (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.rem_s
	return
	end_function
                                        # -- End function
	.section	.text.urem,"",@
	.globl	urem                            # -- Begin function urem
	.type	urem,@function
urem:                                   # @urem
	.functype	urem (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.rem_u
	return
	end_function
                                        # -- End function
	.section	.text.add64,"",@
	.globl	add64                           # -- Begin function add64
	.type	add64,@function
add64:                                  # @add64
	.functype	add64 (i64, i64) -> (i64)
# %bb.0:
	local.get	0
	local.get	1
	i64.add 
	return
	end_function
                                        # -- End function
	.section	.text.mul64,"",@
	.globl	mul64                           # -- Begin function mul64
	.type	mul64,@function
mul64:                                  # @mul64
	.functype	mul64 (i64, i64) -> (i64)
# %bb.0:
	local.get	0
	local.get	1
	i64.mul 
	return
	end_function
                                        # -- End function
	.section	.text.consts,"",@
	.globl	consts                          # -- Begin function consts
	.type	consts,@function
consts:                                 # @consts
	.functype	consts (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	1007
	i32.add 
	return
	end_function
                                        # -- End function
	.section	.text.big,"",@
	.globl	big                             # -- Begin function big
	.type	big,@function
big:                                    # @big
	.functype	big (i64) -> (i64)
# %bb.0:
	local.get	0
	i64.const	81985529216486895
	i64.add 
	return
	end_function
                                        # -- End function
	.section	.text.trunc,"",@
	.globl	trunc                           # -- Begin function trunc
	.type	trunc,@function
trunc:                                  # @trunc
	.functype	trunc (i64) -> (i32)
# %bb.0:
	local.get	0
	i32.wrap_i64
	return
	end_function
                                        # -- End function
	.section	.text.ext,"",@
	.globl	ext                             # -- Begin function ext
	.type	ext,@function
ext:                                    # @ext
	.functype	ext (i32, i32) -> (i64)
# %bb.0:
	local.get	0
	i64.extend_i32_u
	local.get	1
	i64.extend_i32_s
	i64.add 
	return
	end_function
                                        # -- End function
