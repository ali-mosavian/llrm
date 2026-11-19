//! `foldShiftToAvg` and `combineShiftToAVG` (DEVIATE SD600): the average node
//! is formed only where the target has it. A target without it keeps the add
//! and the shift; the first version formed `avgflooru` at the wide type after
//! types were legal and the legalizer could not expand it
//! ("convert node to libcall", 2026-11-08).

use super::dag_function::selected;

fn names(body: &str) -> Vec<String> {
    let ir = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect()
}

#[test]
fn a_target_without_the_average_keeps_the_add_and_the_shift_of_two_bytes() {
    let seen = names(
        "define i8 @f(i8 %a, i8 %b) {
  %x = zext i8 %a to i32
  %y = zext i8 %b to i32
  %s = add nuw nsw i32 %x, %y
  %h = lshr i32 %s, 1
  %r = trunc i32 %h to i8
  ret i8 %r
}",
    );
    assert!(seen.iter().any(|name| name == "shr"), "{seen:?}");
}

#[test]
fn a_target_without_the_average_keeps_the_add_and_the_shift_of_a_nuw_sum() {
    let seen = names(
        "define i32 @f(i32 %a, i32 %b) {
  %s = add nuw i32 %a, %b
  %h = lshr i32 %s, 1
  ret i32 %h
}",
    );
    assert!(seen.iter().any(|name| name == "shr"), "{seen:?}");
}
