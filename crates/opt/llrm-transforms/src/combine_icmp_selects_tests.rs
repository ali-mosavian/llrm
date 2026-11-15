use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitICmpInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if matches!(c.function.instruction(inst).opcode, Opcode::ICmp(_)) {
        crate::combine_compares::visit_icmp(c, inst)
    } else {
        Visit::None
    }
}

fn run(text: &str) -> String {
    let mut module = parsed(text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// Opt: the false arms compare as `0 ult 2`-style constants, so the compare is
/// of the true arms under the condition. Without the fold the compare kept
/// both selects (known_loads_walks -O2: the loop's recurrences stayed).
#[test]
fn a_compare_of_two_selects_on_one_condition_compares_the_arm_that_does_not_fold() {
    // `%y` has a second user, so `foldSelectICmp` leaves it and this fold runs.
    let out = run(
        "define i8 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %x = select i1 %c, i8 %q, i8 2\n  %y = select i1 %c, i8 %p, i8 0\n  %r = icmp ult i8 %x, %y\n  %e = zext i1 %r to i8\n  %o = add i8 %e, %y\n  ret i8 %o\n}\n",
    );
    assert!(
        out.contains("icmp ult i8 %q, %p") && out.contains("select i1 %c, i1 %") && out.contains("i1 false"),
        "{out}"
    );
}

/// Both selects have another user: the compare stays.
#[test]
fn selects_that_have_other_users_are_left() {
    let text = "define i1 @f(i1 %c, i8 %p, i8 %q) {\nb0:\n  %x = select i1 %c, i8 %q, i8 2\n  %y = select i1 %c, i8 %p, i8 0\n  %r = icmp ult i8 %x, %y\n  %s = icmp eq i8 %x, %y\n  %t = and i1 %r, %s\n  %u = xor i1 %t, %r\n  ret i1 %u\n}\n";
    let out = run(text);
    assert!(out.contains("%r = icmp ult i8 %x, %y"), "{out}");
}
