//! A compare whose destination was widened for `cmpa.l`, then given a scaled
//! index (`cmp.l (0,%a0,%d0.l*4), %a1`): the scaled row has no `cmpa.l` twin,
//! so it kept `cmp.l` with an address register, which encodes a data
//! register. Six QB modules (`exec.c`, `linemap.c`..) were refused at 68020
//! -O0.

use llrm_m68k::driver::{self, Level};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i1 @below(ptr %table, i32 %at, i32 %value) {
  %p = getelementptr [4 x i8], ptr %table, i32 %at
  %x = load i32, ptr %p, align 2
  %c = icmp ult i32 %value, %x
  ret i1 %c
}
"#;

#[test]
fn a_scaled_compare_keeps_its_destination_in_a_data_register() {
    for cpu in ["68020", "68040"] {
        for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
            let target = driver::cpu_of(cpu).unwrap();
            driver::object(IR, target, level, "below").unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            let text = driver::assembly(IR, target, level).expect("compiles");
            assert!(text.contains(".l*4)"), "{cpu} {level:?}: premise, a scaled index:\n{text}");
            let address_dest = text
                .lines()
                .any(
                    |line| line.trim_start().starts_with("cmp.l")
                        && line.rsplit(", ").next().is_some_and(|to| to.starts_with("%a")),
                );
            assert!(!address_dest, "{cpu} {level:?}: cmp.l into an address register:\n{text}");
        }
    }
}
