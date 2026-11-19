//! `foldSelectValueEquivalence` against `opt -passes=instcombine` on the same
//! text (pin d1106deb71cc).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// The driver's visit of the selects and compares.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    match c.function.instruction(inst).opcode {
        Opcode::ICmp(_) | Opcode::Select => crate::combine::visit(c, inst),
        _ => Visit::None,
    }
}

/// `@f` after the driver.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

const CTPOP64: &str = "declare i64 @llvm.ctpop.i64(i64)\n";

/// The pin's own example: `x == INT_MAX ? INT_MIN : x + 1` is `x + 1` once the
/// `nsw` that the select guarded is dropped. The select stayed.
#[test]
fn a_select_guarding_an_overflow_is_the_add_without_its_flag() {
    let out = combined(
        "define i32 @f(i32 %x) {\nb0:\n  %c = icmp eq i32 %x, 2147483647\n  %a = add nsw i32 %x, 1\n  %s = select i1 %c, i32 -2147483648, i32 %a\n  ret i32 %s\n}\n",
    );
    assert!(!out.contains("select") && out.contains("add i32 %x, 1") && !out.contains("nsw"), "{out}");
}

/// `x != 0 ? trunc nuw nsw (ctpop x) : 0` is the `trunc` alone: the `trunc`'s
/// flags are dropped (wasm32 `ones64` +6 B, the select left in the loop idiom).
#[test]
fn a_select_guarding_a_flagged_trunc_of_a_count_is_the_trunc_without_flags() {
    for arms in ["i32 0, i32 %t", "i32 %t, i32 0"] {
        let cmp = if arms.starts_with("i32 0") { "eq" } else { "ne" };
        let out = combined(&format!(
            "{CTPOP64}define i32 @f(i64 %x) {{\nb0:\n  %z = icmp {cmp} i64 %x, 0\n  %c = call i64 @llvm.ctpop.i64(i64 %x)\n  %t = trunc nuw nsw i64 %c to i32\n  %r = select i1 %z, {arms}\n  ret i32 %r\n}}\n"
        ));
        assert!(!out.contains("select") && out.contains("trunc i64 %c to i32"), "{cmp}: {out}");
    }
}

/// The equality must give the other arm's value: `x == 5 ? 7 : x + 1` stays.
#[test]
fn a_select_whose_arms_disagree_under_the_equality_stays() {
    let text = "define i32 @f(i32 %x) {\nb0:\n  %c = icmp eq i32 %x, 5\n  %a = add nsw i32 %x, 1\n  %s = select i1 %c, i32 7, i32 %a\n  ret i32 %s\n}\n";
    assert_eq!(combined(text), printed(&parsed(text)));
}
