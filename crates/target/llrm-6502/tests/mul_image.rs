//! A multiply by a constant in a function tuned for size is a call of
//! `__mulsi3` only while the routine is linked for another reason.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

fn asm(functions: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{functions}", datalayout());
    assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"))
}

const BY_THREE: &str = "define i32 @by_three(i32 %x) minsize nounwind {\n  %r = mul i32 %x, 3\n  ret i32 %r\n}\n";
const BY_ARGUMENT: &str =
    "define i32 @by_argument(i32 %x, i32 %y) minsize nounwind {\n  %r = mul i32 %x, %y\n  ret i32 %r\n}\n";

/// `state * 3` at -Os called the 85-byte routine and its 25-byte tail for
/// 3.34 M clocks where the 82-byte shifts and add take a tenth of that
/// (licm_graph_callee_footprint: 467 B against gcc's 296).
#[test]
fn a_constant_multiply_alone_is_shifts_and_an_add() {
    let text = asm(BY_THREE);
    assert!(!text.contains("__mulsi3"), "{text}");
}

/// The routine is linked anyway: the longer expansion would only add bytes.
#[test]
fn a_constant_multiply_beside_a_variable_one_stays_a_call() {
    let text = asm(&format!("{BY_THREE}{BY_ARGUMENT}"));
    let by_three = &text[text.find("by_three:").unwrap()..text.find("by_argument:").unwrap_or(text.len())];
    assert!(by_three.contains("__mulsi3"), "{text}");
}

/// The price is read from the runtime, for any routine: a long division and a
/// variable shift are priced the same way as the multiply.
#[test]
fn a_routine_is_priced_from_the_runtime() {
    let price = llrm_6502::mulimage::library_bytes;
    assert!((100..=120).contains(&price("__mulsi3")), "{}", price("__mulsi3"));
    assert!(price("__udivsi3") > price("__mulsi3"), "{} {}", price("__udivsi3"), price("__mulsi3"));
    assert!(price("__ashlsi3") > 0);
}

/// `(unsigned)x * 40` was `(x << 5) + (x << 3)`, eight two-byte shift steps
/// (53 B at -O2); the sum `x * 5` shifted by three is five (43 B).
#[test]
fn a_constant_with_factors_of_two_shifts_the_sum_once() {
    let ir =
        "define i16 @k_mul40(i8 %x) nounwind {\n  %w = zext i8 %x to i16\n  %r = mul i16 %w, 40\n  ret i16 %r\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_mul40:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    let bytes = estimate(Cpu::Mos6502, body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 43, "{bytes} B\n{body}");
}

/// `y * 20` beside `y * 16`: the pin's `(y << 4) + (y << 2)` shares the
/// `y << 4`; the sum of `y * 5` shifted by two (two steps fewer alone) made it
/// again, 8 shift steps against 6.
#[test]
fn a_shift_the_function_has_is_not_made_again_by_a_factored_multiply() {
    let ir = "define i16 @k_pair(ptr %p, ptr %q, i8 %y) nounwind {\n  %w = zext i8 %y to i16\n  %a = mul i16 %w, 16\n  %b = mul i16 %w, 20\n  store i16 %a, ptr %p\n  store i16 %b, ptr %q\n  ret i16 0\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_pair:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    let steps = body.matches("asl").count();
    assert!(steps <= 6, "{steps} shift steps\n{body}");
}

/// `(unsigned)x * 20` shifted the first pair in a copy of `x` (`asl rc4; rol
/// rc3`, twice) and loaded it for the add: 40 B at -O2. The pair shifts in `a`
/// when only a carry op stands between it and the load.
#[test]
fn a_shift_chain_before_an_add_runs_in_a() {
    let ir =
        "define i16 @k_mul20(i8 %x) nounwind {\n  %w = zext i8 %x to i16\n  %r = mul i16 %w, 20\n  ret i16 %r\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("k_mul20:").unwrap_or_else(|| panic!("{text}"));
    let body = text[from..].split(".segment").next().unwrap();
    let bytes = estimate(Cpu::Mos6502, body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 34, "{bytes} B\n{body}");
}

/// A loop of `x * 31` beside a multiply by a variable: the routine is linked
/// anyway, but the loop ran 2,000+ clocks a trip in calls. At -Os the module
/// takes the shifts when the clocks they save are worth the bytes they add
/// (copy_runs 70,059 -> 57,967 clocks and 1,362 -> 1,342 B; mul_constants
/// 19.4M -> 15.4M, llvm-mos 13.6M): the call stays only for the variable one.
#[test]
fn a_constant_multiply_in_a_loop_is_shifts_though_a_variable_multiply_is_linked() {
    let text = asm("define i32 @hot(i32 %x, i32 %y, i32 %n) minsize nounwind {\n\
         entry:\n  br label %loop\n\
         loop:\n  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n  %a = phi i32 [ %x, %entry ], [ %a1, %loop ]\n\
           %a1 = mul i32 %a, 31\n  %i1 = add i32 %i, 1\n  %c = icmp ult i32 %i1, %n\n  br i1 %c, label %loop, label %exit\n\
         exit:\n  %r = mul i32 %a1, %y\n  ret i32 %r\n}\n");
    let calls = text.matches("__mulsi3").count();
    assert_eq!(calls, 1, "{text}");
}

/// The same multiply run once, in a module whose clocks are somewhere else (a
/// loop of variable multiplies): the share of the module's clocks it saves does
/// not pay its bytes, the call stays.
#[test]
fn a_constant_multiply_run_once_beside_a_busy_loop_stays_a_call() {
    let text = asm("define i32 @busy(i32 %x, i32 %y, i32 %n) minsize nounwind {\n\
         entry:\n  br label %loop\n\
         loop:\n  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n  %a = phi i32 [ %x, %entry ], [ %a1, %loop ]\n\
           %a1 = mul i32 %a, %y\n  %i1 = add i32 %i, 1\n  %c = icmp ult i32 %i1, %n\n  br i1 %c, label %loop, label %exit\n\
         exit:\n  ret i32 %a1\n}\n\
         define i32 @cold(i32 %x) minsize nounwind {\n  %a = mul i32 %x, 31\n  ret i32 %a\n}\n");
    let cold = &text[text.find("cold:").unwrap_or_else(|| panic!("{text}"))..];
    assert!(cold.split(".segment").next().unwrap().contains("__mulsi3"), "{text}");
}
