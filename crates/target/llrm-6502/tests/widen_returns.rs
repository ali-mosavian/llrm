//! `report(f(x))` of a private function that returns a short: every call paid
//! the zero high word of the long it passes (`ldy #0; sty rc2; sty rc3`, 6 B),
//! bit_setclear's four calls 24 B (151 B against a bar of 147). A private
//! function whose callers all extend its result returns the long, extended
//! once where it returns (`DEVIATE WIDEN-RETURNS`).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// Four calls of two private functions, each result extended to a long for
/// `report`.
fn calls() -> String {
    let mut ir = format!(
        "target datalayout = \"{}\"\ndeclare void @report(i32)\ndeclare i16 @src()\n\
         define internal fastcc i16 @set_bit(i16 %n) noinline {{\n  %a = shl i16 1, %n\n  %b = or i16 %a, 1056\n  ret i16 %b\n}}\n\
         define internal fastcc i16 @clear_bit(i16 %n) noinline {{\n  %a = shl i16 1, %n\n  %m = xor i16 %a, -1\n  %b = and i16 %m, 1056\n  ret i16 %b\n}}\n\
         define i16 @main() {{\n",
        datalayout()
    );
    for (at, callee) in ["set_bit", "set_bit", "clear_bit", "clear_bit"].iter().enumerate() {
        ir += &format!(
            "  %s{at} = call i16 @src()\n  %r{at} = call fastcc i16 @{callee}(i16 %s{at})\n  %w{at} = zext i16 %r{at} to i32\n  call void @report(i32 %w{at})\n"
        );
    }
    ir + "  ret i16 0\n}\n"
}

/// The cells the high word of the long argument is stored in, before a call of
/// `report`, in `main`.
fn high_word_stores(text: &str) -> usize {
    let main = text.split("main:").nth(1).expect("main").split(".segment").next().unwrap();
    main.lines().map(str::trim).filter(|line| line.ends_with(" rc2") || line.ends_with(" rc3")).count()
}

/// One test, as it sets the kill switch of the process: each call of the two
/// functions stores the zero high word with the switch off, none with it on.
#[test]
fn the_zero_high_word_is_made_once_in_the_private_function() {
    let ir = calls();
    // SAFETY: this binary has one test; nothing else reads the environment
    // meanwhile
    unsafe { std::env::set_var("LLRM_6502_WIDEN_RETURNS", "0") };
    let narrow = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    unsafe { std::env::remove_var("LLRM_6502_WIDEN_RETURNS") };
    let wide = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(high_word_stores(&narrow) >= 4, "the zero high word is stored at each call:\n{narrow}");
    assert_eq!(high_word_stores(&wide), 0, "main stores no high word:\n{wide}");
}
