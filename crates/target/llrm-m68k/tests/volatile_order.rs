//! Arguments read from volatile globals were pushed as `move.l (g3,%pc),
//! -(%sp)`, `g2`, `g1`: the load-into-push fold (`callpush`) ran the three
//! volatile loads in reverse source order at every level.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@g1 = global i32 0
@g2 = global i32 0
@g3 = global i32 0
declare void @use(i32, i32, i32)
define void @f() {
  %a = load volatile i32, ptr @g1
  %b = load volatile i32, ptr @g2
  %c = load volatile i32, ptr @g3
  call void @use(i32 %a, i32 %b, i32 %c)
  store volatile i32 1, ptr @g1
  ret void
}
";

#[test]
fn volatile_loads_feeding_call_arguments_run_in_source_order() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2, Level::Os] {
            let text = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let at = |name: &str| {
                text.find(&format!("({name},")).unwrap_or_else(|| panic!("{cpu} {level:?}: no {name}\n{text}"))
            };
            assert!(at("g1") < at("g2") && at("g2") < at("g3"), "{cpu} {level:?}: loads out of source order\n{text}");
        }
    }
}
