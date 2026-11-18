//! A function that calls itself twice keeps the plain recursion where it is
//! smaller than the loop tail-recursion elimination makes of it.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const FIB: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
    define i16 @fib(i16 %n) {\n  %c = icmp slt i16 %n, 2\n  br i1 %c, label %base, label %rec\n\
    base:\n  ret i16 %n\nrec:\n  %a = sub i16 %n, 1\n  %x = call i16 @fib(i16 %a)\n  %b = sub i16 %n, 2\n\
    %y = call i16 @fib(i16 %b)\n  %s = add i16 %x, %y\n  ret i16 %s\n}\n";

fn fib(level: Level) -> String {
    let request = Request::new(level);
    let prepared = prepare(FIB, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "fib").expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

/// `fib` at -Os was a loop carrying the accumulator and the argument across the
/// call, spilled through `iy` each trip: 62 B and 3.5M T against 31 B for the
/// recursion (bench fib 96 B, SDCC 54 B).
#[test]
fn fib_stays_a_recursion_at_os() {
    let text = fib(Level::Os);
    let instructions = text.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(!text.contains("iy") && instructions <= 25, "{instructions} instructions:\n{text}");
}
