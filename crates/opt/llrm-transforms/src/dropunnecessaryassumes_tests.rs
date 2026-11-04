//! `DropUnnecessaryAssumes`: an assume nothing else depends on goes with the
//! condition only it reads.

use llrm_analysis::testing::layout;
use llrm_mir::passes::Outer;

use crate::testing::{parsed, printed, results};

/// `text`'s @f with its unnecessary assumes dropped, printed; the interpreter
/// agrees on `inputs`.
fn dropped(
    text: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    let mut module = parsed(text);
    let before = module.clone();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed =
        super::drop_unnecessary_assumes(context, &outer.globals, &outer.metadata, &layout, outer.callees(), function);
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (changed, after)
}

fn assumes(text: &str) -> usize {
    text.matches("call void @llvm.assume(").count()
}

const INPUTS: &[&[i128]] = &[&[0], &[3], &[8]];

/// An `f` whose only reader of `%x` is the compare the assume reads, plus
/// `extra` more pure readers of `%x`.
fn readers(extra: usize) -> String {
    let adds: String = (0..extra).map(|i| format!("  %u{i} = add i16 %x, {i}\n")).collect();
    format!(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %a) {{
b0:
  %x = mul i16 %a, 3
{adds}  %c = icmp ult i16 %x, 10
  call void @llvm.assume(i1 %c)
  ret i16 %a
}}
"
    )
}

/// Without the pass the assume, its compare and `%x` stayed: the assume made
/// `%x` and the compare live for a fact no code used.
#[test]
fn test_an_assume_nothing_else_uses_goes_with_its_dead_condition() {
    let (changed, after) = dropped(&readers(0), INPUTS);
    assert!(changed, "{after}");
    assert_eq!(assumes(&after), 0, "{after}");
    assert!(!after.contains("icmp") && !after.contains("add"), "the condition is deleted too: {after}");
}

/// A store of `%x` is a use that matters: the assume is the only fact the
/// optimizer has about it, so it stays.
#[test]
fn test_an_assume_on_a_value_a_store_uses_is_kept() {
    let text = "@g = global i16 0

declare void @llvm.assume(i1)

define i16 @f(i16 %a) {
b0:
  %x = mul i16 %a, 3
  %c = icmp ult i16 %x, 10
  call void @llvm.assume(i1 %c)
  store i16 %x, ptr @g
  ret i16 %a
}
";
    let (changed, after) = dropped(text, INPUTS);
    assert!(!changed, "{after}");
    assert_eq!(assumes(&after), 1, "{after}");
}

/// The walk gives up at 32 entries, even to look at one it already holds: the
/// assume, the compare and 29 other readers fit and the assume goes; with one
/// more reader the compare's own look at the assume bails and it stays.
#[test]
fn test_the_walk_gives_up_past_32_users() {
    let (changed, after) = dropped(&readers(29), INPUTS);
    assert!(changed && assumes(&after) == 0, "31 entries: {after}");
    let (changed, after) = dropped(&readers(30), INPUTS);
    assert!(!changed && assumes(&after) == 1, "32 entries: {after}");
}

/// A type test has semantics of its own: it names no affected value, so the
/// walk alone would drop it. The verifier does not know the intrinsic, so the
/// body is not verified.
#[test]
fn test_an_assume_of_a_type_test_is_kept() {
    let text = "declare void @llvm.assume(i1)
declare i1 @llvm.type.test(ptr, i16)

define i16 @f(i16 %a, ptr %p) {
b0:
  %t = call i1 @llvm.type.test(ptr %p, i16 1)
  call void @llvm.assume(i1 %t)
  ret i16 %a
}
";
    let mut module = parsed(text);
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed =
        super::drop_unnecessary_assumes(context, &outer.globals, &outer.metadata, &layout, outer.callees(), function);
    let after = llrm_mir::print::module(&module);
    assert!(!changed, "{after}");
    assert_eq!(assumes(&after), 1, "{after}");
}

/// A global's use list reaches other functions: an assume on one is kept.
#[test]
fn test_an_assume_on_a_global_is_kept() {
    let text = "@g = global i16 0

declare void @llvm.assume(i1)

define i16 @f(i16 %a) {
b0:
  %c = icmp ne ptr @g, null
  call void @llvm.assume(i1 %c)
  ret i16 %a
}
";
    let (changed, after) = dropped(text, INPUTS);
    assert!(!changed, "{after}");
    assert_eq!(assumes(&after), 1, "{after}");
}
