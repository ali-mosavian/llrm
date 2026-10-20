//! `getCapturesBefore` of the two providers, against BasicAliasAnalysis.cpp's
//! rules.

use llrm_mir::module::ValueDef;

use super::*;
use crate::testing::{function, parsed, value};

const KEEP: &str = "declare void @keep(ptr)\n";

fn inst_named(
    function: &llrm_mir::module::Function,
    name: &str,
) -> InstId {
    match function.value(value(function, name)).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => panic!("%{name} is an argument"),
    }
}

/// `%slot` is read, then captured by the call. A context before the call sees
/// no capture; one at the call sees it only with `OrAt`; the simple provider
/// cannot tell and always says captured.
#[test]
fn a_load_before_the_escape_is_not_captured_before() {
    let module = parsed(&format!(
        "{KEEP}define void @f() {{\nb:\n  %slot = alloca i32\n  %v = load i32, ptr %slot\n  %c = call i32 @tag()\n  call void @keep(ptr %slot)\n  ret void\n}}\ndeclare i32 @tag()\n"
    ));
    let f = function(&module, "f");
    let layout = crate::testing::layout(&module);
    let unit = crate::memory::Unit::of(&module, &layout, f);
    let slot = Operand::Value(value(f, "slot"));
    let (load, tag) = (inst_named(f, "v"), inst_named(f, "c"));
    let keep = f.users(value(f, "slot")).iter().map(|one| one.user).find(|&user| user != load).expect("the call");
    let mut earliest = EarliestEscapeAnalysis::default();
    let mut simple = SimpleCaptureAnalysis::default();
    // Premise: the object is captured somewhere.
    assert!(simple.captures_before(&unit, slot, None, true, false).captures_anything());
    for before in [load, tag] {
        assert!(earliest.captures_before(&unit, slot, Some(before), false, false).captures_nothing());
        assert!(simple.captures_before(&unit, slot, Some(before), false, false).captures_anything());
    }
    assert!(earliest.captures_before(&unit, slot, Some(keep), true, false).captures_anything());
    assert!(earliest.captures_before(&unit, slot, Some(keep), false, false).captures_nothing());
    assert!(earliest.captures_before(&unit, slot, None, false, false).captures_anything());
}

/// An escape in a loop is before every instruction of the loop, so a context in
/// the loop sees it.
#[test]
fn an_escape_in_a_loop_is_before_the_loop_body() {
    let module = parsed(&format!(
        "{KEEP}define void @f(i1 %c) {{\nentry:\n  %slot = alloca i32\n  br label %body\nbody:\n  %v = load i32, ptr %slot\n  call void @keep(ptr %slot)\n  br i1 %c, label %body, label %exit\nexit:\n  ret void\n}}\n"
    ));
    let f = function(&module, "f");
    let layout = crate::testing::layout(&module);
    let unit = crate::memory::Unit::of(&module, &layout, f);
    let slot = Operand::Value(value(f, "slot"));
    let mut earliest = EarliestEscapeAnalysis::default();
    let before = earliest.captures_before(&unit, slot, Some(inst_named(f, "v")), false, false);
    assert!(before.captures_anything(), "the next iteration reads after the escape");
}

/// Each object is searched once, whatever the number of questions or objects:
/// work grows with the objects, not with the questions.
#[test]
fn each_object_is_searched_once_at_one_and_four_times_the_size() {
    let searches = |objects: usize| {
        let mut body = String::from("define void @f() {\nb:\n");
        for i in 0..objects {
            body.push_str(&format!("  %s{i} = alloca i32\n  store i32 1, ptr %s{i}\n  call void @keep(ptr %s{i})\n"));
        }
        body.push_str("  ret void\n}\n");
        let module = parsed(&format!("{KEEP}{body}"));
        let f = function(&module, "f");
        let layout = crate::testing::layout(&module);
        let unit = crate::memory::Unit::of(&module, &layout, f);
        let mut earliest = EarliestEscapeAnalysis::default();
        let before = SEARCHES.with(std::cell::Cell::get);
        let ret = f.terminator(f.layout()[0]).unwrap();
        for _ in 0..3 {
            for i in 0..objects {
                earliest.captures_before(&unit, Operand::Value(value(f, &format!("s{i}"))), Some(ret), false, false);
            }
        }
        SEARCHES.with(std::cell::Cell::get) - before
    };
    assert_eq!((searches(8), searches(32)), (8, 32));
}
