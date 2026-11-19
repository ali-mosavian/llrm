	.section	.text.callv,"",@
	.globl	callv
	.type	callv,@function
callv:
	.functype	callv () -> ()
	call	v0
	return
	end_function
	.section	.text.calli,"",@
	.globl	calli
	.type	calli,@function
calli:
	.functype	calli () -> (i32)
	call	i0
	return
	end_function
	.section	.text.call2,"",@
	.globl	call2
	.type	call2,@function
call2:
	.functype	call2 (i32, i32) -> (i32)
	local.get	1
	local.get	0
	call	i2
	return
	end_function
	.section	.text.call64,"",@
	.globl	call64
	.type	call64,@function
call64:
	.functype	call64 (i64) -> (i64)
	local.get	0
	call	l1
	return
	end_function
	.section	.text.twice,"",@
	.globl	twice
	.type	twice,@function
twice:
	.functype	twice (i32) -> (i32)
	local.get	0
	i32.const	1
	call	i2
	local.get	0
	call	i2
	return
	end_function
	.section	.text.callp,"",@
	.globl	callp
	.type	callp,@function
callp:
	.functype	callp (i32) -> ()
	local.get	0
	i32.const	42
	call	vp
	return
	end_function
	.section	.text.tail,"",@
	.globl	tail
	.type	tail,@function
tail:
	.functype	tail (i32) -> (i32)
	local.get	0
	local.get	0
	call	i2
	i32.const	1
	i32.add
	return
	end_function
