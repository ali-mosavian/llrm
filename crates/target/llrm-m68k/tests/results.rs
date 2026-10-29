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

/// A byte copy that steps both pointers inside its `move` ran, once its text
/// said `(%a2)+` for the destination, with a second step on the destination
/// the text did not show; the copy must land byte for byte.
#[test]
fn a_byte_copy_through_two_post_incremented_pointers_copies_every_byte() {
    use llrm_m68k::kernels::Arg::{Data, Imm};
    let ir = include_str!("../bench/library/memcpy.ll");
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status_with_data(ir, &[Data(128), Data(0), Imm(14)], options) {
            assert_eq!(code, i32::from(b'!'), "{options:?}");
        }
    }
}

/// Exit status of `f(args)` with the runtime of `bench/library/rt.ll` behind it.
fn status_with_runtime(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let qemu = qemu()?;
    let mut all = start("f", args);
    // one module, so the local labels of the routines do not meet
    let rt = include_str!("../bench/library/rt.ll");
    let runtime = module(&format!("{ir}\n{}", rt.split_once('\n').map_or("", |(_, rest)| rest)));
    all.extend(items_with(&runtime, "f", Cpu::M68000, options).expect("compiles"));
    for name in ["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"] {
        all.extend(items_with(&runtime, name, Cpu::M68000, options).expect("the runtime compiles"));
    }
    let path = std::env::temp_dir().join(format!(
        "llrm-m68k-results-rt-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = Command::new("timeout").args(["10"]).arg(qemu).arg(&path).status().unwrap().code();
    std::fs::remove_file(&path).ok();
    code
}

/// A 32-bit multiply, divide or remainder on the 68000 had no instruction and
/// no conversion to the runtime call, so no routine that used one compiled
/// (`muldiv` of the library set). Each operation, over operands of every sign,
/// with the operands in both orders (the divide does not commute).
#[test]
fn a_wide_multiply_divide_or_remainder_calls_the_runtime() {
    let operations: [(&str, fn(i32, i32) -> i32); 5] = [
        ("mul", i32::wrapping_mul),
        ("sdiv", i32::wrapping_div),
        ("udiv", |a, b| ((a as u32) / (b as u32)) as i32),
        ("srem", i32::wrapping_rem),
        ("urem", |a, b| ((a as u32) % (b as u32)) as i32),
    ];
    let pairs = [(1000, 7), (-1000, 7), (1000, -7), (-1000, -7), (123_456_789, 1000), (7, 1000), (-5, 3)];
    let mut wrong = Vec::new();
    for (op, expect) in operations {
        let ir = format!("define i32 @f(i32 %a, i32 %b) {{\n  %r = {op} i32 %a, %b\n  ret i32 %r\n}}");
        for (a, b) in pairs {
            for options in [Options::default(), Options::PINNED] {
                if let Some(code) = status_with_runtime(&ir, &[i64::from(a), i64::from(b)], options) {
                    if code != expect(a, b) & 255 {
                        wrong.push(format!("{op} {a} {b} {options:?}: got {code}, want {}", expect(a, b) & 255));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The bench's `a * b + a / b + a % b`: the remainder is combined with the
/// divide, and the three calls keep their values across each other.
#[test]
fn a_function_with_three_runtime_calls_keeps_its_values() {
    let ir = include_str!("../bench/library/muldiv.ll").replace("@k", "@f");
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status_with_runtime(&ir, &[1000, 7], options) {
            assert_eq!(code, 236, "{options:?}");
        }
    }
}

/// A condition read past a select got the codes of whatever ran last: the
/// arm of the select (`move` and `sub` set them), so `zext (icmp)` beside
/// `select (icmp)` (the quotient bit of a division loop) was the codes of
/// the subtract. Four conditions, so no one flag's value covers it; the
/// operands make the carry of the add differ from the compare's.
#[test]
fn a_flag_reader_past_a_select_reads_the_codes_of_the_compare() {
    type Model = fn(i32, i32) -> bool;
    let conditions: [(&str, Model); 4] = [
        ("uge", |n, d| n as u32 >= d as u32),
        ("sgt", |n, d| n > d),
        ("slt", |n, d| n < d),
        ("ult", |n, d| (n as u32) < d as u32),
    ];
    let triples = [(10, 3, 5), (3, 10, 5), (-4, 6, 9), (6, -4, 9), (200, 200, 1), (i32::MAX, 1, 7), (-1, 1, 2)];
    let mut wrong = Vec::new();
    for (cond, model) in conditions {
        let ir = format!(
            "define i32 @f(i32 %n, i32 %d, i32 %m) {{
  %c = icmp {cond} i32 %n, %d
  %sum = add i32 %n, %d
  %r = select i1 %c, i32 %sum, i32 %n
  %m2 = shl i32 %m, 1
  %bit = zext i1 %c to i32
  %o = or i32 %m2, %bit
  %t = add i32 %o, %r
  ret i32 %t
}}"
        );
        for (n, d, m) in triples {
            let want = ((m << 1) | i32::from(model(n, d))).wrapping_add(if model(n, d) { n.wrapping_add(d) } else { n });
            for options in [Options::default(), Options::PINNED] {
                if let Some(code) = status(&ir, &[i64::from(n), i64::from(d), i64::from(m)], options) {
                    if code != want & 255 {
                        wrong.push(format!("{cond} {n} {d} {m} {options:?}: got {code}, want {}", want & 255));
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Two selects on one compare with a call between: the second read the codes
/// the call left (`neg` of the result), so `|a| * b` negated back by the sign
/// of `a` (the signed remainder of the runtime) was wrong for a positive `a`.
/// The compare runs again for the second select, as the pin's scheduler does.
/// Another condition and a different call than the motivating one.
#[test]
fn a_second_select_on_a_compare_after_a_call_compares_again() {
    for cond in ["slt", "ult", "sgt"] {
        let ir = format!(
            "define i32 @f(i32 %a, i32 %b) {{
  %c = icmp {cond} i32 %a, %b
  %x = select i1 %c, i32 %a, i32 %b
  %p = call i32 @__mulsi3(i32 %x, i32 3)
  %q = sub i32 0, %p
  %r = select i1 %c, i32 %q, i32 %p
  ret i32 %r
}}"
        );
        for (a, b) in [(2, 9), (9, 2), (-3, 4), (4, -3), (5, 5)] {
            let holds = match cond {
                "slt" => a < b,
                "ult" => (a as u32) < b as u32,
                _ => a > b,
            };
            let x: i32 = if holds { a } else { b };
            let p = x.wrapping_mul(3);
            let want = if holds { p.wrapping_neg() } else { p };
            for options in [Options::default(), Options::PINNED] {
                if let Some(code) = status_with_runtime(&ir, &[i64::from(a), i64::from(b)], options) {
                    assert_eq!(code, want & 255, "{cond} {a} {b} {options:?}");
                }
            }
        }
    }
}

const THREE_POINTERS: &str = "define i32 @k(ptr %c, ptr %a, ptr %b, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i2, %loop ]
  %pc = phi ptr [ %c, %entry ], [ %pc2, %loop ]
  %pa = phi ptr [ %a, %entry ], [ %pa2, %loop ]
  %pb = phi ptr [ %b, %entry ], [ %pb2, %loop ]
  %x = load i8, ptr %pa
  %y = load i8, ptr %pb
  %s = add i8 %x, %y
  store i8 %s, ptr %pc
  %pa2 = getelementptr i8, ptr %pa, i32 1
  %pb2 = getelementptr i8, ptr %pb, i32 1
  %pc2 = getelementptr i8, ptr %pc, i32 1
  %i2 = add i32 %i, 1
  %more = icmp ult i32 %i2, %n
  br i1 %more, label %loop, label %done
done:
  %q = getelementptr i8, ptr %c, i32 1
  %r = load i8, ptr %q
  %z = zext i8 %r to i32
  ret i32 %z
}";

/// A loop over three byte pointers and a count: LSR kept one shared index
/// register for the pointers, `move.b (%a0,%d1), ...`; with post-indexed
/// addressing each pointer steps in its own access. Runs, and no pointer is
/// stepped apart from its access.
#[test]
fn a_loop_over_three_pointers_steps_each_in_its_access() {
    use llrm_m68k::kernels::Arg::{Data, Imm};
    let args = [Data(128), Data(0), Data(0), Imm(5)];
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status_with_data(THREE_POINTERS, &args, options) {
            assert_eq!(code, 2 * i32::from(b'e'), "{options:?}");
        }
    }
    let text = text_with(&module(THREE_POINTERS), "k", Cpu::M68000, Options::default()).unwrap();
    assert!(text.matches(")+").count() >= 3, "{text}");
}

/// A phi only other phis read had no register and its inputs were dropped
/// (`partition` returned its start value). Another shape than the motivating
/// one, at every width a phi can have: a value carried through a chain of two
/// joins to a loop.
#[test]
fn a_phi_read_only_by_other_phis_keeps_its_inputs_at_every_width() {
    for ty in ["i8", "i16", "i32"] {
        let narrow = if ty == "i32" { "add i32 %a, 0".to_owned() } else { format!("trunc i32 %a to {ty}") };
        let ir = format!(
            "define i32 @f(i32 %a, i32 %n) {{
entry:
  %t = {narrow}
  %c = icmp sgt i32 %a, 5
  br i1 %c, label %left, label %right
left:
  %l = add {ty} %t, 1
  br label %join
right:
  br label %join
join:
  %v = phi {ty} [ %l, %left ], [ %t, %right ]
  br label %loop
loop:
  %w = phi {ty} [ %v, %join ], [ %w, %loop ]
  %i = phi i32 [ 0, %join ], [ %i1, %loop ]
  %i1 = add i32 %i, 1
  %more = icmp ult i32 %i1, %n
  br i1 %more, label %loop, label %done
done:
  %z = zext {ty} %w to i32
  ret i32 %z
}}"
        );
        for (a, n) in [(9, 3), (2, 3), (200, 2)] {
            let t = (a as u32) & if ty == "i8" { 255 } else if ty == "i16" { 65535 } else { u32::MAX };
            let want = if a > 5 { t.wrapping_add(1) } else { t } & if ty == "i8" { 255 } else if ty == "i16" { 65535 } else { u32::MAX };
            for options in [Options::default(), Options::PINNED] {
                if let Some(code) = status(&ir, &[a, n], options) {
                    assert_eq!(code, (want & 255) as i32, "{ty} {a} {n} {options:?}");
                }
            }
        }
    }
}

/// `zext (icmp)` where the register the compare reads is the one the `scc`
/// writes, at each width and from memory: the shapes the `neg.b` rewrite does
/// not take must still be right.
#[test]
fn a_compare_widened_to_each_width_is_zero_or_one() {
    for (ty, cond) in [("i8", "ult"), ("i16", "slt"), ("i32", "eq")] {
        let ir = format!(
            "define i32 @f(i32 %a, i32 %b) {{
  %c = icmp {cond} i32 %a, %b
  %z = zext i1 %c to {ty}
  %r = zext {ty} %z to i32
  ret i32 %r
}}"
        );
        for (a, b) in [(1, 2), (2, 1), (3, 3), (-1, 1)] {
            let holds = match cond {
                "ult" => (a as u32) < (b as u32),
                "slt" => a < b,
                _ => a == b,
            };
            for options in [Options::default(), Options::PINNED] {
                if let Some(code) = status(&ir, &[a, b], options) {
                    assert_eq!(code, i32::from(holds), "{ty} {cond} {a} {b} {options:?}");
                }
            }
        }
    }
}

/// `or disjoint x, c` on a data value was `move.l d, a; lea (c,a), a; move.l a, d`
/// (the pin has `add` patterns for `ADD32di` but only `lea` ones match the
/// `add_like` or): 8 bytes and two moves where `add.l #c, d` is 6 and none.
/// The value is the same, and a real pointer step stays an address add.
#[test]
fn an_or_of_disjoint_bits_on_a_data_value_is_a_data_add() {
    for (mask, bits) in [(15, 48), (7, 64), (255, 256), (3, 8)] {
        let ir = format!(
            "define i32 @f(i32 %a) {{
  %m = and i32 %a, {mask}
  %o = or disjoint i32 %m, {bits}
  %r = xor i32 %o, %a
  ret i32 %r
}}"
        );
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert!(!asm.contains("lea"), "mask {mask} bits {bits}: {asm}");
        if let Some(code) = status(&ir, &[0x95], Options::default()) {
            assert_eq!(code, (((0x95 & mask) | bits) ^ 0x95) & 0xFF, "mask {mask} bits {bits}: {asm}");
        }
    }
    // a pointer step is still an address add, and right
    let ir = "define i32 @f(ptr %p) {
  %q = getelementptr i8, ptr %p, i32 1
  %v = load i8, ptr %q, align 1
  %z = zext i8 %v to i32
  ret i32 %z
}";
    let asm = text_with(&module(ir), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(asm.contains("(1,") || asm.contains("addq"), "{asm}");
}
