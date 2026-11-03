//! A function that returns a struct through a hidden `sret` pointer.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const IR: &str = r#"
define void @mk(ptr noalias sret({ i32, i32 }) align 2 %out, i32 %a, i32 %b) noinline {
  store i32 %a, ptr %out
  %hi = getelementptr inbounds i8, ptr %out, i32 4
  store i32 %b, ptr %hi
  ret void
}

; 0 when both halves arrive
define i32 @f() {
  %pair = alloca { i32, i32 }, align 2
  call void @mk(ptr sret({ i32, i32 }) align 2 %pair, i32 7, i32 9)
  %lo = load i32, ptr %pair
  %hip = getelementptr inbounds i8, ptr %pair, i32 4
  %hi = load i32, ptr %hip
  %bad = add i32 %lo, %hi
  %ok = sub i32 %bad, 16
  ret i32 %ok
}
"#;

/// Every struct-returning function was refused ("a parameter attribute that
/// moves it, with a target's calling hook"): `-O0` softfloat's `split()` made
/// the corpus fail to build.
#[test]
fn a_struct_returned_through_a_hidden_pointer_arrives() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{IR}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[]);
        for name in ["f", "mk"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("struct_return");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{cpu:?}: lo + hi - 16");
    }
}

/// The pin copies the `sret` pointer to `d0` before the return.
#[test]
fn the_sret_pointer_is_returned_in_d0() {
    let text = llrm_m68k::driver::assembly(
        &format!("target datalayout = \"{DATALAYOUT}\"\n{IR}"),
        Cpu::M68000,
        llrm_m68k::driver::Level::O0,
    )
    .expect("compiles");
    let mk = text.split("\n\t.globl\tf\nf:").next().unwrap();
    assert!(mk.contains("%d0") && mk.trim_end().ends_with("rts"), "mk returns the pointer in d0:\n{mk}");
}
