	.section	.text.finish,"",@
	.globl	finish                          # -- Begin function finish
	.type	finish,@function
finish:                                 # @finish
	.functype	finish () -> ()
# %bb.0:
	block   	
	call	inited
	i32.eqz
	br_if   	0                               # 0: down to label0
# %bb.1:                                # %then
	i32.const	4
	call	act
.LBB0_2:                                # %done
	end_block                               # label0:
	return
	end_function
                                        # -- End function
	.section	.text.widen,"",@
	.globl	widen                           # -- Begin function widen
	.type	widen,@function
widen:                                  # @widen
	.functype	widen (i32, i32) -> (i32)
# %bb.0:
	local.get	0
	local.get	1
	i32.add 
	call	level
	i32.add 
	return
	end_function
                                        # -- End function
