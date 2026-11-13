//! float_return -O1 (68000/68020, +12 B against gcc, 6 cells): `y + y` on a double is `__adddf3(y, y)`, a load of each
//! word into `%a1` stored twice into the call area. The fold of a push into its load's memory refused a load a second
//! store reads, so both calls kept the area (`lea -16,%sp`, 4 loads and 8 stores): 110 B against gcc's 98.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
                  define double @scale(double %x, double %y) {\n  %a = fadd double %y, %y\n  %b = fadd double %a, %x\n  ret double %b\n}\n";

#[test]
fn a_word_two_arguments_read_is_pushed_from_its_memory_twice() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::O1).expect("compiles");
        let stores = asm.lines().filter(|line| line.trim_start().starts_with("move.l\t%a1, (")).count();
        assert_eq!(stores, 0, "{cpu}: call-area stores remain:\n{asm}");
        assert!(!asm.contains("lea\t(-16, %sp)"), "{cpu}: the call area stayed:\n{asm}");
    }
}
