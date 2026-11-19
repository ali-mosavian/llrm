	.section	.text.ret_undef,"",@
	.globl	ret_undef
	.type	ret_undef,@function
ret_undef:
	.functype	ret_undef () -> (i32)
	i32.const	0
	return
	end_function
	.section	.text.ret_undef_f64,"",@
	.globl	ret_undef_f64
	.type	ret_undef_f64,@function
ret_undef_f64:
	.functype	ret_undef_f64 () -> (f64)
	f64.const	0x0p0
	return
	end_function
	.section	.text.arg_undef,"",@
	.globl	arg_undef
	.type	arg_undef,@function
arg_undef:
	.functype	arg_undef (i32) -> (i32)
	local.get	0
	i32.const	0
	call	use
	return
	end_function
