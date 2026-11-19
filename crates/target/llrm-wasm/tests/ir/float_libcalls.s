	.section	.text.libcall,"",@
	.globl	libcall                         # -- Begin function libcall
	.type	libcall,@function
libcall:                                # @libcall
	.functype	libcall (f64, f64) -> (f64)
# %bb.0:
	local.get	0
	local.get	1
	call	fmod
	return
	end_function
                                        # -- End function
	.section	.text.sc,"",@
	.globl	sc                              # -- Begin function sc
	.type	sc,@function
sc:                                     # @sc
	.functype	sc (f64) -> (f64)
	.local  	f64, f64
# %bb.0:
	local.get	0
	call	sin
	local.set	1
	local.get	0
	call	cos
	local.set	2
	local.get	0
	call	atan
	local.get	1
	local.get	2
	f64.add 
	f64.add 
	return
	end_function
                                        # -- End function
	.section	.text.sf,"",@
	.globl	sf                              # -- Begin function sf
	.type	sf,@function
sf:                                     # @sf
	.functype	sf (f32) -> (f32)
# %bb.0:
	local.get	0
	call	sinf
	return
	end_function
                                        # -- End function
	.section	.text.lr32,"",@
	.globl	lr32                            # -- Begin function lr32
	.type	lr32,@function
lr32:                                   # @lr32
	.functype	lr32 (f64) -> (i32)
# %bb.0:
	local.get	0
	call	lrint
	return
	end_function
                                        # -- End function
	.section	.text.lr16,"",@
	.globl	lr16                            # -- Begin function lr16
	.type	lr16,@function
lr16:                                   # @lr16
	.functype	lr16 (f64) -> (i32)
# %bb.0:
	local.get	0
	call	lrint
	return
	end_function
                                        # -- End function
	.section	.text.lrf,"",@
	.globl	lrf                             # -- Begin function lrf
	.type	lrf,@function
lrf:                                    # @lrf
	.functype	lrf (f32) -> (i32)
# %bb.0:
	local.get	0
	call	lrintf
	return
	end_function
                                        # -- End function
	.section	.text.rn,"",@
	.globl	rn                              # -- Begin function rn
	.type	rn,@function
rn:                                     # @rn
	.functype	rn (f64) -> (f64)
# %bb.0:
	local.get	0
	f64.nearest
	return
	end_function
                                        # -- End function
	.section	.text.ab,"",@
	.globl	ab                              # -- Begin function ab
	.type	ab,@function
ab:                                     # @ab
	.functype	ab (f64) -> (f64)
# %bb.0:
	local.get	0
	f64.abs 
	return
	end_function
                                        # -- End function
