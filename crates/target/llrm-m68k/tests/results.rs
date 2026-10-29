//! Overlay rewrites against their boundaries: the text each one gives, and the
//! value the program computes under `qemu-m68k` (the exit status, 8 bits).

use std::path::PathBuf;
use std::process::Command;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn qemu() -> Option<PathBuf> {
    ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists())
}

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Exit status of `f(args)` compiled with `options`; `None` without qemu.
fn status(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = qemu().or_else(|| {
        eprintln!("qemu-m68k is not installed: not run");
        None
    })?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, options).expect("compiles"));
    let path = std::env::temp_dir().join(format!("llrm-m68k-results-{}-{}", std::process::id(), ir.len()));
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = Command::new("timeout").args(["10"]).arg(qemu).arg(&path).status().unwrap().code();
    std::fs::remove_file(&path).ok();
    code
}

/// `(a & c) + a`: `move.l a, d; and.l #c, d` becomes `moveq #c, d; and.l a, d`
/// only where `c` fits `moveq`'s signed byte. 127 and -128 do, 128 and -129 do
/// not (`moveq #128` would load -128), and the value is the same either way.
#[test]
fn a_mask_is_a_moveq_only_within_its_signed_byte() {
    for (mask, quick) in [(127, true), (128, false), (-128, true), (-129, false)] {
        let ir =
            format!("define i32 @f(i32 %a) {{\n  %m = and i32 %a, {mask}\n  %r = add i32 %m, %a\n  ret i32 %r\n}}");
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert_eq!(asm.contains(&format!("moveq\t#{mask},")), quick, "mask {mask}: {asm}");
        if let Some(code) = status(&ir, &[0x85], Options::default()) {
            assert_eq!(code, ((0x85 & mask) + 0x85) & 0xFF, "mask {mask}: {asm}");
        }
    }
}

fn code_size(
    ir: &str,
    options: Options,
) -> usize {
    let items = items_with(&module(ir), "f", Cpu::M68000, options).unwrap();
    llrm_m68k::object::assemble(&items, 0).unwrap().code.len()
}

/// `s + (a > b) + a` was `slt %d2; and.l #1, %d2; add.l %d2, %d0`: 6 bytes and
/// 16 clocks for the mask. `moveq #0, %d2` before the compare and `neg.b %d2`
/// after the `slt` are 4 bytes and 8 clocks, and the value is the same for
/// either outcome.
#[test]
fn a_boolean_added_is_negated_not_masked() {
    let ir = "define i32 @f(i32 %a, i32 %b, i32 %s) {\n  %c = icmp sgt i32 %a, %b\n  %z = zext i1 %c to i32\n  %t = add i32 %s, %z\n  %r = add i32 %t, %a\n  ret i32 %r\n}";
    let pinned = text_with(&module(ir), "f", Cpu::M68000, Options::PINNED).unwrap();
    assert!(pinned.contains("and.l\t#1,") && !pinned.contains("neg.b"), "{pinned}");
    let ours = text_with(&module(ir), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(ours.contains("neg.b") && !ours.contains("and.l\t#1,"), "{ours}");
    assert!(code_size(ir, Options::default()) < code_size(ir, Options::PINNED));
    for (a, b, s) in [(5, 3, 10), (3, 5, 10), (4, 4, 10), (-1, -2, 7), (-2, -1, 7)] {
        if let Some(code) = status(ir, &[a, b, s], Options::default()) {
            assert_eq!(code, (s as i32 + i32::from(a > b) + a as i32) & 0xFF, "f({a}, {b}, {s}): {ours}");
        }
    }
}

/// Binary search over `a[m] = 2m + 1`, no memory: two selects on one compare,
/// the second reading a loop-carried copy.
const SEARCH: &str = "define i32 @f(i32 %n, i32 %key) {
entry:
  %n1 = add i32 %n, -1
  %e = icmp slt i32 %n, 1
  br i1 %e, label %none, label %loop
loop:
  %hi = phi i32 [%h0, %next], [%n1, %entry]
  %lo = phi i32 [%lo2, %next], [0, %entry]
  %s = add i32 %hi, %lo
  %m = lshr i32 %s, 1
  %w = shl i32 %m, 1
  %v = add i32 %w, 1
  %hit = icmp eq i32 %v, %key
  br i1 %hit, label %found, label %next
next:
  %lt = icmp slt i32 %v, %key
  %up = add i32 %m, 1
  %down = add i32 %m, -1
  %lo2 = select i1 %lt, i32 %up, i32 %lo
  %h0 = select i1 %lt, i32 %hi, i32 %down
  %done = icmp sgt i32 %lo2, %h0
  br i1 %done, label %none, label %loop
found:
  ret i32 %m
none:
  ret i32 -1
}";

/// Two selects on one compare, the second naming a copy of a value from before
/// the compare, were lowered as two diamonds, and the first diamond's arm
/// (`addq`) cleared the codes the second branched on: the search returned -1
/// for a key that is there.
#[test]
fn two_selects_on_one_compare_share_one_branch() {
    for (key, index) in [(21, 10), (1, 0), (31, 15), (22, -1)] {
        if let Some(code) = status(SEARCH, &[16, key], Options::default()) {
            assert_eq!(code, index & 0xFF, "key {key}");
        }
    }
}
