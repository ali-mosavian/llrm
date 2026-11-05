//! A global past 32 KiB of the code cannot be PC-relative: `lea (to,%pc), %a0` has a 16-bit displacement, and
//! the assembler relaxes only branches ("value of 000110f0 too large for field of 2 bytes", 16 sweep cells).
//! The pin picks the form by code model; small is PC-relative, medium is absolute on the 68000 and 68010, large
//! is absolute everywhere (`M68kSubtarget::classifyGlobalReference`).

use std::process::Command;

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, assembly_in, cpu_of};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@to = global [70016 x i8] zeroinitializer, align 2
define i8 @f(i32 %i) {
  %j = add i32 %i, 1
  %p = getelementptr [70016 x i8], ptr @to, i32 0, i32 %j
  %v = load i8, ptr %p
  ret i8 %v
}
";

fn asm(
    cpu: &str,
    model: CodeModel,
) -> String {
    assembly_in(IR, cpu_of(cpu).unwrap(), Level::O2, false, None, model).expect("compiles")
}

#[test]
fn an_unknown_model_is_refused_not_dropped() {
    assert!(CodeModel::parse("huge").is_err());
    assert_eq!(CodeModel::parse("medium"), Ok(CodeModel::Medium));
}

#[test]
fn small_is_pc_relative_at_every_cpu() {
    for cpu in ["68000", "68020"] {
        assert!(asm(cpu, CodeModel::Small).contains("%pc"), "{cpu}");
    }
}

#[test]
fn medium_is_absolute_on_the_68000_and_pc_relative_on_the_68020() {
    assert!(!asm("68000", CodeModel::Medium).contains("%pc"), "{}", asm("68000", CodeModel::Medium));
    assert!(asm("68020", CodeModel::Medium).contains("%pc"));
}

#[test]
fn large_is_absolute_at_every_cpu() {
    for cpu in ["68000", "68020"] {
        assert!(!asm(cpu, CodeModel::Large).contains("%pc"), "{cpu}: {}", asm(cpu, CodeModel::Large));
    }
}

#[test]
fn binutils_assembles_the_big_array_in_the_medium_model_at_68000() {
    let Some(root) = support::oracle_root() else { return };
    let text = asm("68000", CodeModel::Medium);
    let source = support::unique_path("model-s");
    let object = support::unique_path("model-o");
    std::fs::write(&source, format!(".text\n.globl f\n{text}")).unwrap();
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-as"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    let done = command.arg("-m68000").arg("-o").arg(&object).arg(&source).output().unwrap();
    assert!(done.status.success(), "{}\n{text}", String::from_utf8_lossy(&done.stderr));
}
