//! CS1 (`cgpselect`: a select around a dear conversion becomes a branch that
//! skips it) fired on m68k once `costs()` carried clocks (`__fixsfsi`, 600 to
//! 1,000), and added a branch, blocks and spills the pin does not make
//! (sfm16_float -O1 +58 B at the 68000, +48 B at the 68020). It is for targets
//! whose select is a branch around a copy anyway (`sinks_select_arms`); m68k's
//! select is not, so the select stays at every level.

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

/// The select stays whole: no branch before the call to the conversion.
#[test]
fn a_select_around_a_conversion_stays_a_select_at_every_level() {
    for cpu in ["68000", "68020"] {
        for (level, attrs) in [(Level::O1, ""), (Level::O2, ""), (Level::O3, ""), (Level::Os, "optsize")] {
            let asm = assembly(&clamp(attrs), cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!branches_around_the_call(&asm), "{cpu} {level:?}:\n{asm}");
        }
    }
}
