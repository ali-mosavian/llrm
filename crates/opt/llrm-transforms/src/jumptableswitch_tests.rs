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
    let changed = super::converted(context, &layout, function, &outer, &|id: GlobalId| sizes[id.0 as usize]);
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
    let asked = std::cell::Cell::new(0);
    let changed = super::converted(context, &layout, function, &outer, &|_| {
        asked.set(asked.get() + 1);
        Size::Unknown
    });
    assert!(!changed);
    assert_eq!(asked.get(), 0);
}

/// A table whose entries' sizes no one knows converts nothing.
#[test]
fn an_unknown_size_converts_nothing() {
    let mut module = parsed(&format!("{DOS}{}", example("internal constant", "")));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(!super::converted(context, &layout, function, &outer, &|_| Size::Unknown));
}

/// Through the pass manager: it counts the functions the table names, and the
/// table is a switch.
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

/// `x_funcptr` as the C frontend makes it at m32: a table that is a packed
/// struct of pointers, indexed through `[4 x i8]` by a 32-bit value.
#[test]
fn a_table_that_is_a_packed_struct_of_pointers_is_a_switch() {
    let text = "target datalayout = \"e-p:32:32-i32:32-n8:16:32\"

define i32 @f0(i32 %x) {
entry:
  %r = add i32 %x, 3
  ret i32 %r
}
define i32 @f1(i32 %x) {
entry:
  %r = mul i32 %x, 5
  ret i32 %r
}
@t = internal constant <{ ptr, ptr }> <{ ptr @f0, ptr @f1 }>

define i32 @f(i32 %n, i32 %x) {
entry:
  %i = and i32 %n, 1
  %p = getelementptr inbounds nuw [4 x i8], ptr @t, i32 %i
  %fn = load ptr, ptr %p
  %r = call i32 %fn(i32 %x)
  ret i32 %r
}
";
    let mut module = parsed(text);
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
    let changed = super::converted(context, &layout, function, &outer, &|id: GlobalId| sizes[id.0 as usize]);
    assert!(changed, "{}", printed(&module));
}

/// Through the whole pipeline in LLVM's order: `x_funcptr` compiled with the
/// pass in the dist and kept its four indirect calls: the pipeline's passes
/// run inside one `pipeline` pass, which skips a pass that found nothing on a
/// second run, and the first run had found nothing for want of the sizes.
#[test]
fn the_pipeline_makes_the_table_call_a_switch() {
    let text = format!("{DOS}{}", example("internal constant", "").replace("define i16 @f(", "define i16 @main(i16 %n, i16 %x) {\nentry:\n  %r = call i16 @f(i16 %n, i16 %x)\n  ret i16 %r\n}\n\ndefine i16 @f("));
    let mut module = parsed(&text);
    let applied = crate::pipeline::Applied {
        options: crate::pipeline::Options { pre: true, ..Default::default() },
        ..Default::default()
    };
    llrm_mir::program::Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("main".to_owned());
        crate::pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap();
    let after = printed(&module);
    assert!(!after.contains("call i16 %"), "{after}");
}
