use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;

use crate::combine::{Combiner, Visit, combine_with};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(
    params: &str,
    body: &str,
) -> String {
    let text = format!("define i1 @f({params}) {{\nb0:\n{body}}}\n");
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with(context, &globals, &metadata, &DataLayout::default(), function, visit);
    printed(&module)
}

/// wasm32 known_loads_walks: `select (a|n), (b|n), false` with every operand
/// used twice stayed two ors and a logical and per unrolled iteration; the
/// pin's `select a, (b|n), n` leaves the factoring to the next visit. +122 B
/// at O2.
#[test]
fn a_select_on_an_or_whose_arm_the_second_operand_settles_selects_on_the_first() {
    let after = combined(
        "i1 %x, i1 %y, i1 %n",
        "  %a = or i1 %x, %n\n  %b = or i1 %y, %n\n  %s = select i1 %a, i1 %b, i1 false\n  %u = xor i1 %s, %a\n  %w = xor i1 %u, %b\n  ret i1 %w\n",
    );
    assert!(after.contains("select i1 %x, i1 %b, i1 %n"), "{after}");
    assert!(!after.contains("i1 false"), "{after}");
}

/// A plain `and` tries its operands in both orders: `k` is the second here.
#[test]
fn a_select_on_an_and_tries_both_operand_orders() {
    let after = combined(
        "i1 %x, i1 %y, i1 %k",
        "  %c = and i1 %k, %x\n  %t = or i1 %k, %y\n  %s = select i1 %c, i1 %t, i1 false\n  %u = xor i1 %s, %c\n  %w = xor i1 %u, %t\n  ret i1 %w\n",
    );
    assert!(after.contains("select i1 %x, i1 %k, i1 false"), "{after}");
}

/// Nothing settles the arms: the select is left.
#[test]
fn a_select_on_an_or_nothing_settles_is_left() {
    let after = combined(
        "i1 %x, i1 %y, i1 %n, i1 %m",
        "  %a = or i1 %x, %n\n  %b = or i1 %y, %m\n  %s = select i1 %a, i1 %b, i1 false\n  %u = xor i1 %s, %a\n  %w = xor i1 %u, %b\n  ret i1 %w\n",
    );
    assert!(after.contains("select i1 %a, i1 %b, i1 false"), "{after}");
}
