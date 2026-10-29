//! IR to assembly text, against the pin's `llc -mtriple=m68k-linux-gnu
//! -mcpu=M68000 -relocation-model=static -O2` (directives and comments
//! dropped).

use llrm_m68k::compile::text;
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn compiled(
    body: &str,
    name: &str,
) -> String {
    let text_in = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text_in).expect("parses");
    text(&module, name, Cpu::M68000).expect("compiles")
}

#[test]
fn an_add_of_two_arguments_prints_as_llc_does() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(compiled(ir, "f"), "f:\n\tmove.l\t(4,%sp), %d0\n\tadd.l\t(8,%sp), %d0\n\trts\n");
}

const CALLS: &str = "declare i32 @g(i32, i8)\ndeclare ptr @h(ptr)\n\
define i32 @c(i32 %a, i8 %b) {\n  %r = call i32 @g(i32 %a, i8 %b)\n  ret i32 %r\n}\n\
define ptr @p(ptr %x) {\n  %r = call ptr @h(ptr %x)\n  ret ptr %r\n}";

#[test]
fn a_call_with_a_byte_argument_prints_as_llc_does() {
    let expected = "c:\n\tsuba.l\t#12, %sp\n\tmoveq\t#0, %d0\n\tmove.b\t(23,%sp), %d0\n\tmove.l\t%d0, (4,%sp)\n\
                    \tmove.l\t(16,%sp), %d0\n\tmove.l\t%d0, (%sp)\n\tjsr\tg\n\tadda.l\t#12, %sp\n\trts\n";
    assert_eq!(compiled(CALLS, "c"), expected);
}

#[test]
fn a_pointer_call_prints_as_llc_does() {
    let expected =
        "p:\n\tsuba.l\t#4, %sp\n\tmove.l\t(8,%sp), %d0\n\tmove.l\t%d0, (%sp)\n\tjsr\th\n\tadda.l\t#4, %sp\n\trts\n";
    assert_eq!(compiled(CALLS, "p"), expected);
}
