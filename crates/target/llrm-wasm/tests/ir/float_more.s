	.section	.text.conv1,"",@
	.globl	conv1                           # -- Begin function conv1
	.type	conv1,@function
conv1:                                  # @conv1
	.functype	conv1 (i32, i64, i32) -> (f64)
	.local  	f64, f64
# %bb.0:
	local.get	0
	f64.convert_i32_s
	local.set	3
	local.get	1
	f64.convert_i64_s
	local.set	4
	local.get	2
	f64.convert_i32_u
	local.get	3
	local.get	4
	f64.add 
	f64.add 
	return
	end_function
                                        # -- End function
	.section	.text.conv2,"",@
	.globl	conv2                           # -- Begin function conv2
	.type	conv2,@function
conv2:                                  # @conv2
	.functype	conv2 (f64, f32) -> (i32)
# %bb.0:
	local.get	0
	i32.trunc_sat_f64_s
	local.get	1
	i32.trunc_sat_f32_u
	i32.add 
	return
	end_function
                                        # -- End function
	.section	.text.conv3,"",@
	.globl	conv3                           # -- Begin function conv3
	.type	conv3,@function
conv3:                                  # @conv3
	.functype	conv3 (f64) -> (i64)
# %bb.0:
	local.get	0
	i64.trunc_sat_f64_s
	local.get	0
	i64.trunc_sat_f64_u
	i64.add 
	return
	end_function
                                        # -- End function
	.section	.text.conv4,"",@
	.globl	conv4                           # -- Begin function conv4
	.type	conv4,@function
conv4:                                  # @conv4
	.functype	conv4 (i32, i64) -> (f32)
# %bb.0:
	local.get	0
	f32.convert_i32_s
	local.get	1
	f32.convert_i64_u
	f32.add 
	return
	end_function
                                        # -- End function
	.section	.text.bits,"",@
	.globl	bits                            # -- Begin function bits
	.type	bits,@function
bits:                                   # @bits
	.functype	bits (f32, i32) -> (i32)
# %bb.0:
	local.get	0
	i32.reinterpret_f32
	local.get	1
	f32.reinterpret_i32
	i32.trunc_sat_f32_s
	i32.add 
	return
	end_function
                                        # -- End function
	.section	.text.cmps,"",@
	.globl	cmps                            # -- Begin function cmps
	.type	cmps,@function
cmps:                                   # @cmps
	.functype	cmps (f64, f64) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32
# %bb.0:
	local.get	0
	local.get	1
	f64.eq  
	local.set	2
	local.get	0
	local.get	1
	f64.lt  
	local.set	3
	local.get	0
	local.get	1
	f64.gt  
	local.set	4
	local.get	4
	local.get	3
	i32.or  
	local.set	5
	local.get	0
	local.get	1
	f64.ge  
	local.set	6
	local.get	0
	local.get	1
	f64.le  
	local.set	7
	local.get	0
	local.get	1
	f64.ne  
	local.set	8
	local.get	1
	local.get	1
	f64.eq  
	local.get	0
	local.get	0
	f64.eq  
	i32.and 
	local.set	9
	local.get	1
	local.get	1
	f64.ne  
	local.get	0
	local.get	0
	f64.ne  
	i32.or  
	local.set	10
	i32.const	1
	local.set	11
	local.get	6
	local.get	11
	i32.xor 
	local.set	12
	local.get	5
	local.get	11
	i32.xor 
	local.get	12
	local.get	10
	local.get	9
	local.get	8
	local.get	7
	local.get	6
	local.get	4
	local.get	2
	local.get	5
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	i32.add 
	return
	end_function
                                        # -- End function
	.section	.text.sel,"",@
	.globl	sel                             # -- Begin function sel
	.type	sel,@function
sel:                                    # @sel
	.functype	sel (f64, f64, i32) -> (f64)
	.local  	f64
# %bb.0:
	local.get	0
	local.get	1
	local.get	0
	local.get	1
	f64.lt  
	f64.select
	local.set	3
	f64.const	0x1p1
	local.get	3
	local.get	2
	f64.select
	return
	end_function
                                        # -- End function
	.section	.text.branch,"",@
	.globl	branch                          # -- Begin function branch
	.type	branch,@function
branch:                                 # @branch
	.functype	branch (f64, f64) -> (f64)
# %bb.0:                                # %entry
	block   	
	local.get	0
	local.get	1
	f64.gt  
	i32.eqz
	br_if   	0                               # 0: down to label0
# %bb.1:                                # %a
	local.get	0
	local.get	1
	f64.sub 
	return
.LBB7_2:                                # %b
	end_block                               # label0:
	local.get	1
	local.get	0
	f64.sub 
	return
	end_function
                                        # -- End function
	.section	.text.intrinf,"",@
	.globl	intrinf                         # -- Begin function intrinf
	.type	intrinf,@function
intrinf:                                # @intrinf
	.functype	intrinf (f32) -> (f32)
# %bb.0:
	local.get	0
	f32.abs 
	f32.sqrt
	return
	end_function
                                        # -- End function
	.section	.text.libcall,"",@
	.globl	libcall                         # -- Begin function libcall
	.type	libcall,@function
libcall:                                # @libcall
	.functype	libcall (f64) -> (f64)
# %bb.0:
	local.get	0
	call	sin
	local.get	0
	call	fmod
	return
	end_function
                                        # -- End function
	.section	.text.callf,"",@
	.globl	callf                           # -- Begin function callf
	.type	callf,@function
callf:                                  # @callf
	.functype	callf (f32, f64) -> (f32)
# %bb.0:
	local.get	0
	local.get	1
	call	takef
	return
	end_function
                                        # -- End function
	.section	.text.consts,"",@
	.globl	consts                          # -- Begin function consts
	.type	consts,@function
consts:                                 # @consts
	.functype	consts () -> (f64)
# %bb.0:
	f64.const	infinity
	return
	end_function
                                        # -- End function
