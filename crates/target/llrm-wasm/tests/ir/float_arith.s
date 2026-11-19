	.section	.text.a,"",@
	.globl	a                               # -- Begin function a
	.type	a,@function
a:                                      # @a
	.functype	a (f64, f64) -> (f64)
# %bb.0:
	local.get	0
	local.get	1
	f64.add 
	f64.const	-0x1.8p0
	f64.add 
	local.get	1
	f64.mul 
	f64.const	-0x1.388p11
	f64.div 
	return
	end_function
                                        # -- End function
	.section	.text.b,"",@
	.globl	b                               # -- Begin function b
	.type	b,@function
b:                                      # @b
	.functype	b (f32, f32) -> (f32)
# %bb.0:
	local.get	0
	local.get	1
	f32.add 
	f32.const	0x1.99999ap-4
	f32.mul 
	local.get	1
	f32.div 
	return
	end_function
                                        # -- End function
	.section	.text.c,"",@
	.globl	c                               # -- Begin function c
	.type	c,@function
c:                                      # @c
	.functype	c (f64, f64) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	f64.lt  
	return
	end_function
                                        # -- End function
	.section	.text.d,"",@
	.globl	d                               # -- Begin function d
	.type	d,@function
d:                                      # @d
	.functype	d (i32, i32) -> (f64)
	.local  	f64
# %bb.0:
	local.get	0
	local.get	1
	i32.const	3
	i32.shl 
	i32.add 
	f64.load	0
	local.set	2
	local.get	0
	local.get	2
	f64.store	0
	local.get	2
	local.get	0
	f32.load	0
	f64.promote_f32
	f64.add 
	return
	end_function
                                        # -- End function
	.section	.text.e,"",@
	.globl	e                               # -- Begin function e
	.type	e,@function
e:                                      # @e
	.functype	e (f64) -> (f32)
# %bb.0:
	local.get	0
	f32.demote_f64
	return
	end_function
                                        # -- End function
