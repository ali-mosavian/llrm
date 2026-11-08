use llrm_mir::datalayout::DataLayout;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::combine_compares::visit_icmp;
use crate::testing::{parsed, printed};

const TABLES: &str = "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"
@ns = internal constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 64, i16 100], align 2
@flags = internal constant [8 x i8] [i8 1, i8 0, i8 3, i8 0, i8 0, i8 5, i8 0, i8 0], align 1
";

fn visit(
    c: &mut Combiner,
    inst: llrm_mir::module::InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) { visit_icmp(c, inst) } else { Visit::None }
}

/// `@f(i32 %t)` returning i1 after `visitICmpInst` alone: the output of the
/// function must hold each of `wanted`, as the pinned `opt
/// -passes=instcombine`.
fn check(
    body: &str,
    wanted: &[&str],
) {
    let mut module = parsed(&format!("{TABLES}define i1 @f(i32 %t) {{\nb0:\n{body}  ret i1 %r\n}}\n"));
    let layout = module.datalayout.clone();
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let layout = DataLayout::parse(layout.as_deref().expect("a layout")).expect("parses");
    combine_with(context, &globals, &metadata, &layout, function, visit);
    let out = printed(&module);
    let function = out.split("define i1 @f").nth(1).expect("@f");
    for one in wanted {
        assert!(function.contains(one), "`{one}` missing\n{function}");
    }
    assert!(!function.contains("load"), "the load is left\n{function}");
}

fn ns(
    pred: &str,
    value: i32,
) -> String {
    format!(
        "  %p = getelementptr inbounds [2 x i8], ptr @ns, i32 %t\n  %v = load i16, ptr %p, align 2\n  %r = icmp {pred} i16 %v, {value}\n"
    )
}

/// wasm idioms.c -O2: `ns[t] / 10 > 0` is `ns[t] > 9`, which stayed a load
/// and a compare where the pin has `t - 3 u< 3`.
#[test]
fn a_compare_of_a_table_element_true_for_a_range_is_a_range_test_of_the_index() {
    check(&ns("sgt", 9), &["add i32 %t, -3", "icmp ult i32", ", 3\n"]);
}

#[test]
fn a_compare_true_for_one_element_is_an_equality_of_the_index() {
    check(&ns("eq", 17), &["icmp eq i32 %t, 3"]);
    check(&ns("ne", 17), &["icmp ne i32 %t, 3"]);
    check(&ns("sgt", 0), &["icmp ne i32 %t, 0"]);
}

#[test]
fn a_compare_true_for_two_elements_is_two_equalities_of_the_index() {
    // The pin's `or` fold later turns the pair into `icmp ult i32 %t, 2`.
    check(&ns("ult", 2), &["icmp eq i32 %t, 0", "icmp eq i32 %t, 1", "or i1"]);
}

#[test]
fn a_compare_no_element_meets_is_false() {
    check(&ns("eq", 5), &["ret i1 false"]);
}

#[test]
fn a_scattered_byte_table_is_a_bit_of_a_constant() {
    check(
        "  %p = getelementptr inbounds [8 x i8], ptr @flags, i32 0, i32 %t\n  %v = load i8, ptr %p, align 1\n  %r = icmp ne i8 %v, 0\n",
        &["lshr i32 37, %t"],
    );
}

#[test]
fn a_masked_element_compared_with_zero_is_decided_per_element() {
    check(
        "  %p = getelementptr inbounds [8 x i8], ptr @flags, i32 0, i32 %t\n  %v = load i8, ptr %p, align 1\n  %a = and i8 %v, 4\n  %r = icmp eq i8 %a, 0\n",
        &["icmp ne i32 %t, 5"],
    );
}

#[test]
fn a_gep_without_inbounds_masks_the_wrapped_index() {
    check(
        "  %p = getelementptr [2 x i8], ptr @ns, i32 %t\n  %v = load i16, ptr %p, align 2\n  %r = icmp eq i16 %v, 17\n",
        &["and i32 %t, 2147483647", "icmp eq i32", ", 3\n"],
    );
}
