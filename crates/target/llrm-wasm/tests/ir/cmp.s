	.section	.text.eq,"",@
	.globl	eq
	.type	eq,@function
eq:
	.functype	eq (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.eq
	return
	end_function
	.section	.text.slt,"",@
	.globl	slt
	.type	slt,@function
slt:
	.functype	slt (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.lt_s
	return
	end_function
	.section	.text.ule,"",@
	.globl	ule
	.type	ule,@function
ule:
	.functype	ule (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.le_u
	return
	end_function
	.section	.text.sel,"",@
	.globl	sel
	.type	sel,@function
sel:
	.functype	sel (i32, i32, i32, i32) -> (i32)
	local.get	2
	local.get	3
	local.get	0
	local.get	1
	i32.gt_u
	i32.select
	return
	end_function
	.section	.text.eq64,"",@
	.globl	eq64
	.type	eq64,@function
eq64:
	.functype	eq64 (i64, i64) -> (i32)
	local.get	0
	local.get	1
	i64.ne
	return
	end_function
	.section	.text.isz,"",@
	.globl	isz
	.type	isz,@function
isz:
	.functype	isz (i32) -> (i32)
	local.get	0
	i32.eqz
	return
	end_function
