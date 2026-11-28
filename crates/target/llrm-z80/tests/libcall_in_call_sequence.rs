//! A struct passed by value is copied into the outgoing arguments inside the
//! call sequence. When the copy is a library call (-Os, more words than the
//! inline limit) it is a nested call: its `ExternalSymbol` callee was left
//! unconverted (`CannotSelect(ExternalSymbol)`, the function REFUSED), it was
//! the C name `memcpy` where the runtime's is `__memcpy`, and inside a variadic
//! call it took that call's state, so the count and source went to the stack.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8";

fn call(variadic: bool) -> String {
    let (callee, declare) = if variadic {
        ("(ptr, ...) @f", "declare i16 @f(ptr byval(%struct.big) align 1, ...)")
    } else {
        ("@f", "declare i16 @f(ptr byval(%struct.big) align 1, i16)")
    };
    format!(
        "target datalayout = \"{DATALAYOUT}\"\n%struct.big = type {{ [20 x i16] }}\n\
         define i16 @cp(ptr %s) {{\n  %r = call i16 {callee}(ptr byval(%struct.big) align 1 %s, i16 7)\n  ret i16 %r\n}}\n{declare}\n"
    )
}

fn os(ir: &str) -> String {
    text(ir, "cp", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"))
}

/// A 40 B copy at -Os: REFUSED with `CannotSelect(ExternalSymbol)`, then
/// (callee converted) `call memcpy`, a symbol the runtime does not have.
#[test]
fn a_byval_copy_inside_a_call_sequence_calls_the_runtimes_memcpy() {
    let body = os(&call(false));
    assert!(body.contains("call __memcpy"), "no runtime memcpy:\n{body}");
    assert!(!body.contains("call memcpy"), "the C name would not link:\n{body}");
}

/// `variadic` -Os reported nothing: the copy's count and source were pushed,
/// only `hl` was set.
#[test]
fn the_copy_inside_a_variadic_call_takes_its_arguments_in_registers() {
    let body = os(&call(true));
    let before_call = body.split("call __memcpy").next().expect("a call of the runtime's memcpy");
    assert!(!before_call.contains("push bc"), "the count was pushed, not passed in bc:\n{body}");
    assert!(before_call.contains("ld bc, #40"), "no count in bc:\n{body}");
}
