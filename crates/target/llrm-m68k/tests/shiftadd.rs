//! `lsl #1, Dn` is `add Dn, Dn`: the same result and the same carry, extend,
//! negative and zero codes, two clocks fewer on the 68020 (crc16 -O2 4162 ->
//! 4018 clk on the 68000, 1875 -> 1731 on the 68020). The overflow code
//! differs, so a reader of it keeps the shift.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn compiled(body: &str) -> String {
    assembly(&format!("{IR}{body}"), cpu_of("68020").unwrap(), Level::O2).expect("compiles")
}

#[test]
fn a_shift_by_one_whose_codes_nothing_reads_is_an_add() {
    let out = compiled("define i32 @f(i32 %x) {\n  %r = shl i32 %x, 1\n  ret i32 %r\n}\n");
    assert!(out.contains("add.l\t%d0, %d0") && !out.contains("lsl"), "{out}");
}

#[test]
fn a_zero_test_reads_the_adds_codes_as_it_read_the_shifts() {
    let out = compiled(
        "define i32 @f(i32 %x) {\n  %r = shl i32 %x, 1\n  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z\n}\n",
    );
    assert!(out.contains("add.l\t%d0, %d0") && !out.contains("tst"), "{out}");
}

#[test]
fn a_signed_test_of_the_result_keeps_the_shift_for_its_overflow_code() {
    let out = compiled(
        "define i32 @f(i32 %x) {\n  %r = shl i32 %x, 1\n  %c = icmp sgt i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z\n}\n",
    );
    assert!(out.contains("lsl.l\t#1, %d0") && !out.contains("tst"), "{out}");
}
