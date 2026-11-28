//! A callee of about 100 in the pin's inline cost, called from a loop, stayed a
//! call under the flat threshold of 50 the m68k carried (the pin: 225), though
//! the call's latency at the loop's frequency pays for the copy's bytes.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn calls_f(
    cpu: &str,
    level: Level,
) -> bool {
    let out = assembly(include_str!("../fixtures/hot_call.ll"), cpu_of(cpu).unwrap(), level).expect("compiles");
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

/// switch_peeled_table -O1: `pick`, an external callee of one site with a
/// seven-compare switch, was inlined (68000 174 -> 224 B). The size view
/// counted its switch as one branch, so SZ1 read the copy as no growth.
const PICK: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
define i32 @pick(i32 %m, i32 %fall) {\n\
entry:\n  switch i32 %m, label %d [ i32 192, label %a i32 193, label %f i32 194, label %b i32 195, label %f i32 196, label %c i32 219, label %e ]\n\
a:\n  ret i32 3\nb:\n  ret i32 4\nc:\n  ret i32 2\ne:\n  ret i32 1\nf:\n  ret i32 %fall\nd:\n  ret i32 0\n}\n\
define i32 @main(i32 %n) {\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]\n  %acc = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n\
  %v = call i32 @pick(i32 %i, i32 9)\n  %acc2 = add i32 %acc, %v\n  %i1 = add i32 %i, 1\n  %c = icmp ult i32 %i1, %n\n  br i1 %c, label %loop, label %done\n\
done:\n  ret i32 %acc2\n}\n";

/// M68K-INLINE-O1: the callee costs more than the -O1 bar (112,
/// `Tuning::inline_threshold_o1`) and less than the pin's 225, so it is a call
/// at -O1 and inlined at -O2. Before the bar it was inlined at -O1 for its
/// clocks (2026-11-14); the clock trial does not reach it
/// (M68K-O1-INLINE-CLOCK-TRIAL).
#[test]
fn an_external_callee_with_a_switch_is_a_call_at_o1_and_inlined_at_o2() {
    for cpu in ["68000", "68020"] {
        for (level, inlined) in [(Level::O1, false), (Level::O2, true)] {
            let out = assembly(PICK, cpu_of(cpu).unwrap(), level).expect("compiles");
            let main = out.split("main:").nth(1).expect("main");
            let called = main.contains("jsr") || main.contains("bsr");
            assert_eq!(
                called,
                !inlined,
                "{cpu} {level:?}: `pick` is {}:\n{out}",
                if called { "a call" } else { "inlined" }
            );
        }
    }
}
