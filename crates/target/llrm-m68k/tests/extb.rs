//! `(int)c` of a signed byte at 68020 was `ext.w; ext.l` (4 bytes, 2 words):
//! the 68020's `extb.l` is one (2 bytes). memset against gcc -m68020 (+8 B).
//! The pin has no `extb`; the 68000 and 68010 do not either.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand, print};
use llrm_m68k::reg;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const SEXT: &str = "
define i32 @f(i8 %c) {
  %w = sext i8 %c to i32
  ret i32 %w
}";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{SEXT}")).expect("parses")
}

#[test]
fn a_signed_byte_to_long_is_one_extb_at_68020() {
    for cpu in [Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        let out = text_with(&module(), "f", cpu, Options::default()).expect("compiles");
        assert!(out.contains("extb.l"), "{cpu:?}: no extb.l:\n{out}");
        assert!(!out.contains("ext.w"), "{cpu:?}: the word step is left:\n{out}");
    }
}

#[test]
fn the_68000_and_68010_extend_in_two_steps() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        let out = text_with(&module(), "f", cpu, Options::default()).expect("compiles");
        assert!(out.contains("ext.w") && out.contains("ext.l") && !out.contains("extb"), "{cpu:?}:\n{out}");
    }
}

#[test]
fn the_extended_value_is_right_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for (c, want) in [(5_i64, 5), (127, 127), (-1, 255), (-128, 128), (0x80, 128)] {
        let mut all = start("f", &[c]);
        all.extend(items_with(&module(), "f", Cpu::M68020, Options::default()).unwrap());
        let path = support::unique_path("extb");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        std::fs::remove_file(&path).ok();
        // the exit code is the low byte of the result: -1 is 255
        assert_eq!(code, Some(want), "sext i8 {c}");
    }
}

/// The bytes binutils makes of `extb.l %dn`, for each data register.
#[test]
fn extb_encodes_as_binutils_does() {
    let Some(root) = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty()) else {
        assert!(std::env::var("M68K_ISA_SKIP_ORACLE").is_ok_and(|flag| flag == "1"), "GCC_M68K is not set");
        return;
    };
    let tool = |name: &str| {
        let mut command = std::process::Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for register in [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7] {
        let inst = Inst { opcode: "EXTB32", operands: vec![Operand::Reg(register), Operand::Reg(register)] };
        let text = print(&inst).unwrap();
        let (object, source, flat) =
            (support::unique_path("extb-o"), support::unique_path("extb-s"), support::unique_path("extb-bin"));
        std::fs::write(&source, format!("{text}\n")).unwrap();
        assert!(tool("as").args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap().success(), "{text}");
        let status = tool("objcopy").args(["-O", "binary", "-j", ".text"]).arg(&object).arg(&flat).status().unwrap();
        assert!(status.success());
        assert_eq!(encode(&inst).unwrap().bytes, std::fs::read(&flat).unwrap(), "{text}");
        for path in [object, source, flat] {
            std::fs::remove_file(path).ok();
        }
    }
}
