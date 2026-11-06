	.section	.text.shifts,"",@
	.globl	shifts
	.type	shifts,@function
shifts:
	.functype	shifts (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.shl
	local.get	1
	i32.shr_u
	i32.const	3
	i32.shr_s
	return
	end_function
	.section	.text.shl,"",@
	.globl	shl
	.type	shl,@function
shl:
	.functype	shl (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.shl
	return
	end_function
	.section	.text.shr_s,"",@
	.globl	shr_s
	.type	shr_s,@function
shr_s:
	.functype	shr_s (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.shr_s
	return
	end_function
	.section	.text.shr_u,"",@
	.globl	shr_u
	.type	shr_u,@function
shr_u:
	.functype	shr_u (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.shr_u
	return
	end_function
	.section	.text.shl64,"",@
	.globl	shl64
	.type	shl64,@function
shl64:
	.functype	shl64 (i64, i64) -> (i64)
	local.get	0
	local.get	1
	i64.shl
	return
	end_function
	.section	.text.shr_s64,"",@
	.globl	shr_s64
	.type	shr_s64,@function
shr_s64:
	.functype	shr_s64 (i64) -> (i64)
	local.get	0
	i64.const	7
	i64.shr_s
	return
	end_function
