//! Clang writes a variadic read as `va_arg`, which MIR's reader refused: any C
//! function reading its `...` failed to compile ("`va_arg` is not an
//! instruction of MIR's subset"; tests/run/c variadic, 2 of 114 programs).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const SUM: &str = include_str!("fixtures/variadic_raw.ll");

#[test]
fn a_function_reading_its_variadic_arguments_compiles() {
    let asm = assembly(SUM, cpu_of("68020").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("sum:"), "{asm}");
}

/// Every scalar a C `...` reads compiles: the selector expands `va_arg` itself.
#[test]
fn va_arg_of_each_scalar_compiles() {
    for ty in ["i8", "i16", "i32", "i64", "float", "double", "ptr"] {
        let text = format!(
            "target datalayout = \"{}\"\ndefine {ty} @f(ptr %ap) {{\n  %v = va_arg ptr %ap, {ty}\n  ret {ty} %v\n}}\n",
            llrm_m68k::kernels::DATALAYOUT
        );
        assembly(&text, cpu_of("68020").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{ty}: {why}"));
    }
}

/// `sum(3, 10, 20, 30)` reads three longs through `va_start`/`va_arg`: 60. A
/// `va_start` that names no slot, or a step that is not the allocation size,
/// reads other stack words.
#[test]
fn the_variadic_arguments_are_read_from_the_stack() {
    let Some(qemu) = support::qemu() else { return };
    let text = format!(
        "{}\ndefine i32 @f() {{\n  %r = call i32 (i32, ...) @sum(i32 3, i32 10, i32 20, i32 30)\n  ret i32 %r\n}}\n",
        SUM.replace(" dso_local", "").replace("target triple = \"m68k-unknown-linux-gnu\"\n", "")
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("f", &[]);
        for name in ["f", "sum"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("va_arg");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(60), "{cpu:?}");
    }
}

/// `g(1, 2:3 as a double, 5)` reads the double through `va_arg`. The double is
/// expanded into two 4-byte reads, the first asked for 8-byte alignment; the
/// step is the 4-byte allocation size (pin `expandVAArg`), so the second half
/// is at +4. A step rounded up to the alignment read it at +8 and returned
/// garbage (`long_double_variadic`, #7265).
#[test]
fn a_double_is_read_as_two_adjacent_words() {
    let Some(qemu) = support::qemu() else { return };
    let text = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
        define i32 @g(i32 %n, ...) {\n  %ap = alloca ptr, align 2\n  call void @llvm.va_start.p0(ptr %ap)\n\
          %d = va_arg ptr %ap, double\n  %b = bitcast double %d to i64\n  %lo = trunc i64 %b to i32\n\
          %h = lshr i64 %b, 32\n  %hi = trunc i64 %h to i32\n  %e = va_arg ptr %ap, i32\n\
          %h100 = shl i32 %hi, 6\n  %l10 = shl i32 %lo, 3\n  %s = add i32 %h100, %l10\n  %r = add i32 %s, %e\n  ret i32 %r\n}\n\
        define i32 @f() {\n  %dd = bitcast i64 8589934595 to double\n\
          %r = call i32 (i32, ...) @g(i32 1, double %dd, i32 5)\n  ret i32 %r\n}\n\
        declare void @llvm.va_start.p0(ptr)\n";
    let module = llrm_mir::parse::module(text).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("f", &[]);
        for name in ["f", "g"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("va_double");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(157), "{cpu:?}");
    }
}
