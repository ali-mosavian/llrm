	.section	.text.lrint32,"",@
	.globl	lrint32
	.type	lrint32,@function
lrint32:
	.functype	lrint32 (f64) -> (i32)
	local.get	0
	call	lrint
	return
	end_function
	.section	.text.lrint32f,"",@
	.globl	lrint32f
	.type	lrint32f,@function
lrint32f:
	.functype	lrint32f (f32) -> (i32)
	local.get	0
	call	lrintf
	return
	end_function
	.section	.text.llrint64,"",@
	.globl	llrint64
	.type	llrint64,@function
llrint64:
	.functype	llrint64 (f64) -> (i64)
	local.get	0
	call	llrint
	return
	end_function
	.section	.text.llrint64f,"",@
	.globl	llrint64f
	.type	llrint64f,@function
llrint64f:
	.functype	llrint64f (f32) -> (i64)
	local.get	0
	call	llrintf
	return
	end_function
	.section	.text.llrint16,"",@
	.globl	llrint16
	.type	llrint16,@function
llrint16:
	.functype	llrint16 (f64) -> (i32)
	local.get	0
	call	llrint
	return
	end_function
