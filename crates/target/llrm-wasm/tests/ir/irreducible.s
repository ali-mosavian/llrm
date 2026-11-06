	.section	.text.two_entries,"",@
	.globl	two_entries                     # -- Begin function two_entries
	.type	two_entries,@function
two_entries:                            # @two_entries
	.functype	two_entries (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32
# %bb.0:                                # %entry
	i32.const	5
	local.set	2
	i32.const	0
	local.set	3
	local.get	3
	local.set	4
	local.get	2
	local.set	5
	block   	
	block   	
	local.get	0
	br_if   	0                               # 0: down to label1
# %bb.1:
	i32.const	0
	local.set	6
	br      	1                               # 1: down to label0
.LBB0_2:
	end_block                               # label1:
	i32.const	1
	local.set	6
.LBB0_3:                                # =>This Inner Loop Header: Depth=1
	end_block                               # label0:
	loop    	i32                             # label2:
	block   	
	block   	
	block   	
	block   	
	local.get	6
	br_table 	{0, 1, 1}               # 1: down to label5
.LBB0_4:                                # %a
                                        #   in Loop: Header=BB0_3 Depth=1
	end_block                               # label6:
	local.get	4
	local.set	7
	i32.const	1
	local.set	8
	local.get	7
	local.get	8
	i32.add 
	local.set	9
	local.get	9
	local.get	1
	i32.ge_s
	local.set	10
	local.get	9
	local.set	5
	local.get	9
	local.set	11
	local.get	10
	br_if   	1                               # 1: down to label4
# %bb.5:                                #   in Loop: Header=BB0_3 Depth=1
	i32.const	1
	local.set	6
	br      	3                               # 3: up to label2
.LBB0_6:                                # %b
                                        #   in Loop: Header=BB0_3 Depth=1
	end_block                               # label5:
	local.get	5
	local.set	12
	i32.const	3
	local.set	13
	local.get	12
	local.get	13
	i32.mul 
	local.set	14
	local.get	14
	local.get	1
	i32.lt_s
	local.set	15
	local.get	14
	local.set	4
	local.get	14
	local.set	11
	local.get	15
	br_if   	1                               # 1: down to label3
.LBB0_7:                                # %exit
	end_block                               # label4:
	local.get	11
	local.set	16
	local.get	16
	return
.LBB0_8:                                #   in Loop: Header=BB0_3 Depth=1
	end_block                               # label3:
	i32.const	0
	local.set	6
	br      	0                               # 0: up to label2
.LBB0_9:
	end_loop
	end_function
                                        # -- End function
	.section	.text.nested,"",@
	.globl	nested                          # -- Begin function nested
	.type	nested,@function
nested:                                 # @nested
	.functype	nested (i32, i32) -> (i32)
	.local  	i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32
# %bb.0:                                # %entry
	i32.const	0
	local.set	2
	local.get	2
	local.set	3
.LBB1_1:                                # %outer
                                        # =>This Loop Header: Depth=1
                                        #     Child Loop BB1_4 Depth 2
	loop    	i32                             # label7:
	local.get	3
	local.set	4
	i32.const	5
	local.set	5
	i32.const	0
	local.set	6
	local.get	6
	local.set	7
	local.get	5
	local.set	8
	block   	
	block   	
	local.get	0
	br_if   	0                               # 0: down to label9
# %bb.2:                                #   in Loop: Header=BB1_1 Depth=1
	i32.const	0
	local.set	9
	br      	1                               # 1: down to label8
.LBB1_3:                                #   in Loop: Header=BB1_1 Depth=1
	end_block                               # label9:
	i32.const	1
	local.set	9
.LBB1_4:                                #   Parent Loop BB1_1 Depth=1
                                        # =>  This Inner Loop Header: Depth=2
	end_block                               # label8:
	loop    	i32                             # label10:
	block   	
	block   	
	block   	
	block   	
	local.get	9
	br_table 	{0, 1, 1}               # 1: down to label13
.LBB1_5:                                # %a
                                        #   in Loop: Header=BB1_4 Depth=2
	end_block                               # label14:
	local.get	7
	local.set	10
	local.get	10
	local.get	4
	i32.add 
	local.set	11
	local.get	11
	local.get	1
	i32.ge_s
	local.set	12
	local.get	11
	local.set	8
	local.get	11
	local.set	13
	local.get	12
	br_if   	1                               # 1: down to label12
# %bb.6:                                #   in Loop: Header=BB1_4 Depth=2
	i32.const	1
	local.set	9
	br      	3                               # 3: up to label10
.LBB1_7:                                # %b
                                        #   in Loop: Header=BB1_4 Depth=2
	end_block                               # label13:
	local.get	8
	local.set	14
	i32.const	3
	local.set	15
	local.get	14
	local.get	15
	i32.mul 
	local.set	16
	local.get	16
	local.get	1
	i32.lt_s
	local.set	17
	local.get	16
	local.set	7
	local.get	16
	local.set	13
	local.get	17
	br_if   	1                               # 1: down to label11
.LBB1_8:                                # %latch
                                        #   in Loop: Header=BB1_1 Depth=1
	end_block                               # label12:
	local.get	13
	local.set	18
	i32.const	1
	local.set	19
	local.get	4
	local.get	19
	i32.add 
	local.set	20
	local.get	20
	local.get	18
	i32.le_s
	local.set	21
	local.get	20
	local.set	3
	local.get	21
	br_if   	2                               # 2: up to label7
# %bb.9:                                # %exit
	local.get	20
	return
.LBB1_10:                               #   in Loop: Header=BB1_4 Depth=2
	end_block                               # label11:
	i32.const	0
	local.set	9
	br      	0                               # 0: up to label10
.LBB1_11:
	end_loop
	end_loop
	end_function
                                        # -- End function
