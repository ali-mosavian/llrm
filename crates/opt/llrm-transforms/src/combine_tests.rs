use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use super::combine;
use crate::testing::{parsed, printed};

/// `@f` combined; whether anything changed.
fn combined(module: &mut Module) -> bool {
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine(context, &globals, &metadata, &DataLayout::default(), function)
}

fn check(
    text: &str,
    expected: &str,
) {
    let mut module = parsed(text);
    let changed = combined(&mut module);
    assert_eq!(printed(&module), expected);
    assert_eq!(changed, text != expected);
}

/// What simplifies away takes its users with it, in one visit each: `x + 0` is
/// `x`, so `(x + 0) - x` is `x - x`, which is `0`, so the return reads a
/// number.
#[test]
fn a_chain_of_identities_folds_in_one_drain() {
    check(
        "define i16 @f(i16 %x) {\nb0:\n  %a = add i16 %x, 0\n  %b = sub i16 %a, %x\n  %c = add i16 %b, 5\n  ret i16 %c\n}\n",
        "define i16 @f(i16 %x) {\nb0:\n  ret i16 5\n}\n",
    );
}

/// A branch on a number leaves one live successor: the phi input over the dead
/// edge is poison, so the phi has one value.
#[test]
fn a_constant_branch_makes_the_dead_phi_input_poison_and_the_phi_its_other() {
    check(
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 true, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %r = phi i16 [ %x, %a ], [ %y, %b ]\n  ret i16 %r\n}\n",
        "define i16 @f(i16 %x, i16 %y) {\nb0:\n  br i1 true, label %a, label %b\n\na:\n  br label %j\n\nb:\n  br label %j\n\nj:\n  ret i16 %x\n}\n",
    );
}

/// The dead are deleted as chains: nothing reads the last, so each before it
/// goes.
#[test]
fn what_only_a_dead_instruction_read_goes_with_it() {
    check(
        "define i16 @f(i16 %x) {\nb0:\n  %a = mul i16 %x, 3\n  %b = add i16 %a, 7\n  ret i16 %x\n}\n",
        "define i16 @f(i16 %x) {\nb0:\n  ret i16 %x\n}\n",
    );
}

/// A body with nothing to do is left as it is.
#[test]
fn a_body_with_nothing_to_simplify_is_unchanged() {
    let text = "define i16 @f(i16 %x, i16 %y) {\nb0:\n  %a = add i16 %x, %y\n  ret i16 %a\n}\n";
    check(text, text);
}
