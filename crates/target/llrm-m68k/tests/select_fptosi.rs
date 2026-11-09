//! `costs()` priced a soft `fptosi` at one clock, so CS1 (`cgpselect`: a select
//! around a dear conversion becomes a branch that skips it) never fired on
//! m68k. With clocks (`__fixsfsi`, 600 to 1,000) it fires, and in a function
//! optimized for size it added a branch and blocks the pin does not make:
//! `optimizeSelectInst` returns for `shouldOptimizeForSize` (sfm16_float -Os
//! +58 B at the 68000, +48 B at the 68020).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn clamp(attrs: &str) -> String {
    format!(
        r#"{DATALAYOUT}
define i32 @k(float %x, i1 %c) {attrs} {{
  %s = select i1 %c, float 3.0, float %x
  %r = fptosi float %s to i32
  ret i32 %r
}}
"#
    )
}

/// The conversion is made on the one path that needs it: a branch before the
/// `jsr`.
fn branches_around_the_call(asm: &str) -> bool {
    let call = asm.find("__fixsfsi").expect("a call to the conversion");
    asm[..call].lines().any(|line| line.trim_start().starts_with('b') && !line.contains("bsr"))
}

#[test]
fn a_function_optimized_for_size_keeps_the_select_around_a_conversion() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(&clamp("optsize"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(!branches_around_the_call(&asm), "{cpu}:\n{asm}");
    }
}

#[test]
fn a_function_optimized_for_speed_skips_the_conversion_on_the_constant_path() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(&clamp(""), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(branches_around_the_call(&asm), "{cpu}:\n{asm}");
    }
}
