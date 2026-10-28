//! `WidenIV` through `indvars`: a counter read through `sext` or `zext` is a
//! counter of the extension's width.

use crate::indvars::IndVars;
use crate::testing::{managed, parsed, printed, results};

/// `for (i = 0; i < n; i++) buf[ext i] = 1` with `i` of 16 bits and the
/// extension to 32.
fn clearing(
    extension: &str,
    predicate: &str,
    start: &str,
    step: &str,
) -> String {
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
    managed(&mut module, IndVars { lftr: true, size: false });
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
    // LFTR then tests the wide counter against its exit count with `ne`.
    assert!(after.contains("icmp ne i32"), "{after}");
    assert!(!after.contains("sext i16 %i to i32"), "{after}");
}

#[test]
fn test_a_signed_counter_from_a_negative_start_is_widened_from_the_negative_number() {
    let after = widened(&clearing("sext", "slt", "-5", "nsw"));
    assert!(after.contains("[ -5, %b0 ]"), "{after}");
}

#[test]
fn test_an_unsigned_counter_read_through_sext_is_not_widened() {
    let after = widened(&clearing("sext", "ult", "0", ""));
    assert!(!after.contains("phi i32"), "{after}");
}

/// The 16-bit target's X86 model and `n8:16:32` call `i32` legal: a counter is
/// not widened past the pointer's 16 bits there (D18), where its `i32` add
/// costs two.
#[test]
fn test_a_counter_is_not_widened_past_the_pointer_width_on_the_16_bit_target() {
    let text = clearing("zext", "ult", "0", "nsw").replace("e-p:32:32-n8:16:32", "e-p:16:16-n8:16:32");
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { lftr: true, size: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    let after = printed(&module);
    assert!(!after.contains("phi i32"), "{after}");
}

/// A user of the counter with no value (a `store` of it) is a use to truncate
/// for. `widen_iv_use` asked every user for its value first: `bench/bintree`
/// at -O2 -m32 panicked 'an instruction with a value' (#2531).
#[test]
fn test_a_counter_a_store_reads_is_widened_and_the_store_reads_a_truncation() {
    let text = "target datalayout = \"e-p:32:32-n8:16:32\"

@g = global i16 0

define i32 @f(i16 %n) {
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  store i16 %i, ptr @g
  %w = sext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add nsw i16 %i, 1
  br label %b1

b3:
  ret i32 0
}
";
    let after = widened(text);
    assert!(after.contains("phi i32"), "{after}");
    assert!(after.contains("= trunc nuw nsw i32"), "{after}");
}

/// histo-O2 `pipeline.body`: the `int` index `%i` and the `short` counter
/// `%j` the loop tests run side by side; `%i` is `sext` of `%j`'s next value.
/// The narrow counter stayed, with the `sext` and an `slt i16` test, where
/// `opt` has one `i32` counter and `icmp ne i32 %i.next, 4096`.
#[test]
fn test_a_narrow_counter_beside_its_own_sign_extension_is_not_kept() {
    let after = widened(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f() {
b0:
  %buf = alloca [4096 x i8]
  br label %b1

b1:
  %i = phi i32 [ %w, %b1 ], [ 0, %b0 ]
  %j = phi i16 [ %next, %b1 ], [ 0, %b0 ]
  %p = getelementptr inbounds i8, ptr %buf, i32 %i
  store i8 1, ptr %p
  %next = add i16 %j, 1
  %w = sext i16 %next to i32
  %go = icmp slt i16 %next, 4096
  br i1 %go, label %b1, label %b2

b2:
  ret i32 0
}
",
    );
    assert!(!after.contains("phi i16"), "{after}");
    assert!(!after.contains("sext i16"), "{after}");
}

/// induction32: the narrow increment feeds the phi and the exit test, so the
/// wide add was cloned for each; LFTR then found no counter and the exit test
/// stayed `icmp ult i32` beside two identical adds.
#[test]
fn test_one_wide_increment_serves_the_phi_and_the_exit_test() {
    let after = widened(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 zeroext %a) {
b0:
  %buf = alloca [64 x i8]
  %s = add i16 %a, 1
  %go = icmp ult i16 %s, 4
  br i1 %go, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i16 [ %s, %pre ], [ %next, %b1 ]
  %w = zext nneg i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  %c = icmp ult i16 %next, 4
  br i1 %c, label %b1, label %out

out:
  br label %b3

b3:
  ret i32 0
}
",
    );
    assert_eq!(after.matches("add nuw nsw i32").count(), 1, "{after}");
    assert!(after.contains("icmp ne i16"), "the exit test is replaced: {after}");
}
