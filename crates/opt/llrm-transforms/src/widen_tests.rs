//! `WidenIV` through `indvars`: a counter read through `sext` or `zext` is a
//! counter of the extension's width.

use crate::indvars::IndVars;
use crate::testing::{managed, parsed, printed, results};

/// `for (i = 0; i < n; i++) buf[ext i] = 1` with `i` of 16 bits and the
/// extension to 32.
fn clearing(extension: &str, predicate: &str, start: &str, step: &str) -> String {
    format!(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {{
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %go = icmp {predicate} i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = {extension} i16 %i to i32
  %v = add i32 %w, 8
  %p = getelementptr inbounds i8, ptr %buf, i32 %v
  store i8 1, ptr %p
  %next = add {step} i16 %i, 1
  br label %b1

b3:
  ret i32 0
}}
"
    )
}

/// @f of `text` through `indvars` (with the exit values), printed, and
/// computing what it did on `inputs`.
fn widened(text: &str) -> String {
    let mut module = parsed(text);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[40]];
    let before = results(&module, inputs);
    managed(&mut module, IndVars { exit_values: true, unrolls: None });
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

#[test]
fn test_a_counter_read_through_zext_is_a_counter_of_that_width() {
    let after = widened(&clearing("zext", "ult", "0", "nsw"));
    eprintln!("{after}");
    assert!(after.contains("phi i32"), "{after}");
    assert!(!after.contains("phi i16"), "the narrow counter is gone: {after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

#[test]
fn test_a_signed_counter_read_through_sext_is_a_counter_of_that_width() {
    let after = widened(&clearing("sext", "slt", "0", "nsw"));
    assert!(after.contains("phi i32"), "{after}");
    assert!(after.contains("icmp slt i32"), "{after}");
    assert!(!after.contains("sext i16 %i to i32"), "{after}");
}

#[test]
fn test_a_signed_counter_from_a_negative_start_is_widened_from_the_negative_number() {
    let after = widened(&clearing("sext", "slt", "-5", "nsw"));
    assert!(after.contains("phi i32 [ -5, %b0 ]"), "{after}");
}

#[test]
fn test_an_unsigned_counter_read_through_sext_is_not_widened() {
    let after = widened(&clearing("sext", "ult", "0", ""));
    assert!(!after.contains("phi i32"), "{after}");
}

#[test]
fn tmp_dos() {
    let mut module = parsed(&clearing("zext", "ult", "0", "nsw"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { exit_values: true, unrolls: None });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    eprintln!("DOS: {}", printed(&module));
    let m32 = llrm_x86_m16::Dos::default();
    eprintln!("tti: {}", llrm_mir::target::Machine::tti(&m32).is_some());
}
