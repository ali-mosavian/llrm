//! `(hi << 1) | (lo >> 63)` at -Os is `llvm.fshl.i64`, which the type
//! legalizer did not expand (`Unsupported { FShl, I64, "expand result" }`):
//! softfloat's `__divdf3` with `-DSF_DIV_NEWTON` did not compile.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const VALUES: [u64; 3] = [0x0123_4567_89ab_cdef, u64::MAX, 0x8000_0000_0000_0001];
/// Under and over each half, the wrap at 64 and the 0 that returns the first
/// operand.
const AMOUNTS: [u64; 9] = [0, 1, 31, 32, 33, 63, 64, 65, 127];

fn fshl(
    a: u64,
    b: u64,
    n: u64,
) -> u64 {
    let n = (n % 64) as u32;
    if n == 0 { a } else { (a << n) | (b >> (64 - n)) }
}

fn fshr(
    a: u64,
    b: u64,
    n: u64,
) -> u64 {
    let n = (n % 64) as u32;
    if n == 0 { b } else { (a << (64 - n)) | (b >> n) }
}

/// `f()` counts the funnel shifts, with the amount from a volatile slot, that
/// differ from the host's.
fn funnel_shifts() -> String {
    let mut body = String::from(
        "declare i64 @llvm.fshl.i64(i64, i64, i64)\ndeclare i64 @llvm.fshr.i64(i64, i64, i64)\ndefine i32 @f() {\n  %x = alloca i64\n  br label %b0\n",
    );
    let (mut bad, mut k) = (String::from("0"), 0);
    for &a in &VALUES {
        for &b in &VALUES {
            for &n in &AMOUNTS {
                for (name, want) in [("fshl", fshl(a, b, n)), ("fshr", fshr(a, b, n))] {
                    body.push_str(&format!(
                        "b{k}:\n  store volatile i64 {n}, ptr %x\n  %n{k} = load volatile i64, ptr %x\n  %r{k} = call i64 @llvm.{name}.i64(i64 {a}, i64 {b}, i64 %n{k})\n  %e{k} = icmp ne i64 %r{k}, {want}\n  %z{k} = zext i1 %e{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n  br label %b{}\n",
                        k + 1
                    ));
                    bad = format!("%s{k}");
                    k += 1;
                }
            }
        }
    }
    body.push_str(&format!("b{k}:\n  ret i32 {bad}\n}}"));
    body
}

#[test]
fn a_64_bit_funnel_shift_by_a_variable_matches_the_host() {
    let Some(qemu) = support::qemu() else { return };
    let ir = funnel_shifts();
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            all.extend(items_with(&module, "f", cpu, options).expect("compiles"));
            let path = support::unique_path("i64_funnel");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(0), "{cpu:?} size={}: funnel shifts that differ from the host's", options.size);
        }
    }
}
