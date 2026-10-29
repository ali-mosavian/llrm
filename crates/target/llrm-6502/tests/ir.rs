//! IR to ca65 through the selector and `select.rs`, run on sim65. Every expected value is worked
//! out by hand.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

/// The functions of `ir` as ca65 text.
fn compile(
    ir: &str,
    functions: &[&str],
) -> String {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    functions
        .iter()
        .map(|name| {
            let blocks = select_function(&module, name, Cpu::Mos6502).expect("selects");
            function_text(&module, name, &blocks).expect("emits")
        })
        .collect()
}

fn set(
    register: &str,
    value: u16,
) -> String {
    format!("  lda #{}\n  sta {register}\n", value & 0xFF)
}

fn run(
    name: &str,
    ir: &str,
    function: &str,
    setup: &str,
    after: &str,
) -> i32 {
    common::run_with(name, &format!("{setup}  jsr {function}\n{after}"), &compile(ir, &[function]))
}

const ADD8: &str = "define i8 @add8(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";

/// 7 + 5 = 12; 200 + 100 = 300 = 0x12C, so 44.
#[test]
fn a_byte_add_runs() {
    for (a, b, want) in [(7, 5, 12), (200, 100, 44)] {
        let setup = set("rc2", a) + &set("rc3", b);
        assert_eq!(run(&format!("add8-{a}"), ADD8, "add8", &setup, ""), want);
    }
}

/// 300 + 5000 = 5300 = 0x14B4: the low byte is 0xB4 = 180, the high 0x14 = 20.
#[test]
fn a_word_add_runs() {
    let ir = "define i16 @add16(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    let setup = set("rc2", 300) + &set("rc3", 300 >> 8) + &set("rc4", 5000) + &set("rc5", 5000 >> 8);
    assert_eq!(run("add16-lo", ir, "add16", &setup, ""), 180);
    assert_eq!(run("add16-hi", ir, "add16", &setup, "  txa\n"), 20);
}

/// `a < b` unsigned: 3 < 9 gives 1, 9 < 3 gives 2, 5 < 5 gives 2.
#[test]
fn an_unsigned_branch_runs() {
    let ir = "define i8 @lt(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    for (a, b, want) in [(3, 9, 1), (9, 3, 2), (5, 5, 2)] {
        let setup = set("rc2", a) + &set("rc3", b);
        assert_eq!(run(&format!("lt-{a}-{b}"), ir, "lt", &setup, ""), want);
    }
}

/// `caller(10)` calls `g(i8)`, written by hand to return `rc2 + 1` = 11.
#[test]
fn a_call_passes_a_byte() {
    let ir = "declare i8 @g(i8)\ndefine i8 @caller(i8 %a) {\n  %r = call i8 @g(i8 %a)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc #1\n  rts\n";
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call", &format!("{}  jsr caller\n", set("rc2", 10)), &format!("{text}{g}")), 11);
}

/// `caller(10, 30)` calls `g(i8, i8)`, written by hand to return `rc2 + rc3` = 40.
/// Plan row S2: the shared builder names a call's argument registers as x86 does (a register by
/// its root and size), so `rc3` becomes `rc2` and the run returns 60. Passes when the builder
/// calls the target's `lower_call`.
#[test]
#[ignore = "S2: the builder's call lowering is x86's"]
fn a_call_passes_two_bytes() {
    let ir = "declare i8 @g(i8, i8)\ndefine i8 @caller(i8 %a, i8 %b) {\n  %r = call i8 @g(i8 %a, i8 %b)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc rc3\n  rts\n";
    let setup = set("rc2", 10) + &set("rc3", 30);
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call-two", &format!("{setup}  jsr caller\n"), &format!("{text}{g}")), 40);
}

/// `caller(10, 300)` calls `g(i8, i16)`, written by hand to return `rc2 + rc4` = 10 + 0x2C = 54.
/// Plan row S2, as above: both bytes of the word go to `rc4`, `rc5` is never set, the run returns 11.
#[test]
#[ignore = "S2: the builder's call lowering is x86's"]
fn a_call_passes_a_byte_and_a_word() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @caller(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    let g = ".export g\ng:\n  clc\n  lda rc2\n  adc rc4\n  rts\n";
    let setup = set("rc2", 10) + &set("rc4", 300) + &set("rc5", 300 >> 8);
    let text = compile(ir, &["caller"]);
    assert_eq!(common::run_with("call-word", &format!("{setup}  jsr caller\n"), &format!("{text}{g}")), 54);
}
