use llrm_mir::datalayout::DataLayout;

use super::instsimplify;
use crate::testing::{parsed, printed, results};

/// @f of `text` after InstSimplify; whether anything changed.
fn simplified(text: &str) -> (String, bool, bool) {
    let mut module = parsed(text);
    let before = results(&module, &[&[3, 5], &[0, 0], &[-7, 2]]);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = instsimplify(context, &globals, &metadata, &DataLayout::default(), &callees, function);
    let same = results(&module, &[&[3, 5], &[0, 0], &[-7, 2]]) == before;
    (printed(&module), changed, same)
}

/// A chain that collapses: `(x + 0) * 1` is `x`, the compare of the result
/// with itself is true; the users of each replacement are visited again, so
/// the whole chain goes in one run and the dead arithmetic with it.
#[test]
fn a_chain_of_identities_collapses_to_its_operand() {
    let (text, changed, same) = simplified(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = add i16 %x, 0
  %m = mul i16 %a, 1
  %c = icmp eq i16 %m, %x
  %s = select i1 %c, i16 %m, i16 %y
  ret i16 %s
}
",
    );
    assert!(changed && same);
    assert_eq!(
        text,
        "define i16 @f(i16 %x, i16 %y) {
b0:
  ret i16 %x
}
"
    );
}

/// Nothing to simplify leaves the body as it was and says so.
#[test]
fn a_body_with_nothing_to_simplify_is_unchanged() {
    let body = "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = add i16 %x, %y
  ret i16 %a
}
";
    let (text, changed, _) = simplified(body);
    assert!(!changed);
    assert_eq!(text, body);
}

/// An instruction in an unreachable block is left alone: it can take forms
/// the simplifier is not prepared for.
#[test]
fn an_unreachable_block_is_not_visited() {
    let body = "define i16 @f(i16 %x, i16 %y) {
b0:
  ret i16 %x
b1:
  %a = add i16 %x, 0
  ret i16 %a
}
";
    let (text, changed, _) = simplified(body);
    assert!(!changed, "{text}");
}

/// A call that reads and writes nothing and returns, whose only reader
/// simplified away, goes with it (`RecursivelyDeleteTriviallyDeadInstructions`
/// over `isInstructionTriviallyDead`). LFTR's `smax` of an exit limit stayed
/// in d_faces' body until `ipa-pure` removed six of them and the whole
/// Simplify list ran again over it: -O1 1317 -> 1725 M instructions.
#[test]
fn a_pure_call_whose_reader_simplified_away_is_deleted() {
    let mut module = parsed(
        "declare i16 @pure(i16) readnone nounwind willreturn
declare i16 @spins(i16) readnone nounwind

define i16 @f(i16 %x, i16 %y) {
b0:
  %p = call i16 @pure(i16 %x)
  %q = call i16 @spins(i16 %x)
  %c = icmp eq i16 %p, %p
  %d = icmp eq i16 %q, %q
  %s = select i1 %c, i16 %x, i16 %y
  %t = select i1 %d, i16 %s, i16 %y
  ret i16 %t
}
",
    );
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(instsimplify(context, &globals, &metadata, &DataLayout::default(), &callees, function));
    let text = printed(&module);
    assert!(!text.contains("call i16 @pure"), "the pure call stands:\n{text}");
    assert!(text.contains("call i16 @spins"), "a call that may not return went:\n{text}");
}
