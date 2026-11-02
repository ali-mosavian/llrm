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
    run_everywhere(&sums_and_differences(), "the 64-bit sums and differences that differ from the host's");
}

/// `f()` must return 0 at both CPUs and both optimization goals.
fn run_everywhere(
    ir: &str,
    what: &str,
) {
    run_on(ir, what, &[(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")]);
}

fn run_on(
    ir: &str,
    what: &str,
    cpus: &[(Cpu, &str)],
) {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    for &(cpu, flag) in cpus {
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut all = start("f", &[]);
            all.extend(items_with(&module, "f", cpu, options).expect("compiles"));
            let path = support::unique_path("i64_carry");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &["-cpu", flag], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(0), "{cpu:?} size={}: {what}", options.size);
        }
    }
}

/// `f()` counts the pairs of `VALUES` for which `compute` (the IR of `%r{k}`
/// from `%p{k}` and `%q{k}`) differs from `expect`.
fn pairs(
    compute: impl Fn(usize) -> String,
    expect: impl Fn(u64, u64) -> u64,
) -> String {
    let mut body = String::from("define i32 @f() {\n  %x = alloca i64\n  %y = alloca i64\n  br label %b0\n");
    let mut bad = String::from("0");
    let mut block = 0;
    for &a in &VALUES {
        for &b in &VALUES {
            let k = block;
            body.push_str(&format!(
                "b{k}:\n  store volatile i64 {a}, ptr %x\n  store volatile i64 {b}, ptr %y\n  %p{k} = load volatile i64, ptr %x\n  %q{k} = load volatile i64, ptr %y\n{}  %e{k} = icmp ne i64 %r{k}, {}\n  %z{k} = zext i1 %e{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n",
                compute(k),
                expect(a, b)
            ));
            bad = format!("%s{k}");
            block += 1;
            body.push_str(&format!("  br label %b{block}\n"));
        }
    }
    body.push_str(&format!("b{block}:\n  ret i32 {bad}\n}}"));
    body
}

/// Each of these is wrong if a 0 or -1 flag is used as the bit 0 or 1.
#[test]
fn other_uses_of_a_64_bit_flag_match_the_host() {
    let cases: [(&str, Box<dyn Fn(usize) -> String>, Box<dyn Fn(u64, u64) -> u64>); 7] = [
        (
            "uadd.with.overflow flag",
            Box::new(|k| {
                format!(
                    "  %o{k} = call {{ i64, i1 }} @llvm.uadd.with.overflow.i64(i64 %p{k}, i64 %q{k})\n  %f{k} = extractvalue {{ i64, i1 }} %o{k}, 1\n  %r{k} = zext i1 %f{k} to i64\n"
                )
            }),
            Box::new(|a, b| u64::from(a.checked_add(b).is_none())),
        ),
        (
            "usub.with.overflow flag",
            Box::new(|k| {
                format!(
                    "  %o{k} = call {{ i64, i1 }} @llvm.usub.with.overflow.i64(i64 %p{k}, i64 %q{k})\n  %f{k} = extractvalue {{ i64, i1 }} %o{k}, 1\n  %r{k} = zext i1 %f{k} to i64\n"
                )
            }),
            Box::new(|a, b| u64::from(a.checked_sub(b).is_none())),
        ),
        (
            "uadd.with.overflow sum",
            Box::new(|k| {
                format!(
                    "  %o{k} = call {{ i64, i1 }} @llvm.uadd.with.overflow.i64(i64 %p{k}, i64 %q{k})\n  %r{k} = extractvalue {{ i64, i1 }} %o{k}, 0\n"
                )
            }),
            Box::new(|a, b| a.wrapping_add(b)),
        ),
        (
            "zext icmp ult",
            Box::new(|k| format!("  %c{k} = icmp ult i64 %p{k}, %q{k}\n  %r{k} = zext i1 %c{k} to i64\n")),
            Box::new(|a, b| u64::from(a < b)),
        ),
        (
            "select icmp slt",
            Box::new(|k| format!("  %c{k} = icmp slt i64 %p{k}, %q{k}\n  %r{k} = select i1 %c{k}, i64 7, i64 9\n")),
            Box::new(|a, b| if (a as i64) < (b as i64) { 7 } else { 9 }),
        ),
        (
            "shl by variable",
            Box::new(|k| format!("  %n{k} = and i64 %q{k}, 63\n  %r{k} = shl i64 %p{k}, %n{k}\n")),
            Box::new(|a, b| a << (b & 63)),
        ),
        (
            "lshr by variable",
            Box::new(|k| format!("  %n{k} = and i64 %q{k}, 63\n  %r{k} = lshr i64 %p{k}, %n{k}\n")),
            Box::new(|a, b| a >> (b & 63)),
        ),
    ];
    for (what, compute, expect) in cases {
        let ir = format!(
            "declare {{ i64, i1 }} @llvm.uadd.with.overflow.i64(i64, i64)\ndeclare {{ i64, i1 }} @llvm.usub.with.overflow.i64(i64, i64)\n{}",
            pairs(compute, expect)
        );
        run_on(&ir, what, &[(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")]);
    }
}
