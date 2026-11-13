//! object_extent_loop(_f32) and nbody_single (18 cells, +66..182 B against gcc): `(i + j) * 0.5f` over loop counters is
//! a `uitofp nneg`, which called `__floatunsisf` beside the `__floatsisf` the program also needs; gcc calls the signed
//! routine for a value it knows is not negative, so one conversion routine is linked, not two.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn calls(body: &str) -> Vec<String> {
    let ir = format!("target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n{body}\n");
    let asm = assembly(&ir, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    asm.lines().filter_map(|line| line.trim().strip_prefix("jsr\t").map(str::to_owned)).collect()
}

#[test]
fn a_value_known_not_negative_converts_with_the_signed_routine() {
    assert_eq!(calls("define float @f(i32 %x) {\n  %r = uitofp nneg i32 %x to float\n  ret float %r\n}"), ["__floatsisf"]);
    assert_eq!(
        calls("define float @f(i32 %x) {\n  %m = and i32 %x, 255\n  %r = uitofp i32 %m to float\n  ret float %r\n}"),
        ["__floatsisf"]
    );
    assert_eq!(calls("define double @f(i8 %x) {\n  %r = uitofp i8 %x to double\n  ret double %r\n}"), ["__floatsidf"]);
}

#[test]
fn an_unsigned_value_that_may_have_its_top_bit_keeps_the_unsigned_routine() {
    assert_eq!(calls("define float @f(i32 %x) {\n  %r = uitofp i32 %x to float\n  ret float %r\n}"), ["__floatunsisf"]);
}
