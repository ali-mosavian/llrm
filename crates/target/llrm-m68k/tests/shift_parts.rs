//! `shl`, `lshr` and `ashr` of an `i64` by a variable count: the shift-parts
//! nodes were declared custom with no lowering, so every one was refused
//! (`Unsupported ShlParts`, softfloat's `__fixsfdi`).

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";
const X: u64 = 0x9234_5678_9abc_def0;

/// The shift of `X` by `n`, its eight bytes xored into one.
fn module(op: &str) -> llrm_mir::module::Module {
    let mut body = format!("define i32 @f(i32 %n) {{\n  %c = zext i32 %n to i64\n  %r = {op} i64 {}, %c\n", X as i64);
    let mut last = "%r".to_owned();
    for byte in 1..8 {
        body.push_str(&format!("  %s{byte} = lshr i64 %r, {}\n  %x{byte} = xor i64 {last}, %s{byte}\n", byte * 8));
        last = format!("%x{byte}");
    }
    body.push_str(&format!(
        "  %t = trunc i64 {last} to i32\n  %m = and i32 %t, 255\n  %b = xor i32 %m, 33\n  ret i32 %b\n}}"
    ));
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{body}")).expect("parses")
}

fn fold(value: u64) -> i32 {
    // 124 is the harness's timeout code: the xor keeps every case clear of it
    ((0..8).fold(0, |all, byte| all ^ (value >> (byte * 8))) as i32 & 255) ^ 33
}

#[test]
fn a_64_bit_shift_by_a_variable_count_agrees_with_the_host_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    for (op, shift) in [
        ("shl", (|x: u64, n: u32| x << n) as fn(u64, u32) -> u64),
        ("lshr", |x, n| x >> n),
        ("ashr", |x, n| ((x as i64) >> n) as u64),
    ] {
        let module = module(op);
        for n in [0_u32, 1, 5, 16, 31, 32, 33, 47, 63] {
            let mut all = start("f", &[i64::from(n)]);
            all.extend(
                items_with(&module, "f", Cpu::M68020, Options { mid_end: false, ..Options::default() }).unwrap(),
            );
            let path = support::unique_path("shiftparts");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", "m68020"], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(fold(shift(X, n))), "{op} by {n}");
        }
    }
}
