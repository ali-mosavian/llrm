//! switch_i64 -Os/-O1 (68000/68020, +4..+16 B against gcc): a branch on a `long
//! long` ordering compare saved the borrow of the low halves in a byte (`sub.l;
//! scs; add.b #-1`, to make the extend flag again), then `subx.l`, `sge`, `and.
//! b #1` and `bne` where gcc has `sub.l; subx.l; bge`. The compare is flags all
//! the way.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
                  declare void @report(i32)\n\
                  define void @f(i64 %a) {\n  %c = icmp slt i64 %a, 10\n  br i1 %c, label %t, label %e\n\
                  t:\n  call void @report(i32 1)\n  ret void\n\
                  e:\n  ret void\n}\n";

#[test]
fn a_branch_on_a_wide_compare_reads_the_flags_of_its_subx() {
    for cpu in ["68000", "68020"] {
        for level in [Level::Os, Level::O1] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(asm.contains("subx.l"), "{cpu} {level:?}: no borrow chain:\n{asm}");
            for round_trip in ["scs", "add.b\t#-1", "and.b\t#1"] {
                assert!(
                    !asm.contains(round_trip),
                    "{cpu} {level:?}: the borrow goes through a byte ({round_trip}):\n{asm}"
                );
            }
        }
    }
}
