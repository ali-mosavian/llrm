	.section	.text.add,"",@
	.globl	add
	.type	add,@function
add:
	.functype	add (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.add
	return
	end_function
	.section	.text.sub,"",@
	.globl	sub
	.type	sub,@function
sub:
	.functype	sub (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.sub
	return
	end_function
	.section	.text.mul,"",@
	.globl	mul
	.type	mul,@function
mul:
	.functype	mul (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.mul
	return
	end_function
	.section	.text.sdiv,"",@
	.globl	sdiv
	.type	sdiv,@function
sdiv:
	.functype	sdiv (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.div_s
	return
	end_function
	.section	.text.udiv,"",@
	.globl	udiv
	.type	udiv,@function
udiv:
	.functype	udiv (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.div_u
	return
	end_function
	.section	.text.srem,"",@
	.globl	srem
	.type	srem,@function
srem:
	.functype	srem (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.rem_s
	return
	end_function
	.section	.text.urem,"",@
	.globl	urem
	.type	urem,@function
urem:
	.functype	urem (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.rem_u
	return
	end_function
	.section	.text.add64,"",@
	.globl	add64
	.type	add64,@function
add64:
	.functype	add64 (i64, i64) -> (i64)
	local.get	0
	local.get	1
	i64.add
	return
	end_function
	.section	.text.mul64,"",@
	.globl	mul64
	.type	mul64,@function
mul64:
	.functype	mul64 (i64, i64) -> (i64)
	local.get	0
	local.get	1
	i64.mul
	return
	end_function
	.section	.text.consts,"",@
	.globl	consts
	.type	consts,@function
consts:
	.functype	consts (i32) -> (i32)
	local.get	0
	i32.const	1007
	i32.add
	return
	end_function
	.section	.text.big,"",@
	.globl	big
	.type	big,@function
big:
	.functype	big (i64) -> (i64)
	local.get	0
	i64.const	81985529216486895
	i64.add
	return
	end_function
	.section	.text.trunc,"",@
	.globl	trunc
	.type	trunc,@function
trunc:
	.functype	trunc (i64) -> (i32)
	local.get	0
	i32.wrap_i64
	return
	end_function
	.section	.text.ext,"",@
	.globl	ext
	.type	ext,@function
ext:
	.functype	ext (i32, i32) -> (i64)
	local.get	0
	i64.extend_i32_u
	local.get	1
	i64.extend_i32_s
	i64.add
	return
	end_function
