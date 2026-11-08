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

/// Medium is absolute on the 68000 and 68010 and, from the 68020, the full
/// format `(bd32,%pc,%zd0)`: the plain `(to,%pc)` is the 16-bit form to GNU as
/// (memcpy_runtime and memfuncs_runtime failed with ".text: a value of 70016
/// does not fit its 2-byte field").
#[test]
fn medium_is_absolute_on_the_68000_and_full_pc_relative_from_the_68020() {
    assert!(!asm("68000", CodeModel::Medium).contains("%pc"), "{}", asm("68000", CodeModel::Medium));
    for cpu in ["68020", "68040"] {
        let text = asm(cpu, CodeModel::Medium);
        assert!(text.contains("%pc,%zd0)") || text.contains("%pc,%d"), "{cpu}: {text}");
        assert!(!text.contains(",%pc)"), "{cpu}: {text}");
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
    std::fs::write(&source, format!(".text\n.globl f\n.globl g\n.globl h\n{text}")).unwrap();
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
define i32 @g() {
  %a = getelementptr [70016 x i8], ptr @to, i32 0, i32 70000
  %i = ptrtoint ptr %a to i32
  ret i32 %i
}
define i32 @h() {
  %b = getelementptr [70016 x i8], ptr @from, i32 0, i32 69999
  %i = ptrtoint ptr %b to i32
  ret i32 %i
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
/// (exit status 8), and `g` and `h` return the addresses the linker gave them:
/// a displacement off by the 2 bytes between the extension word and the base
/// displacement stores and loads at one wrong address and still reads back 8.
#[test]
fn far_globals_work_in_the_medium_model_at_every_cpu() {
    let Some(root) = support::oracle_root() else { return };
    let Some(qemu) = support::qemu() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    let ir = FAR_IR.replace("internal ", "");
    for (cpu, ours) in
        [("68000", false), ("68020", false), ("68040", false), ("68000", true), ("68020", true), ("68040", true)]
    {
        let object = support::unique_path("far-o");
        let source = support::unique_path("far-s");
        if ours {
            let bytes = object_in(&ir, cpu_of(cpu).unwrap(), Level::O2, "far", CodeModel::Medium).expect("compiles");
            std::fs::write(&object, bytes).unwrap();
        } else {
            let text = assembly_in(&ir, cpu_of(cpu).unwrap(), Level::O2, false, None, CodeModel::Medium).unwrap();
            std::fs::write(&source, format!(".text\n.globl f\n.globl g\n.globl h\n{text}")).unwrap();
            assert!(tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success());
        }
        let exe = support::unique_path("far-exe");
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
        assert_eq!(code, Some(8), "{cpu} object from our writer: {ours}");
    }
}

/// The pin has no `(bd32,%pc)` load, and the medium model at the 68020 refused
/// every plain load of a global (`CannotSelect(Load)`: memcpy_runtime and
/// memfuncs_runtime, 2026-11-09). Loads go through `lea` and `(An)`.
#[test]
fn a_load_of_a_global_selects_in_the_medium_model_at_the_68020() {
    let header = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n@g = global [100 x i32] zeroinitializer, align 4\n";
    let bodies = [
        "define i32 @f() { %v = load i32, ptr @g  ret i32 %v }",
        "define i32 @f() { %p = getelementptr i32, ptr @g, i32 5  %v = load i32, ptr %p  ret i32 %v }",
        "define i32 @f(i32 %i) { %p = getelementptr i32, ptr @g, i32 %i  %v = load i32, ptr %p  ret i32 %v }",
        "define i32 @f() { %v = load i8, ptr @g  %z = zext i8 %v to i32  ret i32 %z }",
        "define i32 @f() { %v = load i16, ptr @g  %z = sext i16 %v to i32  ret i32 %z }",
        "define i32 @f() { %v = load volatile i32, ptr @g  ret i32 %v }",
    ];
    for body in bodies {
        for level in [Level::O0, Level::O2] {
            let text = format!("{header}{body}\n");
            let done = assembly_in(&text, cpu_of("68020").unwrap(), level, false, None, CodeModel::Medium);
            assert!(done.is_ok(), "{level:?} {body}: {done:?}");
        }
    }
}
