//! Adapted from llrm-core's `analysis/noreturn_tests.rs`, the ports of the
//! `noreturn` tests in `tests/test_noreturn.py` and `tests/test_sccp.py`.

use std::collections::BTreeSet;

use llrm_mir::interpret::{self, Trap, Val};
use llrm_mir::module::Module;

use super::*;
use crate::testing::{block, function, parsed};

fn names(module: &Module, items: &[&str]) -> BTreeSet<GlobalId> {
    items.iter().map(|one| module.named(one).unwrap_or_else(|| panic!("no @{one}"))).collect()
}

/// Every call in `@name` to `@callee`.
fn calls_to(module: &Module, name: &str, callee: &str) -> BTreeSet<InstId> {
    let function = function(module, name);
    let target = module.named(callee).unwrap();
    function.walk().map(|(_, inst)| inst).filter(|&inst| effects::callee(&module.context, function, inst) == Some(target)).collect()
}

/// Each block's instructions by mnemonic, in layout order.
fn shape(module: &Module, name: &str) -> Vec<Vec<&'static str>> {
    let function = function(module, name);
    function.layout().iter().map(|&one| function.block(one).instructions().iter().map(|&inst| function.instruction(inst).opcode.mnemonic()).collect()).collect()
}

/// `after_terminal_calls` on `@name` at its calls to `@callee`.
fn cut(module: &mut Module, name: &str, callee: &str) -> bool {
    let sites = calls_to(module, name, callee);
    let (context, function) = module.function_mut(name).unwrap();
    let changed = after_terminal_calls(context, function, &sites);
    assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new());
    changed
}

fn run(module: &Module, name: &str, arguments: Vec<Val>) -> Result<Val, Trap> {
    interpret::run(module, name, arguments, 1000)
}

fn bit(value: bool) -> Val {
    Val::Int { bits: u128::from(value), width: 1 }
}

/// Block ids by name, for the block-level facts.
fn ats(module: &Module, name: &str, blocks: &[&str]) -> BTreeSet<i64> {
    blocks.iter().map(|one| id(block(function(module, name), one))).collect()
}

#[test]
fn test_closed_local_terminal_scc_is_noreturn() {
    let module = parsed(
        "define void @first() {
b:
  call void @second()
  ret void
}

define void @second() {
b:
  call void @first()
  ret void
}
",
    );
    let both = names(&module, &["first", "second"]);
    assert_eq!(inferred(&module, &module.declarations(), &both), both);
}

#[test]
fn test_a_callee_outside_the_bodies_is_not_assumed_terminal() {
    let module = parsed(
        "declare void @external()

define void @spin() {
b:
  br label %b
}

define void @caller() {
b:
  call void @spin()
  call void @external()
  ret void
}
",
    );
    assert_eq!(inferred(&module, &module.declarations(), &names(&module, &["caller"])), BTreeSet::new());
    assert_eq!(inferred(&module, &module.declarations(), &names(&module, &["caller", "spin"])), names(&module, &["caller", "spin"]));
}

#[test]
fn test_unreachable_stops_a_path_and_one_returning_arm_keeps_a_body_returning() {
    let module = parsed(
        "define void @stops(i1 %c) {
b:
  br i1 %c, label %l, label %r

l:
  unreachable

r:
  unreachable
}

define void @returns(i1 %c) {
b:
  br i1 %c, label %l, label %r

l:
  unreachable

r:
  ret void
}
",
    );
    assert_eq!(inferred(&module, &module.declarations(), &names(&module, &["stops", "returns"])), names(&module, &["stops"]));
}

#[test]
fn test_terminal_call_removes_its_newly_unreachable_successor() {
    let mut module = parsed(
        "@g = global i16 0

declare void @exit()

define i16 @f() {
b0:
  call void @exit()
  %x = add i16 1, 2
  br label %b1

b1:
  store i16 %x, ptr @g
  ret i16 %x
}
",
    );
    let before = run(&module, "f", vec![]);
    assert!(cut(&mut module, "f", "exit"));
    assert_eq!(shape(&module, "f"), [vec!["call", "unreachable"]]);
    assert_eq!(run(&module, "f", vec![]), before, "the call still runs first");
}

#[test]
fn test_terminal_call_cuts_its_same_block_tail() {
    let mut module = parsed(
        "declare void @exit()
declare void @other()

define void @f() {
b0:
  call void @other()
  call void @exit()
  call void @other()
  br label %b1

b1:
  ret void
}
",
    );
    assert!(cut(&mut module, "f", "exit"));
    assert_eq!(shape(&module, "f"), [vec!["call", "call", "unreachable"]]);
}

#[test]
fn test_a_join_other_predecessors_reach_keeps_only_their_phi_inputs() {
    let text = "declare void @exit()

define i16 @f(i1 %c) {
entry:
  br i1 %c, label %a, label %b

a:
  call void @exit()
  br label %join

b:
  br label %join

join:
  %v = phi i16 [ 1, %a ], [ 2, %b ]
  ret i16 %v
}
";
    let mut module = parsed(text);
    assert!(cut(&mut module, "f", "exit"));
    let f = function(&module, "f");
    let phi = f.block(block(f, "join")).instructions()[0];
    assert_eq!(f.instruction(phi).operands.len(), 2, "one input, from %b");
    assert_eq!(run(&module, "f", vec![bit(false)]), Ok(Val::Int { bits: 2, width: 16 }));
    assert_eq!(run(&module, "f", vec![bit(true)]), run(&parsed(text), "f", vec![bit(true)]));
}

#[test]
fn test_an_already_cut_block_is_unchanged() {
    let mut module = parsed(
        "declare void @exit()

define void @f() {
b:
  call void @exit()
  unreachable
}
",
    );
    assert!(!cut(&mut module, "f", "exit"));
}

/// Adapted from `test_qrender_main_spill_uses_shutdown_control_proof`: MAIN
/// crashed reserving two spill bytes, as HOST_SHUTDOWN ends via B$CEND but
/// only a direct runtime exit was known terminal; and after B$CEND the
/// handler still lowered a dead call and return.
#[test]
fn test_a_handler_ending_in_a_runtime_exit_is_noreturn_and_cut() {
    for terminal in [true, false] {
        let attribute = if terminal { " noreturn" } else { "" };
        let mut module = parsed(&format!(
            "declare void @end(){attribute}
declare void @dump()

define void @shutdown() {{
b:
  call void @dump()
  call void @end()
  call void @dump()
  ret void
}}

define void @init(i1 %c) {{
b:
  br i1 %c, label %quit, label %go

quit:
  call void @end()
  br label %go

go:
  ret void
}}

define void @main() {{
b:
  call void @init(i1 false)
  call void @shutdown()
  ret void
}}
"
        ));
        let bodies = names(&module, &["shutdown", "init", "main"]);
        let proven = inferred(&module, &module.declarations(), &bodies);
        assert_eq!(proven, if terminal { names(&module, &["shutdown", "main"]) } else { BTreeSet::new() });

        let declarations = module.declarations();
        let sites = terminal_sites(&module.context, &declarations, function(&module, "shutdown"), &proven);
        let (context, shutdown) = module.function_mut("shutdown").unwrap();
        assert_eq!(after_terminal_calls(context, shutdown, &sites), terminal);
        let expected = if terminal { vec!["call", "call", "unreachable"] } else { vec!["call", "call", "call", "ret"] };
        assert_eq!(shape(&module, "shutdown"), [expected]);
    }
}

#[test]
fn test_blocks_leading_only_to_a_stop_or_a_cold_call_are_cold() {
    let module = parsed(
        "declare void @exit()
declare void @report() cold

define void @f(i1 %c, i1 %d) {
entry:
  br i1 %c, label %fail, label %ok

fail:
  br i1 %d, label %exit, label %trap

exit:
  call void @exit()
  br label %ok

trap:
  unreachable

ok:
  br i1 %d, label %warn, label %done

warn:
  call void @report()
  br label %done

done:
  ret void
}
",
    );
    let f = function(&module, "f");
    let found = cold(&module.context, &module.declarations(), f, &calls_to(&module, "f", "exit"));
    assert_eq!(found, ats(&module, "f", &["fail", "exit", "trap", "warn"]));
}

#[test]
fn test_a_loop_that_never_exits_is_not_cold() {
    let module = parsed(
        "define void @f(i1 %c) {
entry:
  br i1 %c, label %spin, label %done

spin:
  br label %spin

done:
  ret void
}
",
    );
    let f = function(&module, "f");
    assert_eq!(cold(&module.context, &module.declarations(), f, &BTreeSet::new()), BTreeSet::new());
}

#[test]
fn test_a_body_that_always_stops_has_no_cold_block() {
    let module = parsed(
        "declare void @exit()

define void @f(i1 %c) {
entry:
  br i1 %c, label %a, label %b

a:
  call void @exit()
  unreachable

b:
  unreachable
}
",
    );
    let f = function(&module, "f");
    assert_eq!(cold(&module.context, &module.declarations(), f, &calls_to(&module, "f", "exit")), BTreeSet::new());
}

#[test]
fn test_a_block_reaching_neither_return_nor_header_is_stranded() {
    let module = parsed(
        "define void @f(i1 %c, i1 %d) {
entry:
  br label %header

header:
  br i1 %c, label %body, label %after

body:
  br i1 %d, label %header, label %die

die:
  unreachable

after:
  ret void
}
",
    );
    let f = function(&module, "f");
    let header = id(block(f, "header"));
    assert_eq!(stranded(f, header), ats(&module, "f", &["die"]));
}
