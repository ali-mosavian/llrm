	.section	.text.l8s,"",@
	.globl	l8s
	.type	l8s,@function
l8s:
	.functype	l8s (i32) -> (i32)
	local.get	0
	i32.load8_s	0
	return
	end_function
	.section	.text.l8u,"",@
	.globl	l8u
	.type	l8u,@function
l8u:
	.functype	l8u (i32) -> (i32)
	local.get	0
	i32.load8_u	0
	return
	end_function
	.section	.text.l16s,"",@
	.globl	l16s
	.type	l16s,@function
l16s:
	.functype	l16s (i32) -> (i32)
	local.get	0
	i32.load16_s	0
	return
	end_function
	.section	.text.l16u,"",@
	.globl	l16u
	.type	l16u,@function
l16u:
	.functype	l16u (i32) -> (i32)
	local.get	0
	i32.load16_u	0
	return
	end_function
	.section	.text.l64,"",@
	.globl	l64
	.type	l64,@function
l64:
	.functype	l64 (i32) -> (i64)
	local.get	0
	i64.load	0
	return
	end_function
	.section	.text.l32s64,"",@
	.globl	l32s64
	.type	l32s64,@function
l32s64:
	.functype	l32s64 (i32) -> (i64)
	local.get	0
	i64.load32_s	0
	return
	end_function
	.section	.text.s8,"",@
	.globl	s8
	.type	s8,@function
s8:
	.functype	s8 (i32, i32) -> ()
	local.get	0
	local.get	1
	i32.store8	0
	return
	end_function
	.section	.text.s16,"",@
	.globl	s16
	.type	s16,@function
s16:
	.functype	s16 (i32, i32) -> ()
	local.get	0
	local.get	1
	i32.store16	0
	return
	end_function
	.section	.text.s64,"",@
	.globl	s64
	.type	s64,@function
s64:
	.functype	s64 (i32, i64) -> ()
	local.get	0
	local.get	1
	i64.store	0
	return
	end_function
	.section	.text.off,"",@
	.globl	off
	.type	off,@function
off:
	.functype	off (i32) -> (i32)
	local.get	0
	i32.load	12
	return
	end_function
	.section	.text.idx,"",@
	.globl	idx
	.type	idx,@function
idx:
	.functype	idx (i32, i32) -> (i32)
	local.get	0
	local.get	1
	i32.const	2
	i32.shl
	i32.add
	i32.load	0
	return
	end_function
	.section	.text.unaligned,"",@
	.globl	unaligned
	.type	unaligned,@function
unaligned:
	.functype	unaligned (i32) -> (i32)
	local.get	0
	i32.load	0:p2align=0
	return
	end_function
	.section	.text.volat,"",@
	.globl	volat
	.type	volat,@function
volat:
	.functype	volat (i32) -> (i32)
	local.get	0
	i32.load	0
	return
	end_function
