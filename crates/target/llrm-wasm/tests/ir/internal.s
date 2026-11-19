	.section	.text.helper,"",@
	.type	helper,@function                # -- Begin function helper
helper:                                 # @helper
	.functype	helper (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	3
	i32.mul 
	return
	end_function
                                        # -- End function
	.section	.text.entry,"",@
	.globl	entry                           # -- Begin function entry
	.type	entry,@function
entry:                                  # @entry
	.functype	entry (i32) -> (i32)
# %bb.0:
	local.get	0
	call	helper
	return
	end_function
                                        # -- End function
