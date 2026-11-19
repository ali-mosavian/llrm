use llrm_mir::interpret::{Val, run};

use super::fold_function;
use crate::testing::{parsed, printed};

/// `word_compares`' `orders(a, b)`: eight selects on the unsigned and signed compares of one
/// pair, summed.
const ORDERS: &str = "define i32 @f(i32 %a, i32 %b, i32 %h) {
entry:
  %lt = icmp ult i32 %a, %b
  %gt = icmp ugt i32 %a, %b
  %slt = icmp slt i32 %a, %b
  %sgt = icmp sgt i32 %a, %b
  %t1 = zext i1 %lt to i32
  %t2 = select i1 %gt, i32 0, i32 2
  %t3 = select i1 %gt, i32 4, i32 0
  %t4 = select i1 %lt, i32 0, i32 8
  %t5 = select i1 %slt, i32 16, i32 0
  %t6 = select i1 %sgt, i32 0, i32 32
  %t7 = select i1 %sgt, i32 64, i32 0
  %t8 = select i1 %slt, i32 0, i32 128
  %s1 = add i32 %h, %t1
  %s2 = add i32 %s1, %t2
  %s3 = add i32 %s2, %t3
  %s4 = add i32 %s3, %t4
  %s5 = add i32 %s4, %t5
  %s6 = add i32 %s5, %t6
  %s7 = add i32 %s6, %t7
  %s8 = add i32 %s7, %t8
  ret i32 %s8
}
";

fn value(
    module: &llrm_mir::module::Module,
    a: u128,
    b: u128,
) -> Val {
    let arguments = vec![
        Val::Int { bits: a, width: 32 },
        Val::Int { bits: b, width: 32 },
        Val::Int { bits: 1000, width: 32 },
    ];
    run(module, "f", arguments, 1000).unwrap()
}

/// Eight selects ran as eight; the fold leaves two chains of selects and an add, and every order
/// of the pair gives the sum it did.
#[test]
fn a_sum_of_selects_on_one_pair_is_a_function_of_its_order() {
    let before = parsed(ORDERS);
    let mut after = before.clone();
    let llrm_mir::module::GlobalKind::Function(function) = &mut after.globals[0].kind else { panic!("a function") };
    assert_eq!(fold_function(&after.context, function), 1);
    let text = printed(&after);
    assert!(text.matches("select").count() <= 4, "{text}");
    let samples = [0u128, 1, 2, 0x7fff, 0x8000, 0xffff, 0x10000, 0x7fff_ffff, 0x8000_0000, 0x8000_0001, 0xffff_fffe, 0xffff_ffff];
    for &a in &samples {
        for &b in &samples {
            assert_eq!(value(&after, a, b), value(&before, a, b), "a={a:#x} b={b:#x}\n{text}");
        }
    }
}

/// The three-way compare, `(a>b) - (a<b)`, is two terms: left as it is.
#[test]
fn a_three_way_compare_is_left_alone() {
    let text = "define i32 @f(i32 %a, i32 %b) {
entry:
  %lt = icmp ult i32 %a, %b
  %gt = icmp ugt i32 %a, %b
  %x = zext i1 %gt to i32
  %y = zext i1 %lt to i32
  %r = sub i32 %x, %y
  ret i32 %r
}
";
    let mut module = parsed(text);
    let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[0].kind else { panic!("a function") };
    assert_eq!(fold_function(&module.context, function), 0);
}
