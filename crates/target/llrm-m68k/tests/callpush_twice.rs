//! One loaded value passed as two arguments: `a + a` on a float is a call
//! `__addsf3(a, a)`. The first push read the load's memory and the load was
//! dropped, so the second push stored a `%d0` nothing had written
//! (`poly` -O2 printed 631 for 643).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @g(i32, i32)
define i32 @f(i32 %a) {
  %r = call i32 @g(i32 %a, i32 %a)
  ret i32 %r
}
";

#[test]
fn a_register_pushed_twice_is_loaded() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu} {level:?}: {why}"));
            let lines: Vec<&str> = asm.lines().map(str::trim).collect();
            let call = lines.iter().position(|line| line.starts_with("jsr")).expect("a call");
            for (at, line) in lines[..call].iter().enumerate() {
                if let Some(reg) = line.strip_prefix("move.l\t").and_then(|rest| rest.strip_suffix(", -(%sp)")) {
                    if reg.starts_with('%') {
                        let written = lines[..at].iter().any(|before| before.ends_with(&format!(", {reg}")));
                        assert!(written, "{cpu} {level:?}: {reg} is pushed before anything wrote it:\n{asm}");
                    }
                }
            }
        }
    }
}
