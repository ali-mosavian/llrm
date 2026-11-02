//! The double-word shifts and compares were `Unsupported { ShlParts }` and
//! `{ SetCCCarry }` ("convert node to libcall"): the target did not lower the
//! custom nodes the type legalizer makes, so every C program with a 64-bit
//! shift by a variable or an ordered 64-bit compare failed to compile. The
//! first had `move.w %ccr` copies once it did (a 68010 instruction) until the
//! scheduler cloned the flag definition as the pin's does. The values run under
//! qemu; the divide and conversion routines are called, not run.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

const PATTERNS: [u64; 3] = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210, 0x8000_0000_0000_0001];

/// Either side of 0, 1, 31, 32 and 33, and the widest.
const AMOUNTS: [u32; 9] = [0, 1, 5, 16, 31, 32, 33, 48, 63];

/// `f()` counts the shifts of each pattern by the amounts above, taken from a
/// volatile slot so that the amount is not a constant, that differ from the
/// value worked out here.
fn shifts() -> String {
    let mut body = String::from("define i32 @f() {\n  %slot = alloca i32\n  %pat = alloca i64\n  br label %b0\n");
    let mut block = 0;
    let mut bad = String::from("0");
    for (p, &pattern) in PATTERNS.iter().enumerate() {
        for n in AMOUNTS {
            for (name, want) in
                [("shl", pattern << n), ("lshr", pattern >> n), ("ashr", ((pattern as i64) >> n) as u64)]
            {
                let k = format!("{p}_{n}_{name}");
                body.push_str(&format!(
                    "b{block}:\n  store volatile i32 {n}, ptr %slot\n  store volatile i64 {}, ptr %pat\n  %m{k} = load volatile i32, ptr %slot\n  %a{k} = load volatile i64, ptr %pat\n  %w{k} = zext i32 %m{k} to i64\n  %r{k} = {name} i64 %a{k}, %w{k}\n  %c{k} = icmp ne i64 %r{k}, {}\n  %z{k} = zext i1 %c{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n",
                    pattern as i64, want as i64
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

#[test]
fn variable_shifts_of_an_i64_give_the_right_words() {
    let Some(qemu) = support::qemu() else { return };
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[]);
        all.extend(items_with(&module(&shifts()), "f", cpu, Options::default()).expect("compiles"));
        let path = support::unique_path("i64_shifts");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{cpu:?}: the number of i64 shifts that differ from the host's");
    }
}

const VALUES: [i64; 5] = [0, 1, -1, 0x1_0000_0000, i64::MIN];

/// Every ordered compare of every pair, against the host's.
fn compares() -> String {
    let mut body = String::from("define i32 @f() {\n  %x = alloca i64\n  %y = alloca i64\n  br label %b0\n");
    let mut block = 0;
    let mut bad = String::from("0");
    for (i, &a) in VALUES.iter().enumerate() {
        for (j, &b) in VALUES.iter().enumerate() {
            for (pred, want) in [
                ("slt", a < b),
                ("sle", a <= b),
                ("sgt", a > b),
                ("sge", a >= b),
                ("ult", (a as u64) < (b as u64)),
                ("ule", (a as u64) <= (b as u64)),
                ("ugt", (a as u64) > (b as u64)),
                ("uge", (a as u64) >= (b as u64)),
            ] {
                let k = format!("{i}_{j}_{pred}");
                body.push_str(&format!(
                    "b{block}:\n  store volatile i64 {a}, ptr %x\n  store volatile i64 {b}, ptr %y\n  %p{k} = load volatile i64, ptr %x\n  %q{k} = load volatile i64, ptr %y\n  %c{k} = icmp {pred} i64 %p{k}, %q{k}\n  %e{k} = icmp ne i1 %c{k}, {want}\n  %z{k} = zext i1 %e{k} to i32\n  %s{k} = add i32 {bad}, %z{k}\n"
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

#[test]
fn ordered_compares_of_an_i64_give_the_hosts_answers() {
    let Some(qemu) = support::qemu() else { return };
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        let mut all = start("f", &[]);
        all.extend(items_with(&module(&compares()), "f", cpu, Options::default()).expect("compiles"));
        let path = support::unique_path("i64_compares");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", flag], &path);
        std::fs::remove_file(&path).ok();
        assert_eq!(code, Some(0), "{cpu:?}: the number of i64 compares that differ from the host's");
    }
}

/// Neither the 68000 nor the shift or compare of a double word may copy the
/// codes through a register: `move %ccr, Dn` is a 68010 instruction.
#[test]
fn a_double_word_shift_and_compare_do_not_copy_the_codes() {
    let ir = "define i64 @f(i64 %a, i32 %n, i64 %b) {
  %m = zext i32 %n to i64
  %s = shl i64 %a, %m
  %c = icmp ult i64 %s, %b
  %r = select i1 %c, i64 %s, i64 %b
  ret i64 %r
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(ir), "f", cpu, Options::default()).expect("compiles");
        assert!(!out.contains("%ccr"), "{cpu:?}:\n{out}");
    }
}

/// The routines the pin calls for a double-word divide and a float conversion.
#[test]
fn double_word_divides_and_remainders_call_the_runtime() {
    for (op, routine) in [("sdiv", "__divdi3"), ("udiv", "__udivdi3"), ("srem", "__moddi3"), ("urem", "__umoddi3")] {
        let ir = format!("define i64 @f(i64 %a, i64 %b) {{ %r = {op} i64 %a, %b ret i64 %r }}");
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let out = text_with(&module(&ir), "f", cpu, Options::default()).expect("compiles");
            assert!(out.contains(&format!("jsr\t{routine}")), "{cpu:?} {op}:\n{out}");
        }
    }
}
