	.section	.text.__original_main,"",@
	.hidden	__original_main
	.globl	__original_main
	.type	__original_main,@function
__original_main:
	.functype	__original_main () -> (i32)
	.local  	i32
	i32.const	7
	call	.Lg_bitcast
	local.set	0
	local.get	0
	i32.const	1
	call	h
	local.get	0
	call	.Lh_bitcast
	drop
	local.get	0
	return
	end_function
	.section	.text..Lg_bitcast,"",@
	.type	.Lg_bitcast,@function
.Lg_bitcast:
	.functype	.Lg_bitcast (i32) -> (i32)
	call	g
	return
	end_function
	.section	.text..Lh_bitcast,"",@
	.type	.Lh_bitcast,@function
.Lh_bitcast:
	.functype	.Lh_bitcast (i32) -> (i64)
	local.get	0
	i32.const	0
	call	h
	i64.const	0
	return
	end_function
	.section	.text.main,"",@
	.hidden	main
	.globl	main
	.type	main,@function
main:
	.functype	main (i32, i32) -> (i32)
	call	__original_main
	return
	end_function
