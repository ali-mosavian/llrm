//! A callee of about 100 in the pin's inline cost, called from a loop, stayed a
//! call under the flat threshold of 50 the m68k carried (the pin: 225), though
//! the call's latency at the loop's frequency pays for the copy's bytes.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn calls_f(
    cpu: &str,
    level: Level,
) -> bool {
    let out = assembly(include_str!("fixtures/hot_call.ll"), cpu_of(cpu).unwrap(), level).expect("compiles");
    let main = out.split("main:").nth(1).expect("main");
    main.contains("jsr") || main.contains("bsr")
}

#[test]
fn a_loop_inlines_a_callee_the_call_latency_pays_for() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::O3] {
            assert!(!calls_f(cpu, level), "{cpu} {level:?} kept the call");
        }
    }
}
