//! The cast of a min or max select moves outside it, as `opt` does.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared, visit as visit_all};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(text: &str) -> String {
    combined_by(text, visit)
}

fn combined_by(
    text: &str,
    by: fn(&mut Combiner, InstId) -> Visit,
) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, by, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// `x > 255 ? -1 : (i8) x` stayed a compare and a select over the byte
/// (wasm32 jpeg: the YCC clamp, +79 instructions); `opt` makes it
/// `trunc (smin x, 255)`.
#[test]
fn a_min_whose_arm_is_a_truncation_is_made_in_the_wide_type() {
    let out = combined(
        "define i8 @f(i32 %x) {
b0:
  %c2 = icmp sgt i32 %x, 255
  %t = trunc i32 %x to i8
  %s = select i1 %c2, i8 -1, i8 %t
  ret i8 %s
}
",
    );
    assert!(out.contains("@llvm.smin.i32(i32 %x, i32 255)"), "{out}");
    assert!(out.contains("trunc i32"), "{out}");
    assert!(!out.contains("select"), "{out}");
}

/// The whole clamp in one run: `smax` then `smin`, then one truncation. The
/// `smin` the run declared was not a min to its own queries, so the clamp
/// waited for the next run.
#[test]
fn a_clamp_stored_as_a_byte_is_smax_smin_and_a_truncation() {
    let out = combined_by(
        "define i8 @f(i32 %x) {
b0:
  %c = icmp slt i32 %x, 0
  %c2 = icmp sgt i32 %x, 255
  %t = trunc i32 %x to i8
  %s = select i1 %c2, i8 -1, i8 %t
  %r = select i1 %c, i8 0, i8 %s
  ret i8 %r
}
",
        visit_all,
    );
    assert!(out.contains("@llvm.smax.i32(i32 %x, i32 0)"), "{out}");
    assert!(out.contains("@llvm.smin.i32("), "{out}");
    assert!(!out.contains("select"), "{out}");
}
