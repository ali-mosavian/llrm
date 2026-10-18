//! What `infer` stamps, against `FunctionAttrs.cpp`'s rules.

use llrm_mir::module::{GlobalValue, Module};

use super::*;
use crate::testing::parsed;

fn declarations(module: &Module) -> Vec<GlobalValue> {
    module.globals.iter().map(GlobalValue::declaration).collect()
}

/// What `infer` says of the SCC `names`.
fn inferred(
    text: &str,
    names: &[&str],
) -> Vec<Inferred> {
    let module = parsed(text);
    let members: Vec<GlobalId> = names.iter().map(|one| module.named(one).expect("a global")).collect();
    infer(&module, &declarations(&module), &members).into_iter().map(|(_, one)| one).collect()
}

fn all_of(
    no_recurse: bool,
    no_unwind: bool,
    will_return: bool,
    no_return: bool,
) -> Inferred {
    Inferred { no_recurse, no_unwind, will_return, no_return }
}

#[test]
fn test_a_body_over_norecurse_callees_is_stamped_all_three() {
    let text = "declare i32 @leaf(i32) norecurse nounwind willreturn

define i32 @f(i32 %x) {
b0:
  %a = call i32 @leaf(i32 %x)
  %b = add i32 %a, 1
  ret i32 %b
}
";
    assert_eq!(inferred(text, &["f"]), [all_of(true, true, true, false)]);
}

/// A call to a body nothing states leaves the function recursive, throwing and
/// not returning; a call to itself leaves it recursive, but a call to an SCC
/// member does not break nounwind.
#[test]
fn test_an_unstated_callee_and_a_self_call_leave_nothing_stamped() {
    let text = "declare void @unknown()

define void @calls_unknown() {
b0:
  call void @unknown()
  ret void
}

define void @itself() {
b0:
  call void @itself()
  ret void
}
";
    assert_eq!(inferred(text, &["calls_unknown"]), [all_of(false, false, false, false)]);
    assert_eq!(inferred(text, &["itself"]), [all_of(false, true, false, false)]);
}

/// Mutual recursion: norecurse needs a one-member SCC; nounwind is the whole
/// SCC's, so one throwing instruction takes it from both.
#[test]
fn test_nounwind_is_all_or_none_of_an_scc_and_norecurse_needs_one_member() {
    let quiet = "define void @a() {
b0:
  call void @b()
  ret void
}

define void @b() {
b0:
  call void @a()
  ret void
}
";
    assert_eq!(inferred(quiet, &["a", "b"]), [all_of(false, true, false, false), all_of(false, true, false, false)]);
    let throwing = "declare void @unknown()

define void @a() {
b0:
  call void @b()
  ret void
}

define void @b() {
b0:
  call void @a()
  call void @unknown()
  ret void
}
";
    assert_eq!(
        inferred(throwing, &["a", "b"]),
        [all_of(false, false, false, false), all_of(false, false, false, false)]
    );
}

/// A loop may not come back; with `mustprogress` and no writes it does.
#[test]
fn test_a_loop_returns_only_by_mustprogress_without_writes() {
    let body = "define void @spin(i1 %c) {
b0:
  br label %loop

loop:
  br i1 %c, label %loop, label %done

done:
  ret void
}
";
    assert!(!inferred(body, &["spin"])[0].will_return);
    let promised = body.replace("define void @spin(i1 %c) {", "define void @spin(i1 %c) mustprogress memory(read) {");
    assert!(inferred(&promised, &["spin"])[0].will_return);
}

/// Only a path to `ret` that no `noreturn` call cuts makes the body return.
#[test]
fn test_a_body_with_no_path_to_a_ret_past_a_noreturn_call_is_noreturn() {
    let text = "declare void @exit() noreturn

define void @stops() {
b0:
  call void @exit()
  ret void
}

define void @returns(i1 %c) {
b0:
  br i1 %c, label %quit, label %go

quit:
  call void @exit()
  ret void

go:
  ret void
}
";
    assert!(inferred(text, &["stops"])[0].no_return);
    assert!(!inferred(text, &["returns"])[0].no_return);
}

/// An `invoke` may unwind whatever its callee states.
#[test]
fn test_an_invoke_takes_nounwind_away() {
    let text = "declare void @leaf() nounwind

define void @f() personality ptr null {
b0:
  invoke void @leaf() to label %ok unwind label %bad

ok:
  ret void

bad:
  %p = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %p
}
";
    assert!(!inferred(text, &["f"])[0].no_unwind);
}
