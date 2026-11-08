//! A global past 32 KiB of the code cannot be PC-relative: `lea (to,%pc), %a0`
//! has a 16-bit displacement, and the assembler relaxes only branches ("value
//! of 000110f0 too large for field of 2 bytes", 16 sweep cells). The pin picks
//! the form by code model; small is PC-relative, medium is absolute on the
//! 68000 and 68010, large is absolute everywhere
//! (`M68kSubtarget::classifyGlobalReference`).

use std::process::Command;

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, assembly_in, cpu_of, object_in};

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

const ADDRESS_IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@to = global [70016 x i8] zeroinitializer, align 2
define ptr @addr() { ret ptr @to }
";

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

/// The 68020 stayed PC-relative in the medium model, and its 16-bit
/// displacement refused a global past 32 KiB (memcpy_runtime and
/// memfuncs_runtime, -O0 sweep: ".text: a value of 70016 does not fit its
/// 2-byte field").
/// Medium reaches any offset at every cpu with `abs.l` (DEVIATE in `CodeModel`): the pin's PC-relative medium at the
/// 68020 has only a 16-bit displacement and fails to link past 32 KiB.
#[test]
fn medium_is_absolute_at_every_cpu_because_the_pin_pc16_cannot_reach() {
    for cpu in ["68000", "68020"] {
        assert!(!asm(cpu, CodeModel::Medium).contains("%pc"), "{cpu}: {}", asm(cpu, CodeModel::Medium));
    }
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

/// The absolute form printed `move.l to, %a0`: the text of a load from `to`,
/// where the object code (`movea.l #to`) had the address. The three `*_runtime`
/// programs read the array's first bytes as a pointer and faulted at 68000.
#[test]
fn an_absolute_address_is_an_immediate_not_a_load_of_the_symbol() {
    for (cpu, model) in [("68000", CodeModel::Medium), ("68000", CodeModel::Large), ("68020", CodeModel::Large)] {
        let text = assembly_in(ADDRESS_IR, cpu_of(cpu).unwrap(), Level::O2, false, None, model).expect("compiles");
        assert!(text.contains("#to,"), "{cpu} {model:?}: {text}");
        assert!(!text.contains("move.l\tto,"), "{cpu} {model:?}: {text}");
    }
}

const FAR_IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@from = internal global [70016 x i8] zeroinitializer, align 2
@to = internal global [70016 x i8] zeroinitializer, align 2
define i32 @f() {
  %a = getelementptr [70016 x i8], ptr @to, i32 0, i32 70000
  store volatile i8 5, ptr %a
  %b = getelementptr [70016 x i8], ptr @from, i32 0, i32 69999
  store volatile i8 3, ptr %b
  %x = load volatile i8, ptr %a
  %y = load volatile i8, ptr %b
  %s = add i8 %x, %y
  %r = zext i8 %s to i32
  ret i32 %r
}
";

/// A global 70016 B into `.bss` cannot be reached by `(d16,%pc)`: the small
/// model refuses the object ("does not fit its 2-byte field"), never truncating
/// the displacement; medium writes it at every cpu.
#[test]
fn a_displacement_past_the_field_is_refused_not_truncated() {
    for cpu in ["68000", "68020"] {
        let cpu = cpu_of(cpu).unwrap();
        let small = object_in(FAR_IR, cpu, Level::O0, "f", CodeModel::Small);
        assert!(
            small.as_ref().is_err_and(|error| error.contains("does not fit")),
            "{:?}",
            small.map(|bytes| bytes.len())
        );
        assert!(object_in(FAR_IR, cpu, Level::O0, "f", CodeModel::Medium).is_ok());
    }
}

/// The far arrays read back what was stored, linked and run at 68000 and 68020
/// (exit status 8).
#[test]
fn far_globals_work_in_the_medium_model_at_every_cpu() {
    let Some(root) = support::oracle_root() else { return };
    let Some(qemu) = support::qemu() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for cpu in ["68000", "68020"] {
        let text =
            assembly_in(FAR_IR, cpu_of(cpu).unwrap(), Level::O2, false, None, CodeModel::Medium).expect("compiles");
        let source = support::unique_path("far-s");
        let object = support::unique_path("far-o");
        let exe = support::unique_path("far-exe");
        std::fs::write(&source, format!(".text\n.globl f\n{text}")).unwrap();
        assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
        let main = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt/far_main.c");
        let linked = tool("gcc")
            .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
            .arg(main)
            .arg(&object)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap();
        assert!(linked.success());
        let code = support::run(&qemu, &["-cpu", &format!("m{cpu}")], &exe);
        for path in [source, object, exe] {
            std::fs::remove_file(path).ok();
        }
        assert_eq!(code, Some(8), "{cpu}");
    }
}
