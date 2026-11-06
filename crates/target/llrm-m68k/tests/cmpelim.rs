//! A `tst` the instruction before it makes redundant is dropped, and one whose
//! codes differ from `tst`'s for the branch that reads them is kept. Each case
//! runs under qemu: a dropped test that was needed branches the wrong way.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

fn text(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

fn status(
    ir: &str,
    args: &[i64],
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("cmpelim");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

/// A load that is compared with zero and branched on was `move.l (4,%sp), %d1;
/// tst.l %d1; ble`: the move sets the codes.
#[test]
fn a_loaded_value_compared_with_zero_is_not_tested_again() {
    let ir = "define i32 @f(i32 %n) {
  %c = icmp sgt i32 %n, 0
  %r = select i1 %c, i32 %n, i32 5
  ret i32 %r
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(ir, cpu);
        assert!(!out.contains("tst") && !out.contains("cmp"), "{cpu:?}:\n{out}");
    }
}

/// `a + b` against zero for an ordering reads V, which an add computes from
/// its operands and `tst` clears: the test stays.
#[test]
fn an_add_does_not_stand_in_for_a_test_that_an_ordering_reads() {
    let ir = "define i32 @f(i32 %a, i32 %b) {
  %s = add i32 %a, %b
  %c = icmp sgt i32 %s, 0
  %r = select i1 %c, i32 7, i32 3
  ret i32 %r
}";
    assert!(text(ir, Cpu::M68000).contains("tst.l") || text(ir, Cpu::M68000).contains("cmpi"));
}

/// `lsr` sets C to the last bit shifted out, so `x >> 1 != 0` read as
/// "higher" (C and Z clear) must keep its test: 3 >> 1 is 1, nonzero.
#[test]
fn a_shift_does_not_stand_in_for_a_test_that_reads_the_carry() {
    let ir = "define i32 @f(i32 %x) {
  %s = lshr i32 %x, 1
  %c = icmp ugt i32 %s, 0
  %r = zext i1 %c to i32
  ret i32 %r
}";
    for (x, want) in [(0, 0), (1, 0), (2, 1), (3, 1), (7, 1)] {
        if let Some(code) = status(ir, &[x]) {
            assert_eq!(code, want, "f({x})");
        }
    }
}

type Op = (&'static str, fn(u32, u32) -> u32);
const OPS: [Op; 6] = [
    ("add", |a, b| a.wrapping_add(b)),
    ("sub", |a, b| a.wrapping_sub(b)),
    ("and", |a, b| a & b),
    ("or", |a, b| a | b),
    ("xor", |a, b| a ^ b),
    ("lshr", |a, b| a >> (b & 31)),
];
/// Eight conditions on a result against zero, as bit `n` of the answer.
const CONDITIONS: [(&str, fn(i32) -> bool); 8] = [
    ("eq", |s| s == 0),
    ("ne", |s| s != 0),
    ("slt", |s| s < 0),
    ("sle", |s| s <= 0),
    ("sgt", |s| s > 0),
    ("sge", |s| s >= 0),
    ("ugt", |s| s != 0),
    ("ule", |s| s == 0),
];

/// Every operation against zero under every condition at once, over values at
/// the edges: bit `n` of the answer is condition `n` on the result, whether or
/// not the test was dropped.
#[test]
fn a_result_compared_with_zero_branches_as_the_value_says() {
    for (op, apply) in OPS {
        let mut body = format!("  %s = {op} i32 %a, %b\n  %acc0 = add i32 0, 0\n");
        for (at, (cond, _)) in CONDITIONS.iter().enumerate() {
            body += &format!(
                "  %c{at} = icmp {cond} i32 %s, 0\n  %z{at} = zext i1 %c{at} to i32\n  %w{at} = shl i32 %z{at}, {at}\n  %acc{} = or i32 %acc{at}, %w{at}\n",
                at + 1
            );
        }
        let ir = format!("define i32 @f(i32 %a, i32 %b) {{\n{body}  ret i32 %acc{}\n}}", CONDITIONS.len());
        for (a, b) in [(0u32, 0u32), (1, 0), (3, 1), (0x8000_0000, 1), (0x7fff_ffff, 1), (5, 5), (0xffff_ffff, 1)] {
            let result = apply(a, b) as i32;
            let want: i32 = CONDITIONS.iter().enumerate().map(|(at, (_, holds))| i32::from(holds(result)) << at).sum();
            if let Some(code) = status(&ir, &[i64::from(a as i32), i64::from(b as i32)]) {
                assert_eq!(code, want, "{op} ({a:#x}, {b:#x})");
            }
        }
    }
}
