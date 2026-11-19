//! The pin's `RegisterCoalescer` leaves a function that calls `setjmp` alone
//! (`exposesReturnsTwice`). Here m68k did not tell the shared coalescer, so
//! the loop's phi copies were joined as in any other function.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

fn moves(setjmp_attrs: &str) -> usize {
    let ir = format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %n, ptr %buf) {{
entry:
  %r = call i32 @setjmp(ptr %buf)
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i32 [ %r, %entry ], [ %s2, %loop ]
  %s2 = add i32 %s, %i
  %i2 = add i32 %i, 1
  %c = icmp ult i32 %i2, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s2
}}
declare i32 @setjmp(ptr) {setjmp_attrs}
"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let text = text_with(&module, "f", Cpu::M68000, Options::PINNED).expect("compiles");
    text.lines().filter(|line| line.trim_start().starts_with("move")).count()
}

#[test]
fn a_function_that_calls_setjmp_keeps_its_copies() {
    assert!(moves("returns_twice") > moves(""), "{} moves with setjmp, {} without", moves("returns_twice"), moves(""));
}
