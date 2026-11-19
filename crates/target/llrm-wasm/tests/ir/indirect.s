	.section	.text.callit,"",@
	.globl	callit                          # -- Begin function callit
	.type	callit,@function
callit:                                 # @callit
	.functype	callit (i32, f64) -> (i64)
# %bb.0:
	local.get	1
	i32.const	5
	local.get	0
	call_indirect	 __indirect_function_table, (f64, i32) -> (i64)
	return
	end_function
                                        # -- End function
	.section	.text.v,"",@
	.globl	v                               # -- Begin function v
	.type	v,@function
v:                                      # @v
	.functype	v (i32) -> ()
# %bb.0:
	local.get	0
	call_indirect	 __indirect_function_table, () -> ()
	return
	end_function
                                        # -- End function
	.section	.text.addr_call,"",@
	.globl	addr_call                       # -- Begin function addr_call
	.type	addr_call,@function
addr_call:                              # @addr_call
	.functype	addr_call (i32) -> (i32)
# %bb.0:
	local.get	0
	call	self
	return
	end_function
                                        # -- End function
	.section	.text.self,"",@
	.globl	self                            # -- Begin function self
	.type	self,@function
self:                                   # @self
	.functype	self (i32) -> (i32)
# %bb.0:
	local.get	0
	return
	end_function
                                        # -- End function
	.section	.text.addr,"",@
	.globl	addr                            # -- Begin function addr
	.type	addr,@function
addr:                                   # @addr
	.functype	addr () -> (i32)
# %bb.0:
	i32.const	self
	return
	end_function
                                        # -- End function
	.section	.text.via_table,"",@
	.globl	via_table                       # -- Begin function via_table
	.type	via_table,@function
via_table:                              # @via_table
	.functype	via_table (i32) -> (i32)
# %bb.0:
	local.get	0
	call	self
	call	addr
	call_indirect	 __indirect_function_table, (i32) -> (i32)
	return
	end_function
                                        # -- End function
