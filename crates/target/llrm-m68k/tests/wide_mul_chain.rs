//! sysv_abi 68000 -Os/-O1 (+38/+62 B against gcc): `b + b + b` on a `long long` is `b * 3`, and the pair's multiply
//! by a constant was 16-bit partial products (72 B, `mulu` per half) where gcc's pair of `add.l` and `addx.l`
//! is 28 B. From the 68020 the inline multiply is shorter than the chain and stays.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\
                  define i64 @f(i64 %b) {\n  %r = mul i64 %b, 3\n  ret i64 %r\n}\n";

#[test]
fn a_pair_times_three_below_the_68020_is_shifts_and_adds() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        for level in [Level::O1, Level::O2] {
            let asm = assembly(IR, cpu, level).expect("compiles");
            assert!(!asm.contains("mulu") && !asm.contains("__muldi3"), "{cpu:?} {level:?}: a multiply stayed:\n{asm}");
            assert!(asm.contains("addx.l"), "{cpu:?} {level:?}: no carry chain:\n{asm}");
        }
    }
}

#[test]
fn a_pair_times_three_from_the_68020_stays_the_multiply() {
    let asm = assembly(IR, Cpu::M68020, Level::O2).expect("compiles");
    assert!(asm.contains("mulu.l") || asm.contains("muls.l"), "the multiply went:\n{asm}");
}
