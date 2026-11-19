	.section	.text.cpy,"",@
	.globl	cpy                             # -- Begin function cpy
	.type	cpy,@function
cpy:                                    # @cpy
	.functype	cpy (i32, i32, i32) -> ()
# %bb.0:
	block   	
	local.get	2
	i32.eqz
	br_if   	0                               # 0: down to label0
# %bb.1:
	local.get	0
	local.get	1
	local.get	2
	memory.copy	0, 0
.LBB0_2:
	end_block                               # label0:
	return
	end_function
                                        # -- End function
	.section	.text.cpyc,"",@
	.globl	cpyc                            # -- Begin function cpyc
	.type	cpyc,@function
cpyc:                                   # @cpyc
	.functype	cpyc (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i64.load	8:p2align=0
	i64.store	8:p2align=0
	local.get	0
	local.get	1
	i64.load	0:p2align=0
	i64.store	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.mov,"",@
	.globl	mov                             # -- Begin function mov
	.type	mov,@function
mov:                                    # @mov
	.functype	mov (i32, i32, i32) -> ()
# %bb.0:
	block   	
	local.get	2
	i32.eqz
	br_if   	0                               # 0: down to label1
# %bb.1:
	local.get	0
	local.get	1
	local.get	2
	memory.copy	0, 0
.LBB2_2:
	end_block                               # label1:
	return
	end_function
                                        # -- End function
	.section	.text.set,"",@
	.globl	set                             # -- Begin function set
	.type	set,@function
set:                                    # @set
	.functype	set (i32, i32, i32) -> ()
# %bb.0:
	block   	
	local.get	2
	i32.eqz
	br_if   	0                               # 0: down to label2
# %bb.1:
	local.get	0
	local.get	1
	local.get	2
	memory.fill	0
.LBB3_2:
	end_block                               # label2:
	return
	end_function
                                        # -- End function
	.section	.text.setc,"",@
	.globl	setc                            # -- Begin function setc
	.type	setc,@function
setc:                                   # @setc
	.functype	setc (i32) -> ()
	.local  	i64
# %bb.0:
	i64.const	0
	local.set	1
	local.get	0
	local.get	1
	i64.store	8:p2align=0
	local.get	0
	local.get	1
	i64.store	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.set16,"",@
	.globl	set16                           # -- Begin function set16
	.type	set16,@function
set16:                                  # @set16
	.functype	set16 (i32) -> ()
	.local  	i32
# %bb.0:
	i32.const	400
	local.set	1
	local.get	0
	i32.const	0
	local.get	1
	memory.fill	0
	return
	end_function
                                        # -- End function
	.section	.text.cpy15,"",@
	.globl	cpy15                           # -- Begin function cpy15
	.type	cpy15,@function
cpy15:                                  # @cpy15
	.functype	cpy15 (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i64.load	7:p2align=0
	i64.store	7:p2align=0
	local.get	0
	local.get	1
	i64.load	0:p2align=0
	i64.store	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.cpy100,"",@
	.globl	cpy100                          # -- Begin function cpy100
	.type	cpy100,@function
cpy100:                                 # @cpy100
	.functype	cpy100 (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i32.const	100
	memory.copy	0, 0
	return
	end_function
                                        # -- End function
	.section	.text.set7,"",@
	.globl	set7                            # -- Begin function set7
	.type	set7,@function
set7:                                   # @set7
	.functype	set7 (i32, i32) -> ()
	.local  	i32
# %bb.0:
	local.get	1
	i32.const	255
	i32.and 
	i32.const	16843009
	i32.mul 
	local.set	2
	local.get	0
	local.get	2
	i32.store	3:p2align=0
	local.get	0
	local.get	2
	i32.store	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.set3,"",@
	.globl	set3                            # -- Begin function set3
	.type	set3,@function
set3:                                   # @set3
	.functype	set3 (i32) -> ()
	.local  	i32
# %bb.0:
	i32.const	-1
	local.set	1
	local.get	0
	local.get	1
	i32.store8	2
	local.get	0
	local.get	1
	i32.store16	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.mov8,"",@
	.globl	mov8                            # -- Begin function mov8
	.type	mov8,@function
mov8:                                   # @mov8
	.functype	mov8 (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i64.load	0:p2align=0
	i64.store	0:p2align=0
	return
	end_function
                                        # -- End function
	.section	.text.mem_phi,"",@
	.globl	mem_phi                         # -- Begin function mem_phi
	.type	mem_phi,@function
mem_phi:                                # @mem_phi
	.functype	mem_phi (i32, i32, i32) -> (i32)
	.local  	i32
# %bb.0:                                # %entry
	block   	
	block   	
	local.get	2
	i32.const	5
	i32.eq  
	br_if   	0                               # 0: down to label4
# %bb.1:                                # %a
	block   	
	local.get	2
	i32.eqz
	br_if   	0                               # 0: down to label5
# %bb.2:                                # %a
	local.get	0
	local.get	1
	local.get	2
	memory.copy	0, 0
.LBB11_3:                               # %a
	end_block                               # label5:
	i32.const	1
	local.set	3
	br      	1                               # 1: down to label3
.LBB11_4:                               # %b
	end_block                               # label4:
	i32.const	2
	local.set	3
.LBB11_5:                               # %j
	end_block                               # label3:
	local.get	3
	return
	end_function
                                        # -- End function
	.section	.text.mem_loop,"",@
	.globl	mem_loop                        # -- Begin function mem_loop
	.type	mem_loop,@function
mem_loop:                               # @mem_loop
	.functype	mem_loop (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32
# %bb.0:                                # %entry
	i32.const	0
	local.set	2
.LBB12_1:                               # %l
                                        # =>This Inner Loop Header: Depth=1
	loop    	                                # label6:
	local.get	2
	local.set	3
	i32.const	0
	local.set	4
	block   	
	local.get	1
	i32.eqz
	br_if   	0                               # 0: down to label7
# %bb.2:                                # %l
                                        #   in Loop: Header=BB12_1 Depth=1
	local.get	0
	local.get	4
	local.get	1
	memory.fill	0
.LBB12_3:                               # %l
                                        #   in Loop: Header=BB12_1 Depth=1
	end_block                               # label7:
	local.get	3
	i32.const	1
	i32.add 
	local.set	5
	local.get	5
	local.get	1
	i32.lt_u
	local.set	6
	local.get	5
	local.set	2
	local.get	6
	br_if   	0                               # 0: up to label6
# %bb.4:                                # %out
	end_loop
	local.get	5
	return
	end_function
                                        # -- End function
	.section	.text.mem_zero,"",@
	.globl	mem_zero                        # -- Begin function mem_zero
	.type	mem_zero,@function
mem_zero:                               # @mem_zero
	.functype	mem_zero (i32, i32) -> ()
	.local  	i32
# %bb.0:
	i32.const	200
	local.set	2
	local.get	0
	i32.const	1
	local.get	2
	memory.fill	0
	return
	end_function
                                        # -- End function
