//! A 64-bit add or subtract whose low halves carry or borrow.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// Around the carry out of the low word, and the extremes.
const VALUES: [u64; 8] =
    [0, 1, u64::MAX, 0xffff_ffff, 0x1_0000_0000, 0x1_ffff_ffff, 0x8000_0000_0000_0000, 0x7fff_ffff_ffff_ffff];

/// `f()` counts the sums and differences of every pair, taken from volatile
/// slots so that nothing folds, that differ from the host's wrapping ones.
fn sums_and_differences() -> String {
    let mut body = String::from("define i32 @f() {\n  %x = alloca i64\n  %y = alloca i64\n  br label %b0\n");
    let mut block = 0;
    let mut bad = String::from("0");
    for (i, &a) in VALUES.iter().enumerate() {
        for (j, &b) in VALUES.iter().enumerate() {
            for (op, want) in [("add", a.wrapping_add(b)), ("sub", a.wrapping_sub(b))] {
                let k = format!("{i}_{j}_{op}");
                body.push_str(&format!(
                    "b{block}:\n  store volatile i64 {a}, ptr %x\n  store volatile i64 {b}, ptr %y\n  %p{k} = load volatile i64, ptr %x\n  %q{k} = load volatile i64, ptr %y\n  %r{k} = {op} i64 %p{k}, %q{k}\n  %e{k} = icmp ne i64 %r{k}, {want}\n  %z{k} = zext i1 %e{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n"
                ));
                bad = format!("%s{k}");
                block += 1;
                body.push_str(&format!("  br label %b{block}\n"));
            }
        }
    }
    body.push_str(&format!("b{block}:\n  ret i32 {bad}\n}}"));
    body
}

/// The carry of the low words came from `scs` (0xFF) zero-extended to a long
/// with no mask: `add64(0xffffffff, 1)` had 255 in the high word, wrong for
/// 836 of 3000 random pairs of adds and 1440 of subtracts (softfloat's f64
/// harness). The pin selects 1 or 0 when the target's booleans are not 0 or 1.
#[test]
fn the_carry_and_borrow_of_a_64_bit_add_and_subtract_are_one() {
    let Some(qemu) = support::qemu() else { return };
    let ir = sums_and_differences();
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            all.extend(items_with(&module, "f", cpu, options).expect("compiles"));
            let path = support::unique_path("i64_carry");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(
                code,
                Some(0),
                "{cpu:?} size={}: the 64-bit sums and differences that differ from the host's",
                options.size
            );
        }
    }
}
