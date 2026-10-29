//! Overlay rewrites against their boundaries: the text each one gives, and the
//! value the program computes under `qemu-m68k` (the exit status, 8 bits).

use std::path::PathBuf;
use std::process::Command;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn qemu() -> Option<PathBuf> {
    ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists())
}

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Exit status of `f(args)` compiled with `options`; `None` without qemu.
fn status(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = qemu().or_else(|| {
        eprintln!("qemu-m68k is not installed: not run");
        None
    })?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, options).expect("compiles"));
    let path = std::env::temp_dir().join(format!("llrm-m68k-results-{}-{}", std::process::id(), ir.len()));
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = Command::new("timeout").args(["10"]).arg(qemu).arg(&path).status().unwrap().code();
    std::fs::remove_file(&path).ok();
    code
}

/// `(a & c) + a`: `move.l a, d; and.l #c, d` becomes `moveq #c, d; and.l a, d`
/// only where `c` fits `moveq`'s signed byte. 127 and -128 do, 128 and -129 do
/// not (`moveq #128` would load -128), and the value is the same either way.
#[test]
fn a_mask_is_a_moveq_only_within_its_signed_byte() {
    for (mask, quick) in [(127, true), (128, false), (-128, true), (-129, false)] {
        let ir = format!("define i32 @f(i32 %a) {{\n  %m = and i32 %a, {mask}\n  %r = add i32 %m, %a\n  ret i32 %r\n}}");
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert_eq!(asm.contains(&format!("moveq\t#{mask},")), quick, "mask {mask}: {asm}");
        if let Some(code) = status(&ir, &[0x85], Options::default()) {
            assert_eq!(code, ((0x85 & mask) + 0x85) & 0xFF, "mask {mask}: {asm}");
        }
    }
}
