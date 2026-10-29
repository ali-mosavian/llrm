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

/// A join's phi read only by other phis (the loop's and the exit's) was never
/// named by a node of its block, so it got no register and its inputs were
/// dropped: the loop counted nothing and returned its start value.
#[test]
fn a_phi_read_only_by_other_phis_keeps_its_inputs() {
    let ir = "define i32 @f(i32 %n) {
entry:
  br label %l
l:
  %i = phi i32 [0, %entry], [%i2, %j]
  %c = phi i32 [0, %entry], [%c2, %j]
  %odd = and i32 %i, 1
  %t = icmp eq i32 %odd, 0
  br i1 %t, label %a, label %j
a:
  %c1 = add i32 %c, 1
  br label %j
j:
  %c2 = phi i32 [%c1, %a], [%c, %l]
  %i2 = add i32 %i, 1
  %d = icmp eq i32 %i2, %n
  br i1 %d, label %x, label %l
x:
  %r = phi i32 [%c2, %j]
  ret i32 %r
}";
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status(ir, &[7], options) {
            assert_eq!(code, 4, "{options:?}");
        }
    }
}

/// The low byte of an `i32` argument used in another block: `EXTRACT_SUBREG`
/// had no lowering and the function did not compile; narrowed to the argument's
/// slot the byte read must be the low one (big-endian: the slot's last byte).
#[test]
fn the_low_byte_of_an_argument_read_in_another_block_is_the_low_byte() {
    let ir = "define i32 @f(i32 %c) {
entry:
  %b = trunc i32 %c to i8
  br label %x
x:
  %z = zext i8 %b to i32
  ret i32 %z
}";
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status(ir, &[0x1234], options) {
            assert_eq!(code, 0x34, "{options:?}: {}", text_with(&module(ir), "f", Cpu::M68000, options).unwrap());
        }
    }
}

/// Exit status of `k(args)` over the library data block.
fn status_with_data(
    ir: &str,
    args: &[llrm_m68k::kernels::Arg],
    options: Options,
) -> Option<i32> {
    let qemu = qemu()?;
    let code = items_with(&module(ir), "k", Cpu::M68000, options).expect("compiles");
    let program = llrm_m68k::kernels::program_with_data(args, code, &llrm_m68k::kernels::library_data());
    let path = std::env::temp_dir().join(format!("llrm-m68k-results-data-{}-{}", std::process::id(), ir.len()));
    write_executable(&path, &executable(&program).unwrap()).unwrap();
    let status = Command::new("timeout").args(["10"]).arg(qemu).arg(&path).status().unwrap().code();
    std::fs::remove_file(&path).ok();
    status
}

/// A byte loaded from the address a long was just stored to is the long's
/// first byte, its most significant: the store was forwarded as if the byte
/// were the least significant one (the struct copy of a library routine added
/// the wrong byte).
#[test]
fn a_byte_loaded_back_from_a_stored_long_is_its_most_significant_byte() {
    use llrm_m68k::kernels::Arg::{Data, Imm};
    let ir = "define i32 @k(ptr %p, i32 %x) {
  store i32 %x, ptr %p
  %v = load i8, ptr %p
  %z = zext i8 %v to i32
  ret i32 %z
}";
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status_with_data(ir, &[Data(16), Imm(0x11223344)], options) {
            assert_eq!(code, 0x11, "{options:?}: {}", text_with(&module(ir), "k", Cpu::M68000, options).unwrap());
        }
    }
}

/// A byte constant moved in another block (`MOVI8di`) had no expansion, and a
/// 200 must stay 200 after the zero extension.
#[test]
fn a_byte_constant_chosen_in_a_branch_keeps_its_value() {
    let ir = "define i32 @f(i32 %a) {
entry:
  %c = icmp eq i32 %a, 0
  br i1 %c, label %x, label %y
x:
  br label %j
y:
  br label %j
j:
  %v = phi i8 [200, %x], [7, %y]
  %z = zext i8 %v to i32
  ret i32 %z
}";
    for (arg, want) in [(0, 200), (1, 7)] {
        if let Some(code) = status(ir, &[arg], Options::default()) {
            assert_eq!(code, want, "f({arg})");
        }
    }
}

/// A function with a stack array: the frame had no room for static allocas
/// ("an unknown frame index"), so no routine with a local buffer compiled.
#[test]
fn a_local_array_is_a_frame_object() {
    let ir = "define i32 @f(i32 %n) {
entry:
  %a = alloca [4 x i32]
  br label %l
l:
  %i = phi i32 [0, %entry], [%i1, %l]
  %p = getelementptr [4 x i32], ptr %a, i32 0, i32 %i
  %v = add i32 %n, %i
  store i32 %v, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, 4
  br i1 %c, label %l, label %s
s:
  %q = getelementptr [4 x i32], ptr %a, i32 0, i32 3
  %r = load i32, ptr %q
  ret i32 %r
}";
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status(ir, &[10], options) {
            assert_eq!(code, 13, "{options:?}: {}", text_with(&module(ir), "f", Cpu::M68000, options).unwrap());
        }
    }
}

/// A pointer stepped in another block than the load through it (copies in
/// between, the phi's) went `move.l %a0, %d2; addq.l #1, %d2; move.l %d2, %a0`
/// for want of seeing the copies as one value: now `addq.l #1, %a0`.
#[test]
fn a_pointer_stepped_in_another_block_is_stepped_in_its_address_register() {
    use llrm_m68k::kernels::Arg::{Data, Imm};
    let ir = "define i32 @k(ptr %p, i32 %n) {
entry:
  br label %o
o:
  %q = phi ptr [%p, %entry], [%q1, %c]
  %m = phi i32 [%n, %entry], [%m1, %c]
  %s = phi i32 [0, %entry], [%a1, %c]
  %v = load i8, ptr %q
  %z = zext i8 %v to i32
  br label %in
in:
  %j = phi i32 [0, %o], [%j1, %in]
  %a = phi i32 [%s, %o], [%a1, %in]
  %a1 = add i32 %a, %z
  %j1 = add i32 %j, 1
  %d = icmp eq i32 %j1, 3
  br i1 %d, label %c, label %in
c:
  %q1 = getelementptr i8, ptr %q, i32 1
  %m1 = add i32 %m, -1
  %e = icmp eq i32 %m1, 0
  br i1 %e, label %x, label %o
x:
  ret i32 %a1
}";
    let options = Options { mid_end: false, ..Options::default() };
    let asm = text_with(&module(ir), "k", Cpu::M68000, options).unwrap();
    assert!(asm.contains("addq.l\t#1, %a") && !asm.contains("move.l\t%a0, %d"), "{asm}");
    if let Some(code) = status_with_data(ir, &[Data(0), Imm(3)], options) {
        assert_eq!(code, (313 * 3) & 0xFF, "{asm}");
    }
}
