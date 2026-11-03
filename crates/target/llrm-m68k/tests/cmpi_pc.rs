//! `cmpi #imm, (sym+n,%pc)` was selected for a compare of a loaded global byte
//! with a constant; the 68000 has no PC-relative form of `cmpi`, so binutils
//! rejected the output ("operands mismatch"). The 68000 loads the byte first;
//! the 68020 keeps the one instruction.

use std::process::Command;

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const COMPARE: &str = "
@buf = external constant [16 x i8], align 1
define i32 @f(i32 %a, i32 %b) {
  %v = load i8, ptr getelementptr (i8, ptr @buf, i32 9), align 1
  %c = icmp eq i8 %v, -1
  %r = select i1 %c, i32 %a, i32 %b
  ret i32 %r
}";

fn text(cpu: Cpu) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{COMPARE}")).expect("parses");
    text_with(&module, "f", cpu, Options::default()).expect("compiles")
}

fn has_cmpi_pc(text: &str) -> bool {
    text.lines().any(|line| line.trim_start().starts_with("cmpi") && line.contains("%pc"))
}

#[test]
fn the_68000_never_compares_an_immediate_with_a_pc_relative_operand() {
    let out = text(Cpu::M68000);
    assert!(!has_cmpi_pc(&out), "a PC-relative cmpi on the 68000:\n{out}");
    assert!(out.contains("(buf+9,%pc)"), "the byte is no longer read PC-relative:\n{out}");
}

#[test]
fn the_68000_output_assembles() {
    let Some(root) = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty()) else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return;
    };
    let source = support::unique_path("cmpi-pc-s");
    let object = support::unique_path("cmpi-pc-o");
    std::fs::write(&source, format!(".text\n{}\n", text(Cpu::M68000))).unwrap();
    let output = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-as"))
        .env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"))
        .args(["-m68000", "-o"])
        .arg(&object)
        .arg(&source)
        .output()
        .unwrap();
    std::fs::remove_file(&source).ok();
    std::fs::remove_file(&object).ok();
    assert!(output.status.success(), "binutils rejects the output: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn the_68020_keeps_the_pc_relative_cmpi() {
    let out = text(Cpu::M68020);
    assert!(has_cmpi_pc(&out), "the 68020 lost its one-instruction compare:\n{out}");
}
