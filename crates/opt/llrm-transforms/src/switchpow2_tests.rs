//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! shapes are what the pinned `opt -passes=simplifycfg -mcpu=i486` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// @f of `text` simplified for the i486 with native integers; its printed
/// text. It computes what it did on `inputs`.
fn run(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("n8:16:32").expect("a datalayout");
    let tti = llrm_x86::tti::X86::of_cpu("i486").expect("an i486");
    let spec = Speculation { tti: &tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    // The pinned `opt` leaves `switch-range-to-icmp` off.
    let options = Options { convert_switch_range_to_icmp: false, ..Options::default() };
    simplified_with(&options, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("placed");
    let text = printed(&module);
    assert_eq!(results(&module, inputs), before, "{text}");
    text
}

/// @f on `a` with `cases` (value, arm) and the arms @x, @y, @z that store
/// and return 10, 20, 50; @d, the default, stores and returns 7 or is
/// unreachable.
fn switch(
    cases: &[(i32, char)],
    reachable_default: bool,
) -> String {
    let arms: String = cases.iter().map(|(value, to)| format!("    i32 {value}, label %{to}\n")).collect();
    let arm = |name: &str, value: i32| format!("{name}:\n  store i32 {value}, ptr @g\n  ret i32 {value}\n\n");
    let default = if reachable_default { arm("d", 7).trim_end().to_owned() } else { "d:\n  unreachable".to_owned() };
    format!(
        "@g = global i32 0\n\ndefine i32 @f(i32 %a) {{\nb0:\n  switch i32 %a, label %d [\n{arms}  ]\n\n{}{}{}{default}\n}}\n",
        arm("x", 10),
        arm("y", 20),
        arm("z", 50)
    )
}

const CASES: [(i32, char); 4] = [(1, 'x'), (2, 'y'), (64, 'z'), (128, 'y')];
const INPUTS: &[&[i128]] = &[&[1], &[2], &[3], &[64], &[128], &[0], &[96], &[-1]];

/// Cases 1, 2, 64 and 128 stayed a sparse switch on the value itself.
#[test]
fn test_a_switch_of_powers_of_two_is_on_the_exponent() {
    let out = run(&switch(&CASES, true), INPUTS);
    assert!(
        out.contains("%0 = call i32 @llvm.ctpop.i32(i32 %a)\n  %1 = icmp eq i32 %0, 1\n  br i1 %1, label"),
        "{out}"
    );
    assert!(out.contains("call i32 @llvm.cttz.i32(i32 %a, i1 true)\n  switch i32 %3, label %d ["), "{out}");
    assert!(out.contains("i32 0, label") && out.contains("i32 1, label") && out.contains("i32 6, label"), "{out}");
    assert!(out.contains("i32 7, label"), "{out}");
}

/// With an unreachable default no `ctpop` guard was needed, yet one was made
/// (or the switch stayed on the value).
#[test]
fn test_an_unreachable_default_needs_no_power_of_two_guard() {
    let out = run(&switch(&CASES, false), &[&[1], &[2], &[64], &[128]]);
    assert!(out.contains("%0 = call i32 @llvm.cttz.i32(i32 %a, i1 true)\n  switch i32 %0, label %d ["), "{out}");
    assert!(!out.contains("ctpop"), "{out}");
}

/// A case that is not a power of two left the switch alone, but it was
/// rewritten on `cttz`, which sends 3 to the case of 1.
#[test]
fn test_a_case_that_is_not_a_power_of_two_leaves_the_switch() {
    let out = run(&switch(&[(1, 'x'), (2, 'y'), (3, 'z'), (128, 'y')], true), INPUTS);
    assert!(!out.contains("cttz"), "{out}");
}

/// A switch of three cases was rewritten; SDAG makes no jump table of fewer
/// than four.
#[test]
fn test_a_switch_of_three_powers_of_two_is_left() {
    let out = run(&switch(&[(1, 'x'), (2, 'y'), (64, 'z')], true), INPUTS);
    assert!(!out.contains("cttz"), "{out}");
}
