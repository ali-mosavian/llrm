use llrm_mir::datalayout::DataLayout;

use crate::combine::combine;
use crate::testing::{parsed, printed};

/// `@f` combined by the whole combiner, printed.
fn combined(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let mut declared = llrm_mir::passes::Declared::over(std::rc::Rc::new(globals.clone()), metadata.len());
    combine(context, &globals, &Default::default(), &metadata, &DataLayout::default(), function, &mut declared);
    printed(&module)
}

/// `(down & sign_ne) & mask_ne` with `down` read again: the two compares are
/// `and`ed first and `down` last, as the pin's `round_whole` (QuickrBasic
/// `double.c`): the compares were split by `down`, so the backend could not
/// chain them with `ccmp` (6 more arm64 instructions).
#[test]
fn a_multi_use_operand_moves_out_of_the_pair_of_one_use_compares() {
    let text = combined(
        "define i1 @f(i1 %x, i64 %a, i64 %b, ptr %m) {\nb0:\n  store i1 %x, ptr %m\n  %p = icmp ne i64 %a, 0\n  %q = and i1 %x, %p\n  %r = icmp ne i64 %b, 0\n  %s = and i1 %q, %r\n  ret i1 %s\n}\n",
    );
    assert!(text.contains("%0 = and i1 %p, %r"), "{text}");
    assert!(text.contains("%1 = and i1 %0, %x"), "{text}");
}

/// Both operands one-use: nothing to gain, nothing changes.
#[test]
fn one_use_operands_are_left_alone() {
    let text = combined(
        "define i1 @f(i1 %x, i64 %a, i64 %b) {\nb0:\n  %p = icmp ne i64 %a, 0\n  %q = and i1 %x, %p\n  %r = icmp ne i64 %b, 0\n  %s = and i1 %q, %r\n  ret i1 %s\n}\n",
    );
    assert!(text.contains("%q = and i1 %x, %p") || text.contains("%q = and i1 %p, %x"), "{text}");
    assert!(text.contains("%s = and i1 %q, %r"), "{text}");
}
