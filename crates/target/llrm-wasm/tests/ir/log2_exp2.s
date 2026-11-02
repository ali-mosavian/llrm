	.section	.text.log2d,"",@
	.globl	log2d
	.type	log2d,@function
log2d:
	.functype	log2d (f64) -> (f64)
	local.get	0
	call	log2
	return
	end_function
	.section	.text.log2f_,"",@
	.globl	log2f_
	.type	log2f_,@function
log2f_:
	.functype	log2f_ (f32) -> (f32)
	local.get	0
	call	log2f
	return
	end_function
	.section	.text.exp2d,"",@
	.globl	exp2d
	.type	exp2d,@function
exp2d:
	.functype	exp2d (f64) -> (f64)
	local.get	0
	call	exp2
	return
	end_function
	.section	.text.exp2f_,"",@
	.globl	exp2f_
	.type	exp2f_,@function
exp2f_:
	.functype	exp2f_ (f32) -> (f32)
	local.get	0
	call	exp2f
	return
	end_function
