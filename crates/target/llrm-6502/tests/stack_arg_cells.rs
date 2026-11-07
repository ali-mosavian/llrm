//! A function with a long stack argument names only cells that exist.

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8";

/// `pass32` copied a register that the allocator numbers below `rc0` into a
/// spill slot (`lda rc18446744073709551603`: a cell name from `register_name`
/// minus the first cell, unsigned), and cl65 stopped on the undefined symbol.
/// `ir::a_call_with_a_stack_argument_runs` was the first to see it, after a
/// commit outside the 6502 changed the registers a stack argument arrives in.
#[test]
fn a_call_with_a_stack_argument_names_only_real_cells() {
    let ir = format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         define i32 @add32(i32 %a, i32 %b) {{\n  %r = add i32 %a, %b\n  ret i32 %r\n}}\n\
         define i32 @pass32(i32 %a, i32 %b) {{\n  %r = call i32 @add32(i32 %a, i32 %b)\n  ret i32 %r\n}}"
    );
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let selection = select_function(&module, "pass32", Cpu::Mos6502).expect("selects");
    let text = function_text(&module, "pass32", &selection).expect("emits");
    let bad: Vec<&str> = text.lines().filter(|line| line.contains("rc1844")).collect();
    assert!(bad.is_empty(), "{bad:?}\n{text}");
}
