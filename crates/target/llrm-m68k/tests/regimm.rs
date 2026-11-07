//! A long operation with a small immediate: `and.l #15,%d2` is 6 bytes and 16
//! clocks on the 68000, `moveq #15,%d3; and.l %d3,%d2` 4 and 12 (gcc's form,
//! hex -Os 72 against 62 bytes).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %a, i32 %b) {
  %m = and i32 %a, 15
  %c = icmp sgt i32 %m, 9
  %r = select i1 %c, i32 %b, i32 %m
  %s = add i32 %r, 100
  ret i32 %s
}
";

/// `and.l #15`, `cmpi.l #9` and `add.l #100` were 6-byte immediate forms.
#[test]
fn a_small_long_immediate_comes_from_a_register() {
    for (cpu, level) in [("68000", Level::Os), ("68000", Level::O2), ("68020", Level::Os)] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
        let wide: Vec<&str> = asm
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                ["and.l\t#", "cmpi.l\t#", "add.l\t#", "sub.l\t#", "or.l\t#"].iter().any(|op| line.starts_with(op))
            })
            .collect();
        assert!(wide.is_empty(), "{cpu} {level:?}: {wide:?}\n{asm}");
        assert!(asm.contains("moveq\t#15"), "{cpu} {level:?}\n{asm}");
    }
}

/// The register forms compute what the immediate forms did: `f(0x2b, 7)` is
/// `(0x2b & 15 = 11) > 9`, so `7 + 100`.
#[test]
fn the_register_forms_compute_the_same() {
    let Some(qemu) = support::qemu() else { return };
    let text = format!(
        "{}\ndefine i32 @main2() {{\n  %r = call i32 @f(i32 43, i32 7)\n  %q = call i32 @f(i32 33, i32 7)\n  %s = shl i32 %q, 7\n  %t = add i32 %r, %s\n  ret i32 %t\n}}\n",
        IR
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut all = start("main2", &[]);
        for name in ["main2", "f"] {
            all.extend(items_with(&module, name, cpu, Options::default()).expect("compiles"));
        }
        let path = support::unique_path("regimm");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &[], &path);
        std::fs::remove_file(&path).ok();
        // f(43,7) = 107; f(33,7): 33&15 = 1, not > 9, 1 + 100 = 101; 107 + (101
        // << 7) wraps to a byte
        assert_eq!(code, Some(((107 + (101 << 7)) & 255) as i32), "{cpu:?}");
    }
}

/// With every data register the function uses live across the `and`, the
/// immediate form stays: the rewrite never adds a spill, a copy or a saved
/// register (`d3` would cost a `movem`).
#[test]
fn no_free_data_register_keeps_the_immediate_form() {
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @g(i32 %a, i32 %b) {
  %m = and i32 %a, 15
  %x = add i32 %m, %b
  %y = add i32 %x, %a
  %z = mul i32 %y, %b
  ret i32 %z
}
";
    let asm = assembly(ir, cpu_of("68020").unwrap(), Level::Os).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("and.l\t#15"), "{asm}");
    assert!(!asm.contains("moveq\t#15") && !asm.contains("movem"), "{asm}");
}
