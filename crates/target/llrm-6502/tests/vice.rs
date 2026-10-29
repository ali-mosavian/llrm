//! The C64 PRGs run in VICE's `x64sc`: the top-left screen cell ($0400) holds the program's result.
//! Needs `x64sc`, `xvfb-run` and the C64 ROMs (kernal-901227-03, basic-901226-01, chargen-901225-01)
//! in ~/.local/share/vice/C64, so these are ignored unless asked for:
//! `cargo test -p llrm-6502 --test vice -- --ignored`. One at a time: VICE's monitor is on port 6510.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

#[test]
#[ignore = "needs x64sc, xvfb-run and the C64 ROMs in ~/.local/share/vice/C64"]
fn the_prg_runs_in_vice_and_leaves_7_on_the_screen() {
    let (_, _, prg) = common::build();
    let answers = common::vice(&prg, &["m 0400 0400", "m 080d 0814"]);
    assert!(answers[0].contains(">C:0400  07"), "screen cell: {}", answers[0]);
    // lda #$00; sta $02; lda #$9F; sta $03: the soft stack pointer in rc0:rc1, then `jsr _main`
    assert!(answers[1].contains(">C:080d  a9 00 85 02  a9 9f 85 03"), "code at 2061: {}", answers[1]);
}

/// Compiled from IR: `_main` calls `sum(5)` (a phi loop, 1+2+3+4+5 = 15) and doubles it through the
/// runtime's multiply: 30 = $1E in the screen cell.
#[test]
#[ignore = "needs x64sc, xvfb-run and the C64 ROMs in ~/.local/share/vice/C64"]
fn a_compiled_program_runs_in_vice() {
    let ir = "define i8 @sum(i8 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i8 [0, %entry], [%i2, %loop]\n  %s = phi i8 [0, %entry], [%s2, %loop]\n  %i2 = add i8 %i, 1\n  %s2 = add i8 %s, %i2\n  %c = icmp ult i8 %i2, %n\n  br i1 %c, label %loop, label %done\ndone:\n  ret i8 %s2\n}\n\
define i8 @_main() {\n  %s = call i8 @sum(i8 5)\n  %r = mul i8 %s, 2\n  ret i8 %r\n}";
    let text = format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let code: String = ["sum", "_main"]
        .iter()
        .map(|name| {
            let selection = select_function(&module, name, Cpu::Mos6502).expect("selects");
            function_text(&module, name, &selection).expect("emits")
        })
        .collect();
    let (_, _, prg) = common::link("prg-ir", &code);
    let answers = common::vice(&prg, &["m 0400 0400"]);
    assert!(answers[0].contains(">C:0400  1e"), "screen cell: {}", answers[0]);
}
