//! `and x, 0xFFFF` of a value that stays live was `move.l %d1,%d2; and.l
//! #65535,%d2` (8 B, 16 clocks): the extension of the low word is `moveq
//! #0,%d2; move.w %d1,%d2` (4 B). bench/sieve -Os 68000 was +12 B on gcc.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};

use crate::support;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @mix(i32 %x) noinline nounwind optsize {
  %w = and i32 %x, 65535
  %b = and i32 %x, 255
  %t = add i32 %w, %b
  %r = xor i32 %t, %x
  ret i32 %r
}
define i32 @main2() {
  %a = call i32 @mix(i32 305419896)
  %b = call i32 @mix(i32 2596069104)
  %ok1 = icmp eq i32 %a, 305397896
  %ok2 = icmp eq i32 %b, 2596012304
  %z1 = zext i1 %ok1 to i32
  %z2 = zext i1 %ok2 to i32
  %s2 = shl i32 %z2, 1
  %r = or i32 %z1, %s2
  ret i32 %r
}
";

/// The masks of a value that is read again are extensions, not `and.l`: 4 B
/// each, not 8.
#[test]
fn a_mask_of_a_live_value_is_a_zero_extension() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let mix = asm.split("mix:").nth(1).and_then(|rest| rest.split("main2:").next()).expect("mix");
        assert!(!mix.contains("and.l"), "{cpu}: the mask is an and.l:\n{mix}");
        assert!(mix.contains("move.w") && mix.contains("move.b"), "{cpu}: no zero extension:\n{mix}");
    }
}

/// Both masks keep the low part and clear the rest, on the CPU the code is
/// built for.
#[test]
fn the_extension_keeps_the_low_part() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(IR).expect("parses");
    for (cpu, name) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("main2", &[]);
        for function in ["main2", "mix"] {
            all.extend(
                items_with(&module, function, cpu, Options { size: true, ..Options::default() }).expect("compiles"),
            );
        }
        let path = support::unique_path("and_mask");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", name], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(3), "{cpu:?}");
    }
}

/// The pass runs at every level and CPU: its two instructions are never slower
/// than the copy and the `and` (the timing tables, 2 B + 2 B against 2 B + 6
/// B).
#[test]
fn the_extension_is_never_slower_than_the_copy_and_the_mask() {
    use llrm_m68k::isa::Cpu as Model;
    use llrm_m68k::timing::cost_on;
    for cpu in Model::ALL {
        let clocks = |text: &str, bytes| cost_on(cpu, text, bytes, false).unwrap().clocks;
        let copy = clocks("move.l\t%d1, %d2", 2);
        for (mask, extend) in [("255", "move.b\t%d1, %d2"), ("65535", "move.w\t%d1, %d2")] {
            let masked = copy + clocks(&format!("and.l\t#{mask}, %d2"), 6);
            let extended = clocks("moveq\t#0, %d2", 2) + clocks(extend, 2);
            assert!(extended <= masked, "{cpu:?} #{mask}: {extended} clocks against {masked}");
        }
    }
}
