//! call_clobbers_flags -Os (68000/68020, +12 B against gcc, 6 cells): three `report(pick(k))` pairs kept their call
//! area (`lea -4,%sp`, `move.l #k,(%sp)` 6 B each, `addq #4,%sp`) because pushing was priced with one cleanup a call
//! (`pea k` + `addq` = 6 B, a tie), though the expansion owes the cleanups until something reads the stack pointer and
//! pays them once (gcc: `pea k`, one `lea (12,%sp),%sp`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
                  declare i32 @pick(i32)\ndeclare void @report(i32)\n\
                  define i32 @main() {\n\
                    %a = call i32 @pick(i32 12)\n  call void @report(i32 %a)\n\
                    %b = call i32 @pick(i32 13)\n  call void @report(i32 %b)\n\
                    %c = call i32 @pick(i32 30)\n  call void @report(i32 %c)\n  ret i32 0\n}\n";

#[test]
fn consecutive_calls_with_one_argument_push_and_pay_their_cleanups_once() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let stores = asm.lines().filter(|line| line.trim_start().starts_with("move.l\t#") && line.ends_with("(%sp)")).count();
        let cleanups = asm.lines().filter(|line| line.contains("%sp") && (line.contains("addq.l") || line.contains("lea\t("))).count();
        assert_eq!(stores, 0, "{cpu}: constants stored into a kept call area:\n{asm}");
        assert_eq!(cleanups, 1, "{cpu}: the cleanups were not merged:\n{asm}");
    }
}
