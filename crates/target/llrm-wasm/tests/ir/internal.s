	.file	"internal.ll"
	.functype	helper (i32) -> (i32)
	.functype	entry (i32) -> (i32)
	.section	.text.helper,"",@
	.type	helper,@function                # -- Begin function helper
helper:                                 # @helper
	.functype	helper (i32) -> (i32)
# %bb.0:
	local.get	0
	i32.const	3
	i32.mul 
	return
	end_function
                                        # -- End function
	.section	.text.entry,"",@
	.globl	entry                           # -- Begin function entry
	.type	entry,@function
entry:                                  # @entry
	.functype	entry (i32) -> (i32)
# %bb.0:
	local.get	0
	call	helper
	return
	end_function
                                        # -- End function
	.section	.custom_section.target_features,"",@
	.int8	8
	.int8	43
	.int8	11
	.ascii	"bulk-memory"
	.int8	43
	.int8	15
	.ascii	"bulk-memory-opt"
	.int8	43
	.int8	22
	.ascii	"call-indirect-overlong"
	.int8	43
	.int8	10
	.ascii	"multivalue"
	.int8	43
	.int8	15
	.ascii	"mutable-globals"
	.int8	43
	.int8	19
	.ascii	"nontrapping-fptoint"
	.int8	43
	.int8	15
	.ascii	"reference-types"
	.int8	43
	.int8	8
	.ascii	"sign-ext"
	.section	.text.entry,"",@
