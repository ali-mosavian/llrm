//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what the pinned `opt -passes=simplifycfg` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// @f of `text` simplified, with native integers; its printed text and
/// whether it changed. It computes what it did on `inputs`.
fn run(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("n8:16:32:64").expect("a datalayout");
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    // The pinned `opt` leaves `switch-range-to-icmp` off.
    let options = Options { convert_switch_range_to_icmp: false, ..Options::default() };
    let changed = simplified_with(&options, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("placed");
    let text = printed(&module);
    assert_eq!(results(&module, inputs), before, "{text}");
    (text, changed)
}

/// The declarations of the compare intrinsics and the cell the arms write.
const PRELUDE: &str = "@g = global i32 0

declare i8 @llvm.scmp.i8.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i8 @llvm.ucmp.i8.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

";

/// An arm of @f: a block `name` that stores `value` and returns it.
fn arm(
    name: &str,
    value: i32,
) -> String {
    format!("{name}:\n  store i32 {value}, ptr @g\n  ret i32 {value}\n\n")
}

/// Case 5 of `a & 3` stayed in the switch.
#[test]
fn test_a_case_the_known_bits_rule_out_is_removed() {
    let text = format!(
        "{PRELUDE}define i32 @f(i32 %a) {{
b0:
  %c = and i32 %a, 3
  switch i32 %c, label %d [
    i32 1, label %x
    i32 2, label %y
    i32 5, label %z
  ]

{}{}{}{}}}
",
        arm("x", 10),
        arm("y", 20),
        arm("z", 50),
        arm("d", 0).trim_end()
    );
    let (out, changed) = run(&text, &[&[0], &[1], &[2], &[3], &[5]]);
    assert!(changed);
    assert!(!out.contains("i32 5, label"), "{out}");
}

/// Cases 0 and 1 of `a & 1` left a default the value cannot reach.
#[test]
fn test_cases_that_cover_every_value_make_the_default_unreachable() {
    let text = format!(
        "{PRELUDE}define i32 @f(i32 %a) {{
b0:
  %c = and i32 %a, 1
  switch i32 %c, label %d [
    i32 0, label %x
    i32 1, label %y
  ]

{}{}{}}}
",
        arm("x", 10),
        arm("y", 20),
        arm("d", 0).trim_end()
    );
    let (out, changed) = run(&text, &[&[0], &[1], &[2], &[3]]);
    assert!(changed);
    assert!(out.contains("switch i32 %c, label %b0.unreachabledefault ["), "{out}");
    assert!(out.contains("b0.unreachabledefault:\n  unreachable"), "{out}");
}

/// Cases 0, 1 and 2 of `a & 3` left the default taking the only value
/// missing, 3, without being a case of its own.
#[test]
fn test_the_one_missing_case_is_given_to_the_default_block() {
    let text = format!(
        "{PRELUDE}define i32 @f(i32 %a) {{
b0:
  %c = and i32 %a, 3
  switch i32 %c, label %d [
    i32 0, label %x
    i32 1, label %y
    i32 2, label %z
  ]

{}{}{}{}}}
",
        arm("x", 10),
        arm("y", 20),
        arm("z", 50),
        arm("d", 7).trim_end()
    );
    let (out, changed) = run(&text, &[&[0], &[1], &[2], &[3], &[7]]);
    assert!(changed);
    assert!(out.contains("i32 3, label %d"), "{out}");
    assert!(out.contains("label %b0.unreachabledefault ["), "{out}");
}

/// Case 3 of a value that is 1 or 2 stayed in the switch.
#[test]
fn test_a_case_outside_the_values_of_a_phi_is_removed() {
    let text = format!(
        "{PRELUDE}define i32 @f(i1 %b, i32 %v) {{
b0:
  br i1 %b, label %p, label %q

p:
  br label %j

q:
  br label %j

j:
  %c = phi i32 [ 1, %p ], [ 2, %q ]
  switch i32 %c, label %d [
    i32 1, label %x
    i32 2, label %y
    i32 3, label %z
  ]

{}{}{}{}}}
",
        arm("x", 10),
        arm("y", 20),
        arm("z", 50),
        arm("d", 0).trim_end()
    );
    let (out, changed) = run(&text, &[&[0, 5], &[1, 5]]);
    assert!(changed);
    assert!(!out.contains("i32 3, label"), "{out}");
}

/// `switch (scmp a, b)` with -1 and 0 to one block and a default stayed a
/// switch instead of `a > b`.
#[test]
fn test_a_switch_on_scmp_is_one_compare() {
    let text = format!(
        "{PRELUDE}define i32 @f(i32 %a, i32 %b) {{
b0:
  %c = call i8 @llvm.scmp.i8.i32(i32 %a, i32 %b)
  switch i8 %c, label %d [
    i8 -1, label %x
    i8 0, label %x
  ]

{}{}}}
",
        arm("x", 10),
        arm("d", 0).trim_end()
    );
    let (out, changed) = run(&text, &[&[1, 2], &[2, 2], &[3, 2]]);
    assert!(changed);
    assert!(out.contains("%0 = icmp sgt i32 %a, %b\n  br i1 %0, label %d, label %x"), "{out}");
    assert!(!out.contains("scmp.i8.i32(i32 %a"), "{out}");
}

/// `switch (ucmp a, b)` with three cases and an unreachable default stayed
/// a switch instead of `a < b`.
#[test]
fn test_a_three_case_switch_on_ucmp_is_one_compare() {
    let text = format!(
        "{PRELUDE}define i32 @f(i32 %a, i32 %b) {{
b0:
  %c = call i8 @llvm.ucmp.i8.i32(i32 %a, i32 %b)
  switch i8 %c, label %u [
    i8 -1, label %x
    i8 0, label %y
    i8 1, label %y
  ]

{}{}u:
  unreachable
}}
",
        arm("x", 10),
        arm("y", 20)
    );
    let (out, changed) = run(&text, &[&[1, 2], &[2, 2], &[3, 2]]);
    assert!(changed);
    assert!(out.contains("%0 = icmp ult i32 %a, %b\n  br i1 %0, label %x, label %y"), "{out}");
}
