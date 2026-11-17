//! Where an argument travels. A function private to its module (`fastcc`) takes up to 18 bytes in `a`, `x`, `rc2`..`rc7`,
//! `rc16`..`rc25`; one other modules can call keeps 8 bytes in registers and the rest on the soft stack, where the runtime's
//! members read theirs (`calling.toml`: `[mos]` and `[mos_private]`).

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn at(
    level: Level,
    ir: &str,
) -> String {
    let text = format!("target datalayout = \"{}\"\n{ir}", Mos.layout().datalayout);
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"))
}

/// The lines of the function `name`.
fn function<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let from = text.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let rest = &text[from + 1..];
    let end = rest.find("\n.segment").unwrap_or(rest.len());
    &rest[..end]
}

const PRIVATE: &str = "declare void @report(i32)

define internal fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d) noinline {
  %1 = mul i32 %b, 3
  %2 = add i32 %a, %1
  %3 = mul i32 %c, 5
  %4 = add i32 %2, %3
  %5 = mul i32 %d, 7
  %6 = add i32 %4, %5
  ret i32 %6
}

define i32 @call4(i32 %a, i32 %b, i32 %c) {
  %d = add i32 %a, %b
  %r = call fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  %s = add i32 %r, %c
  ret i32 %s
}

define i32 @same(i32 %a, i32 %b, i32 %c, i32 %d) {
  %r = tail call fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  ret i32 %r
}
";

const EXTERNAL: &str = "define i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d) noinline {
  %1 = mul i32 %b, 3
  %2 = add i32 %a, %1
  %3 = add i32 %c, %d
  %4 = add i32 %2, %3
  ret i32 %4
}

define i32 @call4(i32 %a, i32 %b, i32 %c) {
  %d = add i32 %a, %b
  %r = call i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  ret i32 %r
}
";

/// frameless (+235 B against llvm-mos at O1) and tail_calls (+190 B): the 16 bytes of `add4(a, b, c, d)` were 8 in
/// registers and 8 stored through `(rc0),y` at the call and loaded back by the callee, a frame at every call.
#[test]
fn a_private_function_takes_sixteen_argument_bytes_in_registers() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, PRIVATE);
        for name in ["add4", "call4"] {
            let body = function(&text, name);
            assert!(!body.contains("(rc0),y"), "{level:?} {name} reads or writes the soft stack\n{body}");
        }
        assert!(function(&text, "call4").contains("rc16"), "{level:?}: the third argument is in rc16..\n{text}");
    }
}

/// tail_calls `same`: `jsr __outl_1; jsr __outl_4; jsr add3; jmp __outl_2` (12 B) for a call that is `jmp add3`.
#[test]
fn a_sibling_call_with_more_than_eight_argument_bytes_is_a_jump() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, PRIVATE);
        let body = function(&text, "same");
        assert!(body.contains("jmp add4"), "{level:?}\n{body}");
        assert!(!body.contains("jsr add4"), "{level:?}\n{body}");
    }
}

/// Negative: a function other modules can call keeps the placement the runtime's members read: 8 bytes in registers, the
/// rest on the soft stack.
#[test]
fn an_external_function_keeps_its_arguments_beyond_eight_bytes_on_the_stack() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, EXTERNAL);
        let caller = function(&text, "call4");
        assert!(caller.contains("(rc0),y") || caller.contains("),y"), "{level:?}: the call stores its stack arguments\n{caller}");
        assert!(!caller.contains("rc16"), "{level:?}: no argument goes in rc16..\n{caller}");
    }
}
