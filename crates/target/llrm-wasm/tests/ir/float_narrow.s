	.section	.text.s16,"",@
	.globl	s16                             # -- Begin function s16
	.type	s16,@function
s16:                                    # @s16
	.functype	s16 (i32) -> (f64)
# %bb.0:
	local.get	0
	i32.extend16_s
	f64.convert_i32_s
	return
	end_function
                                        # -- End function
	.section	.text.u8,"",@
	.globl	u8                              # -- Begin function u8
	.type	u8,@function
u8:                                     # @u8
	.functype	u8 (i32) -> (f32)
# %bb.0:
	local.get	0
	i32.const	255
	i32.and 
	f32.convert_i32_u
	return
	end_function
                                        # -- End function
	.section	.text.s8,"",@
	.globl	s8                              # -- Begin function s8
	.type	s8,@function
s8:                                     # @s8
	.functype	s8 (i32) -> (f64)
# %bb.0:
	local.get	0
	i32.extend8_s
	f64.convert_i32_s
	return
	end_function
                                        # -- End function
	.section	.text.f16,"",@
	.globl	f16                             # -- Begin function f16
	.type	f16,@function
f16:                                    # @f16
	.functype	f16 (f64) -> (i32)
# %bb.0:
	local.get	0
	i32.trunc_sat_f64_s
	return
	end_function
                                        # -- End function
	.section	.text.fu8,"",@
	.globl	fu8                             # -- Begin function fu8
	.type	fu8,@function
fu8:                                    # @fu8
	.functype	fu8 (f32) -> (i32)
# %bb.0:
	local.get	0
	i32.trunc_sat_f32_u
	return
	end_function
                                        # -- End function
