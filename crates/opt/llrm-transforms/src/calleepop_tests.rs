use llrm_mir::passes::{ModuleAnalyses, ModulePass};

use crate::calleepop::CalleePop;
use crate::testing::{parsed, printed};

/// `text` after the pass.
fn run(text: &str) -> String {
    run_for(text, true)
}

/// `text` after the pass, priced in bytes (`size`) or in clocks, on the real-mode target.
fn run_for(text: &str, size: bool) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    let mut analyses = ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
    CalleePop { size }.run(&mut module, &mut analyses);
    printed(&module)
}

const CALLEE: &str = "define internal i16 @work(i16 %a, i16 %b, i16 %c) {
b0:
  %x = add i16 %a, %b
  %y = add i16 %x, %c
  ret i16 %y
}
";

/// QCport -Os spent `add sp,N` at some 720 calls of internal functions: 3 bytes each, where
/// the callee popping them is `ret N` once. The function and each call of it take one
/// convention, so the call's cleanup and the function's `ret N` cannot disagree.
#[test]
fn an_internal_function_called_directly_twice_pops_its_own_arguments() {
    let after = run(&format!("{CALLEE}define i16 @f(i16 %x) {{
b0:
  %p = call i16 @work(i16 %x, i16 2, i16 3)
  %q = call i16 @work(i16 3, i16 %x, i16 1)
  %s = add i16 %p, %q
  ret i16 %s
}}
"));
    assert!(after.contains("define internal fastcc i16 @work"), "{after}");
    assert_eq!(after.matches("call fastcc i16 @work").count(), 2, "{after}");
}

/// Where a caller outside the module's sight, or an indirect one, may reach it, the callee
/// popping would unbalance that caller's stack: an external function, one whose address is
/// stored or passed, one called with another convention, and a variadic one stay as they are.
#[test]
fn a_function_something_else_may_call_keeps_the_caller_cleaning() {
    let caller = "define i16 @f(i16 %x) {
b0:
  %p = call i16 @work(i16 %x, i16 2, i16 3)
  %q = call i16 @work(i16 3, i16 %x, i16 1)
  %s = add i16 %p, %q
  ret i16 %s
}
";
    let external = run(&format!("{}{caller}", CALLEE.replace("internal ", "")));
    assert!(!external.contains("fastcc"), "{external}");
    let taken = run(&format!("@table = global ptr @work\n{CALLEE}{caller}"));
    assert!(!taken.contains("fastcc"), "{taken}");
    let passed = run(&format!("declare void @take(ptr)\n{CALLEE}{caller}define void @g() {{\nb0:\n  call void @take(ptr @work)\n  ret void\n}}\n"));
    assert!(!passed.contains("fastcc"), "{passed}");
}

/// `ret N` costs 2 bytes more than `ret` at each return, and a one-word call saves 1 (a pop's worth
/// is not worth an instruction more, so a byte over breaks even): one call of a function that returns
/// twice keeps the caller's cleanup in bytes; in clocks `ret N` costs nothing and the call saves its
/// `add sp`, so -O2 takes it.
#[test]
fn a_function_the_convention_would_cost_bytes_keeps_the_caller_cleaning_only_for_size() {
    let text = "define internal i16 @work(i16 %a) {
b0:
  %t = icmp eq i16 %a, 0
  br i1 %t, label %one, label %two
one:
  ret i16 1
two:
  ret i16 2
}

define i16 @f(i16 %x) {
b0:
  %p = call i16 @work(i16 %x)
  ret i16 %p
}
";
    assert!(!run_for(text, true).contains("fastcc"));
    assert!(run_for(text, false).contains("define internal fastcc i16 @work"));
}

/// What a call saves is the words its arguments take, not how many there are: a dword argument is
/// two words (two pops), so two calls of a function with two returns pay for `ret 4` where two
/// calls of a one-word function do not.
#[test]
fn an_argument_of_two_words_counts_two_words_of_cleanup() {
    let callee = |ty: &str| format!("define internal i16 @work({ty} %a) {{\nb0:\n  %t = icmp eq {ty} %a, 0\n  br i1 %t, label %one, label %two\none:\n  ret i16 1\ntwo:\n  ret i16 2\n}}\n");
    let callers = |ty: &str, value: &str| format!("define void @f() {{\nb0:\n  %p = call i16 @work({ty} {value})\n  %q = call i16 @work({ty} {value})\n  ret void\n}}\n");
    assert!(run(&format!("{}{}", callee("i32"), callers("i32", "1"))).contains("fastcc"));
    assert!(!run(&format!("{}{}", callee("i16"), callers("i16", "1"))).contains("fastcc"));
}
