//! `lower_phis` puts the top-of-block copies of a block's phis in reverse phi
//! order (`PHIElimination::LowerPHINode` lowers the first phi first and inserts
//! its copy after the phis that remain). In order, a gcd loop's divisor copy
//! came before its dividend copy, and the dividend (read after the divisor's
//! register was defined) kept the join of the bottom copy from the pin's: a
//! `mov` stayed beside the `sdiv`.

use llrm_arm64::dag::emit::emit;
use llrm_arm64::dag::function::select_function;
use llrm_arm64::mfunc::{Op, lower_phis};

const IR: &str = "target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n\
define i32 @f(i32 %a0, i32 %b0) {\n\
entry:\n  br label %loop\n\
loop:\n  %b = phi i32 [ %b0, %entry ], [ %r, %loop ]\n  %a = phi i32 [ %a0, %entry ], [ %b, %loop ]\n  %r = srem i32 %a, %b\n  %done = icmp eq i32 %r, 0\n  br i1 %done, label %exit, label %loop\n\
exit:\n  ret i32 %b\n}";

#[test]
fn the_copies_of_a_blocks_phis_stand_in_reverse_phi_order() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let mut machine = emit("f", &select_function(&module, "f").expect("selects"), false).expect("emits");
    let phis: Vec<_> = machine
        .blocks
        .iter()
        .find_map(|block| {
            let phis: Vec<_> = block
                .ops
                .iter()
                .map_while(|op| if let Op::Phi { dst, .. } = op { Some(*dst) } else { None })
                .collect();
            (phis.len() == 2).then_some(phis)
        })
        .expect("the premise: a block with two phis");
    lower_phis(&mut machine).expect("lowers");
    let copies: Vec<_> = machine
        .blocks
        .iter()
        .find_map(|block| {
            let heads: Vec<_> = block
                .ops
                .iter()
                .map_while(|op| if let Op::Copy { dst, .. } = op { Some(*dst) } else { None })
                .collect();
            (heads.len() >= 2 && heads.contains(&phis[0]) && heads.contains(&phis[1])).then_some(heads)
        })
        .expect("the premise: the block's top copies");
    assert_eq!(copies[..2], [phis[1], phis[0]], "the second phi's copy comes first");
}
