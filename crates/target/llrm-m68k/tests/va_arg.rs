//! Clang writes a variadic read as `va_arg`, which MIR's reader refused: any C
//! function reading its `...` failed to compile ("`va_arg` is not an
//! instruction of MIR's subset"; tests/run/c variadic, 2 of 114 programs).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::vaarg::without_va_arg;

mod support;

const SUM: &str = include_str!("fixtures/variadic_raw.ll");

#[test]
fn a_function_reading_its_variadic_arguments_compiles() {
    let asm = assembly(SUM, cpu_of("68020").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("sum:"), "{asm}");
}

/// The pin's `expandVAArg`: load the list, round to the type's alignment (a
/// long is 2-aligned), step by its allocation size, store back, load.
#[test]
fn va_arg_becomes_the_pins_expansion() {
    let out = without_va_arg("  %11 = va_arg ptr %3, i32\n  %12 = va_arg ptr %3, double\n");
    assert!(!out.contains("va_arg"), "{out}");
    assert!(out.contains("%va.0.sum = add i32 %va.0.int, 1\n"), "{out}");
    assert!(out.contains("getelementptr inbounds i8, ptr %va.0.at, i32 4\n"), "{out}");
    assert!(out.contains("%11 = load i32, ptr %va.0.at, align 2\n"), "{out}");
    assert!(out.contains("%va.1.sum = add i32 %va.1.int, 7\n"), "{out}");
    assert!(out.contains("getelementptr inbounds i8, ptr %va.1.at, i32 8\n"), "{out}");
}

#[test]
fn a_type_it_does_not_know_is_left_alone() {
    let text = "  %1 = va_arg ptr %0, %struct.s\n";
    assert_eq!(without_va_arg(text), text);
}

/// `sum(3, 10, 20, 30)` reads three longs through `va_start`/`va_arg`: 60. A
/// `va_start` that names no slot, or a step that is not the allocation size,
/// reads other stack words.
#[test]
fn the_variadic_arguments_are_read_from_the_stack() {
    let Some(qemu) = support::qemu() else { return };
    let text = format!(
        "{}\ndefine i32 @f() {{\n  %r = call i32 (i32, ...) @sum(i32 3, i32 10, i32 20, i32 30)\n  ret i32 %r\n}}\n",
        without_va_arg(SUM).replace(" dso_local", "").replace("target triple = \"m68k-unknown-linux-gnu\"\n", "")
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
