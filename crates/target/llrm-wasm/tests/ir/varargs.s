	.section	.text.none,"",@
	.globl	none                            # -- Begin function none
	.type	none,@function
none:                                   # @none
	.functype	none (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	0
	call	printf
	return
	end_function
                                        # -- End function
	.section	.text.one,"",@
	.globl	one                             # -- Begin function one
	.type	one,@function
one:                                    # @one
	.functype	one (i32, i32) -> (i32)
	.local  	i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	2
	local.get	2
	global.set	__stack_pointer
	local.get	2
	local.get	1
	i32.store	0
	local.get	0
	local.get	2
	call	printf
	local.set	3
	local.get	2
	i32.const	16
	i32.add 
	global.set	__stack_pointer
	local.get	3
	return
	end_function
                                        # -- End function
	.section	.text.through_a_pointer,"",@
	.globl	through_a_pointer               # -- Begin function through_a_pointer
	.type	through_a_pointer,@function
through_a_pointer:                      # @through_a_pointer
	.functype	through_a_pointer (i32, i32, i32) -> (i32)
	.local  	i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	3
	local.get	3
	global.set	__stack_pointer
	local.get	3
	local.get	2
	i32.store	0
	local.get	1
	local.get	3
	local.get	0
	call_indirect	 __indirect_function_table, (i32, i32) -> (i32)
	local.set	4
	local.get	3
	i32.const	16
	i32.add 
	global.set	__stack_pointer
	local.get	4
	return
	end_function
                                        # -- End function
