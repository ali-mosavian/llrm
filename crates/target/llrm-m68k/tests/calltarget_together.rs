//! Three callees called five times each, as pc-relative calls (4 B): alone each
//! saves 10 B of calls and pays the 6 B `lea` and the 4 B save and restore of
//! its register, so none was kept; the three share one `movem` (8 B) and save
//! 30 B for the 18 B of `lea`s. `loop_versioning` came out 8 B larger once
//! calls were pc-relative, for want of the three being priced together.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn module() -> String {
    let mut ir = String::from(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
         @s = global i32 0\n",
    );
    for name in ["a", "b", "c"] {
        ir.push_str(&format!("define void @{name}() noinline {{\n  store volatile i32 1, ptr @s\n  ret void\n}}\n"));
    }
    ir.push_str("define void @f() {\n");
    for _ in 0..5 {
        for name in ["a", "b", "c"] {
            ir.push_str(&format!("  call void @{name}()\n"));
        }
    }
    ir.push_str("  store volatile i32 2, ptr @s\n  ret void\n}\n");
    ir
}

#[test]
fn callees_that_pay_only_together_are_all_called_through_registers() {
    let out = assembly(&module(), cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    let through = out.lines().filter(|line| line.contains("jsr\t(%a")).count();
    assert_eq!(through, 15, "{out}");
}
