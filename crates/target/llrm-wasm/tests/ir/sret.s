	.section	.text.make,"",@
	.globl	make                            # -- Begin function make
	.type	make,@function
make:                                   # @make
	.functype	make (i32, i32) -> ()
# %bb.0:
	local.get	0
	local.get	1
	i32.store	0
	local.get	0
	i32.const	7
	i32.store	4
	local.get	0
	i32.const	3
	i32.store16	8
	return
	end_function
                                        # -- End function
	.section	.text.use,"",@
	.globl	use                             # -- Begin function use
	.type	use,@function
use:                                    # @use
	.functype	use (i32) -> (i32)
	.local  	i32, i32
# %bb.0:
	global.get	__stack_pointer
	i32.const	16
	i32.sub 
	local.set	1
	local.get	1
	global.set	__stack_pointer
	local.get	1
	i32.const	4
	i32.add 
	local.get	0
	call	make
	local.get	1
	i32.load	8
	local.set	2
	local.get	1
	i32.const	16
	i32.add 
	global.set	__stack_pointer
	local.get	2
	return
	end_function
                                        # -- End function
