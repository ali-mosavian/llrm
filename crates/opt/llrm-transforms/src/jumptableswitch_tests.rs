//! `JumpTableToSwitch`: a call through a table of functions is a switch of
//! direct calls.

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::context::{GlobalId, mask};
use llrm_mir::interpret::Val;
use llrm_mir::module::GlobalKind;
use llrm_mir::passes::{Outer, Size};

use crate::testing::{parsed, printed, results};

/// What the table's call returns for `input` (`n`, `x`): the interpreter
/// cannot call through a computed pointer, so the converted module is checked
/// against this.
fn expected(input: &[i128]) -> i128 {
    let x = input[1];
    match input[0] & 3 {
        0 => x + 3,
        1 => x * 5,
        2 => x ^ 85,
        _ => x >> 1,
    }
}

/// `text`'s @f converted, printed; where it converted, the interpreter gives
/// `expected` on `inputs`.
fn converted(
    text: &str,
    inputs: &[&[i128]],
) -> (bool, String) {
    let mut module = parsed(&format!("{DOS}{text}"));
    let sizes: Vec<Size> = module
        .globals
        .iter()
        .map(|global| match &global.kind {
            GlobalKind::Function(function) if !function.is_declaration() => {
                Size::Instructions(function.live_instructions())
            }
            _ => Size::Declaration,
        })
        .collect();
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = super::converted(context, &layout, function, &outer, &mut |id: GlobalId| sizes[id.0 as usize]);
    let after = printed(&module);
    if changed {
        let want: Vec<_> =
            inputs.iter().map(|input| Val::Int { bits: mask(16) & expected(input) as u128, width: 16 }).collect();
        assert_eq!(results(&module, inputs), want, "{after}");
    }
    (changed, after)
}

fn callee(
    name: &str,
    body: &str,
) -> String {
    format!("define i16 @{name}(i16 %x) {{\nentry:\n{body}}}\n")
}

/// Four small functions and a call through the table, as `x_funcptr` does.
fn example(
    table: &str,
    extra: &str,
) -> String {
    let functions = [
        callee("f0", "  %r = add i16 %x, 3\n  ret i16 %r\n"),
        callee("f1", "  %r = mul i16 %x, 5\n  ret i16 %r\n"),
        callee("f2", "  %r = xor i16 %x, 85\n  ret i16 %r\n"),
        callee("f3", "  %r = ashr i16 %x, 1\n  ret i16 %r\n"),
    ]
    .concat();
    format!(
        "{extra}{functions}@t = {table} [4 x ptr] [ptr @f0, ptr @f1, ptr @f2, ptr @f3]

define i16 @f(i16 %n, i16 %x) {{
entry:
  %i = and i16 %n, 3
  %p = getelementptr [4 x ptr], ptr @t, i16 0, i16 %i
  %fn = load ptr, ptr %p
  %r = call i16 %fn(i16 %x)
  ret i16 %r
}}
"
    )
}

const INPUTS: &[&[i128]] = &[&[0, 7], &[1, 7], &[2, 7], &[3, 100], &[5, 9]];

/// Without the pass the call stayed indirect: four calls the inliner could
/// not see (`x_funcptr`: the pin makes it a switch and inlines each).
#[test]
fn a_call_through_a_table_is_a_switch_of_direct_calls() {
    let (changed, after) = converted(&example("internal constant", ""), INPUTS);
    assert!(changed, "{after}");
    let body = after.split("define i16 @f(").nth(1).expect("@f");
    assert!(body.contains("switch i16 %i"), "{after}");
    assert!(body.contains("default.switch.case.unreachable"), "{after}");
    for entry in 0..4 {
        assert!(body.contains(&format!("call.{entry}:")), "{after}");
        assert!(body.contains(&format!("call i16 @f{entry}(i16 %x)")), "{after}");
    }
    assert!(!body.contains("call i16 %fn"), "{after}");
    assert!(body.contains("phi i16"), "{after}");
}

/// A table that is not constant may change: the call stays.
#[test]
fn a_table_that_is_not_constant_stays() {
    let (changed, after) = converted(&example("internal global", ""), INPUTS);
    assert!(!changed, "{after}");
}

/// A callee of more than 50 instructions keeps the table
/// (`jump-table-to-switch-function-size-threshold`).
#[test]
fn a_callee_over_the_size_threshold_keeps_the_table() {
    let mut big = String::new();
    let mut last = "%x".to_owned();
    for at in 0..50 {
        big.push_str(&format!("  %a{at} = add i16 {last}, 1\n"));
        last = format!("%a{at}");
    }
    big.push_str(&format!("  ret i16 {last}\n"));
    let text = example("internal constant", "")
        .replace(&callee("f3", "  %r = ashr i16 %x, 1\n  ret i16 %r\n"), &callee("f3", &big));
    let (changed, after) = converted(&text, INPUTS);
    assert!(!changed, "{after}");
}

/// A call whose callee type differs from an entry's keeps the table.
#[test]
fn an_entry_of_another_type_keeps_the_table() {
    let text = example("internal constant", "").replace("define i16 @f3(i16 %x)", "define i16 @f3(i16 %x, i16 %y)");
    let (changed, after) = converted(&text, INPUTS);
    assert!(!changed, "{after}");
}

/// A function with no call through a table asks no size: where the pass finds
/// nothing, the manager counts nothing (the cost the pass adds to a compile
/// with no table).
#[test]
fn a_function_without_a_table_call_asks_no_size() {
    let mut module = parsed(&format!(
        "{DOS}{}define i16 @f(i16 %n, i16 %x) {{\nentry:\n  %r = call i16 @f0(i16 %x)\n  ret i16 %r\n}}\n",
        callee("f0", "  %r = add i16 %x, 3\n  ret i16 %r\n")
    ));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let mut asked = 0;
    let changed = super::converted(context, &layout, function, &outer, &mut |_| {
        asked += 1;
        Size::Unknown
    });
    assert!(!changed);
    assert_eq!(asked, 0);
}

/// A table call asks the size of each entry, and an `Unknown` answer converts
/// nothing: the manager counts and runs the pass again.
#[test]
fn an_unknown_size_converts_nothing_and_is_asked() {
    let mut module = parsed(&format!("{DOS}{}", example("internal constant", "")));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let mut asked = Vec::new();
    let changed = super::converted(context, &layout, function, &outer, &mut |id| {
        asked.push(id);
        Size::Unknown
    });
    assert!(!changed);
    assert_eq!(asked.len(), 4, "every entry is asked, so one count serves the table");
}

/// Through the pass manager: the first run asks the sizes, the manager counts
/// them and runs the pass over @f again, and the table is a switch. Without
/// the rerun the table stayed a table for every real compile.
#[test]
fn the_manager_counts_the_callees_and_runs_the_pass_again() {
    let mut module = parsed(&format!("{DOS}{}", example("internal constant", "")));
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(super::JumpTableToSwitch);
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("a run");
    let after = printed(&module);
    assert!(after.contains("switch i16 %i"), "{after}");
    assert!(!after.contains("call i16 %fn"), "{after}");
    let want: Vec<_> =
        INPUTS.iter().map(|input| Val::Int { bits: mask(16) & expected(input) as u128, width: 16 }).collect();
    assert_eq!(results(&module, INPUTS), want, "{after}");
}
