	.section	.text.f,"",@
	.weak	f                               # -- Begin function f
	.type	f,@function
f:                                      # @f
	.functype	f () -> ()
# %bb.0:
	return
	end_function
                                        # -- End function
	.section	.text.g,"",@
	.weak	g                               # -- Begin function g
	.type	g,@function
g:                                      # @g
	.functype	g () -> ()
# %bb.0:
	return
	end_function
                                        # -- End function
	.section	.text.h,"",@
	.type	h,@function                     # -- Begin function h
h:                                      # @h
	.functype	h () -> ()
# %bb.0:
	return
	end_function
                                        # -- End function
	.section	.text.user,"",@
	.globl	user                            # -- Begin function user
	.type	user,@function
user:                                   # @user
	.functype	user () -> ()
# %bb.0:
	call	f
	call	g
	call	h
	return
	end_function
                                        # -- End function
