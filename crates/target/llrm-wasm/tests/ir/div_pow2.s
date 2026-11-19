	.section	.text.a,"",@
	.globl	a                               # -- Begin function a
	.type	a,@function
a:                                      # @a
	.functype	a (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	3
	i32.shr_u
	return
	end_function
                                        # -- End function
	.section	.text.b,"",@
	.globl	b                               # -- Begin function b
	.type	b,@function
b:                                      # @b
	.functype	b (i64) -> (i64)
# %bb.0:
	local.get	0
	i64.const	4
	i64.shr_u
	return
	end_function
                                        # -- End function
	.section	.text.c,"",@
	.globl	c                               # -- Begin function c
	.type	c,@function
c:                                      # @c
	.functype	c (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	252
	i32.and 
	i32.const	2
	i32.shr_u
	return
	end_function
                                        # -- End function
	.section	.text.d,"",@
	.globl	d                               # -- Begin function d
	.type	d,@function
d:                                      # @d
	.functype	d (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	4
	i32.div_s
	return
	end_function
                                        # -- End function
	.section	.text.e,"",@
	.globl	e                               # -- Begin function e
	.type	e,@function
e:                                      # @e
	.functype	e (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.extend16_s
	i32.const	2
	i32.div_s
	return
	end_function
                                        # -- End function
	.section	.text.f,"",@
	.globl	f                               # -- Begin function f
	.type	f,@function
f:                                      # @f
	.functype	f (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	7
	i32.rem_u
	return
	end_function
                                        # -- End function
