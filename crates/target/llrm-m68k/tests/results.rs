//! Overlay rewrites against their boundaries: the text each one gives, and the
//! value the program computes under `qemu-m68k` (the exit status, 8 bits).

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Exit status of `f(args)` compiled with `options`; `None` without qemu.
fn status(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, options).expect("compiles"));
    let path = support::unique_path("results");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    code
}

/// `(a & c) + a` is the pin's `move.l a, d; and.l #c, d` for every `c`; the
/// `moveq #c, d; and.l a, d` form, for a `c` within a signed byte, was a
/// rewrite of it. The value must be the same at the edges of that range.
#[test]
fn a_mask_keeps_its_immediate_form_and_its_value() {
    for mask in [127, 128, -128, -129] {
        let ir =
            format!("define i32 @f(i32 %a) {{\n  %m = and i32 %a, {mask}\n  %r = add i32 %m, %a\n  ret i32 %r\n}}");
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert!(asm.contains(&format!("and.l\t#{mask},")), "mask {mask}: {asm}");
        if let Some(code) = status(&ir, &[0x85], Options::default()) {
            assert_eq!(code, ((0x85 & mask) + 0x85) & 0xFF, "mask {mask}: {asm}");
        }
    }
}

/// `s + (a > b) + a` is `sgt; and.l #1; add`, the pin's patterns. `moveq #0`
/// before the compare and `neg.b` after the `sgt` were a rewrite of it, 2 bytes
/// shorter, that the pin does not make.
#[test]
fn a_boolean_added_is_masked_as_the_pin_does() {
    let ir = "define i32 @f(i32 %a, i32 %b, i32 %s) {\n  %c = icmp sgt i32 %a, %b\n  %z = zext i1 %c to i32\n  %t = add i32 %s, %z\n  %r = add i32 %t, %a\n  ret i32 %r\n}";
    let ours = text_with(&module(ir), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(ours.contains("and.l\t#1,") && !ours.contains("neg.b"), "{ours}");
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
    let qemu = support::qemu()?;
    let code = items_with(&module(ir), "k", Cpu::M68000, options).expect("compiles");
    let program = llrm_m68k::kernels::program_with_data(args, code, &llrm_m68k::kernels::library_data());
    let path = support::unique_path("results-data");
    write_executable(&path, &executable(&program).unwrap()).unwrap();
    let status = support::run(&qemu, &[], &path);
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

/// Exit status of `f(args)` with the runtime of `bench/library/rt.ll` behind
/// it.
fn status_with_runtime(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    // one module, so the local labels of the routines do not meet
    let rt = include_str!("../bench/library/rt.ll");
    let runtime = module(&format!("{ir}\n{}", rt.split_once('\n').map_or("", |(_, rest)| rest)));
    all.extend(items_with(&runtime, "f", Cpu::M68000, options).expect("compiles"));
    for name in ["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"] {
        all.extend(items_with(&runtime, name, Cpu::M68000, options).expect("the runtime compiles"));
    }
    let path = support::unique_path("results-rt");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
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
            let want =
                ((m << 1) | i32::from(model(n, d))).wrapping_add(if model(n, d) { n.wrapping_add(d) } else { n });
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

/// A loop over three byte pointers and a count: the pin's addressing mode is
/// none, so no pointer is post-incremented. Runs.
#[test]
fn a_loop_over_three_pointers_runs_without_post_increment() {
    use llrm_m68k::kernels::Arg::{Data, Imm};
    let args = [Data(128), Data(0), Data(0), Imm(5)];
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status_with_data(THREE_POINTERS, &args, options) {
            assert_eq!(code, 2 * i32::from(b'e'), "{options:?}");
        }
    }
    let text = text_with(&module(THREE_POINTERS), "k", Cpu::M68000, Options::default()).unwrap();
    assert!(!text.contains("%a0)+") && !text.contains("%a1)+") && !text.contains("%a2)+"), "{text}");
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
            let t = (a as u32)
                & if ty == "i8" {
                    255
                } else if ty == "i16" {
                    65535
                } else {
                    u32::MAX
                };
            let want = if a > 5 { t.wrapping_add(1) } else { t }
                & if ty == "i8" {
                    255
                } else if ty == "i16" {
                    65535
                } else {
                    u32::MAX
                };
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

/// `or disjoint x, c` on a data value is the pin's `lea` through an address
/// register (its `ADD32di` patterns take `add` only): the value must be the
/// or's. An `addq` in place of it was a rewrite the pin does not make.
#[test]
fn an_or_of_disjoint_bits_on_a_data_value_is_the_pins_lea_and_right() {
    for (mask, bits) in [(3, 8), (1, 2), (3, 4), (7, 8)] {
        let ir = format!(
            "define i32 @f(i32 %a) {{
  %m = and i32 %a, {mask}
  %o = or disjoint i32 %m, {bits}
  %r = xor i32 %o, %a
  ret i32 %r
}}"
        );
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert!(asm.contains("lea"), "mask {mask} bits {bits}: {asm}");
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

/// The data add sets the condition codes the `lea` did not. A compare read
/// again after the join of the diamond that holds the `or disjoint` must still
/// be the compare's, whether read by a select or a `zext`.
#[test]
fn a_compare_read_after_the_join_of_an_or_disjoint_arm_keeps_its_value() {
    for second in ["select i1 %c, i32 %m, i32 %a", "zext i1 %c to i32"] {
        let ir = format!(
            "define i32 @f(i32 %a) {{
  %m = and i32 %a, 7
  %c = icmp ult i32 %m, 4
  %o = or disjoint i32 %m, 8
  %add = add i32 %m, 87
  %v = select i1 %c, i32 %o, i32 %add
  %s = {second}
  %r = add i32 %v, %s
  ret i32 %r
}}"
        );
        for a in [1_i64, 3, 5, 0x96] {
            let m = a & 7;
            let v = if m < 4 { m | 8 } else { m + 87 };
            let s = if second.starts_with("select") { if m < 4 { m } else { a } } else { i64::from(m < 4) };
            if let Some(code) = status(&ir, &[a], Options::default()) {
                assert_eq!(i64::from(code), (v + s) & 0xFF, "{second} with {a}");
            }
        }
    }
}

/// Kept by choice (not an open defect): a constant over 8 stays a `lea`, 8
/// bytes and 16 clocks against `add.l #c`'s 6 and 16 plus a move, and the
/// indexed form `(48,%a0,%d0)` for `(m | 48) + a` is 8 and 20 against 8 and 24.
/// The values are right either way.
#[test]
fn a_disjoint_or_with_a_wide_constant_keeps_its_value() {
    let ir = "define i32 @f(i32 %a) {
  %m = and i32 %a, 15
  %o = or disjoint i32 %m, 48
  %r = add i32 %o, %a
  ret i32 %r
}";
    let asm = text_with(&module(ir), "f", Cpu::M68000, Options::default()).unwrap();
    if let Some(code) = status(ir, &[0x95], Options::default()) {
        assert_eq!(code, (((0x95 & 15) | 48) + 0x95) & 0xFF, "{asm}");
    }
}

/// The shared builder refuses `llvm.mem*` for a target with no `full()`
/// description (arm64's adapter). m68k has one, so its inline expansions must
/// keep compiling and running: a 24-byte copy, a fill and an overlapping move.
#[test]
fn memory_intrinsics_compile_and_run_on_m68k() {
    let ir = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
define i32 @f(i32 %x) {
  %a = alloca [6 x i32], align 4
  %b = alloca [6 x i32], align 4
  %a2 = getelementptr i32, ptr %a, i32 2
  store i32 %x, ptr %a2, align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %b, ptr align 4 %a, i32 24, i1 false)
  %b2 = getelementptr i32, ptr %b, i32 2
  %v = load i32, ptr %b2, align 4
  call void @llvm.memset.p0.i32(ptr align 4 %a, i8 7, i32 8, i1 false)
  %f = load i8, ptr %a, align 4
  %fz = zext i8 %f to i32
  call void @llvm.memmove.p0.p0.i32(ptr align 4 %b, ptr align 4 %b2, i32 12, i1 false)
  %m = load i32, ptr %b, align 4
  %r = add i32 %v, %fz
  %q = add i32 %r, %m
  ret i32 %q
}";
    let asm = text_with(&module(ir), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(!asm.is_empty(), "{asm}");
    if let Some(code) = status(ir, &[5], Options::default()) {
        assert_eq!(code, 5 + 7 + 5, "{asm}");
    }
}

fn copy_of(
    bytes: u32,
    after: &str,
) -> String {
    format!(
        "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define i32 @f(ptr %d, ptr %s) {{
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 {bytes}, i1 false)
{after}
}}"
    )
}

/// An inline struct copy is the pin's moves with displacements, `(4,%a0)`, and
/// `(An)+` moves by default, where what is read from the destination afterwards
/// is read at its displacement less what the pointer stepped.
#[test]
fn an_inline_copy_is_post_increment_moves_unless_pinned() {
    for bytes in [8, 24, 32] {
        let after = format!(
            "  %p = getelementptr i8, ptr %d, i32 {}
  %v = load i32, ptr %p, align 4
  %q = getelementptr i8, ptr %d, i32 4
  %w = load i32, ptr %q, align 4
  %r = add i32 %v, %w
  ret i32 %r",
            bytes - 4
        );
        let ir = copy_of(bytes, &after);
        let asm = text_with(&module(&ir), "f", Cpu::M68000, Options::default()).unwrap();
        assert!(asm.contains(")+, (%a") && asm.contains(")+"), "{bytes}: {asm}");
        let pinned = text_with(&module(&ir), "f", Cpu::M68000, Options::PINNED).unwrap();
        assert!(!pinned.contains(")+"), "{pinned}");
    }
}

/// Not a chain: the destination is read as a value between two accesses, or
/// it is the pointer the function returns. The copy must still be right.
#[test]
fn a_copy_whose_pointer_is_used_whole_is_not_stepped_past_the_use() {
    let between = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define i32 @f(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 12, i1 false)
  %x = ptrtoint ptr %d to i32
  %p = getelementptr i8, ptr %d, i32 8
  %v = load i32, ptr %p, align 4
  %r = add i32 %v, %x
  ret i32 %r
}";
    let asm = text_with(&module(between), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(asm.contains("move.l"), "{asm}");
    let returned = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define ptr @f(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 12, i1 false)
  %q = getelementptr i8, ptr %d, i32 4
  ret ptr %q
}";
    let asm = text_with(&module(returned), "f", Cpu::M68000, Options::default()).unwrap();
    assert!(asm.contains("rts"), "{asm}");
}

/// Run: a 24-byte copy between two frame arrays, then fields read back.
#[test]
fn a_stepped_copy_copies_every_word() {
    let ir = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define i32 @f(i32 %x) {
  %a = alloca [6 x i32], align 4
  %b = alloca [6 x i32], align 4
  %a0 = getelementptr i32, ptr %a, i32 0
  %a5 = getelementptr i32, ptr %a, i32 5
  %a2 = getelementptr i32, ptr %a, i32 2
  store i32 %x, ptr %a0, align 4
  %y = add i32 %x, 1
  store i32 %y, ptr %a2, align 4
  %z = add i32 %x, 2
  store i32 %z, ptr %a5, align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %b, ptr align 4 %a, i32 24, i1 false)
  %b0 = load i32, ptr %b, align 4
  %b2p = getelementptr i32, ptr %b, i32 2
  %b2 = load i32, ptr %b2p, align 4
  %b5p = getelementptr i32, ptr %b, i32 5
  %b5 = load i32, ptr %b5p, align 4
  %s1 = add i32 %b0, %b2
  %s2 = add i32 %s1, %b5
  ret i32 %s2
}";
    for options in [Options::default(), Options::PINNED] {
        if let Some(code) = status(ir, &[10], options) {
            assert_eq!(code, 10 + 11 + 12, "{options:?}");
        }
    }
}

/// A copy or fill past the inline limit (eight moves; four at size) panicked
/// with "calling.toml names no register ecx": the builder took x86's `rep movs`
/// and `rep stos` for any target. A target with no string instructions calls
/// `memcpy`, `memmove`, `memset`, as the pin does.
#[test]
fn a_copy_or_fill_past_the_inline_limit_is_a_call() {
    for (name, call) in [
        ("memcpy", "call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 40, i1 false)"),
        ("memmove", "call void @llvm.memmove.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 40, i1 false)"),
        ("memset", "call void @llvm.memset.p0.i32(ptr align 4 %d, i8 1, i32 96, i1 false)"),
    ] {
        let ir = format!(
            "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @f(ptr %d, ptr %s) {{
  {call}
  ret void
}}"
        );
        for options in [Options::default(), Options::PINNED] {
            let asm = text_with(&module(&ir), "f", Cpu::M68000, options).unwrap();
            assert!(asm.contains(&format!("jsr\t{name}")) || asm.contains(name), "{name} {options:?}: {asm}");
        }
    }
}

/// The `(An)+` chain moves no access and sets the flags as `(An)` does, so a
/// store between the copy and a read of the same bytes, and a branch on the
/// flags of the last moved word, give what the unstepped code gives.
#[test]
fn a_stepped_copy_keeps_a_later_store_and_a_flag_reader() {
    let ir = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define i32 @f(i32 %x) {
  %a = alloca [4 x i32], align 4
  %b = alloca [4 x i32], align 4
  %a3 = getelementptr i32, ptr %a, i32 3
  store i32 %x, ptr %a3, align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %b, ptr align 4 %a, i32 16, i1 false)
  %b1 = getelementptr i32, ptr %b, i32 1
  store i32 9, ptr %b1, align 4
  %v1 = load i32, ptr %b1, align 4
  %b3 = getelementptr i32, ptr %b, i32 3
  %v3 = load i32, ptr %b3, align 4
  %z = icmp eq i32 %v3, 0
  %r = select i1 %z, i32 %v1, i32 77
  ret i32 %r
}";
    for (x, want) in [(0, 9), (5, 77)] {
        for options in [Options::default(), Options::PINNED] {
            if let Some(code) = status(ir, &[x], options) {
                assert_eq!(code, want, "x {x} {options:?}");
            }
        }
    }
}

/// `(x & (1 << n)) == 0` was `moveq #1; lsl.l n; and.l x; ...`; the pin
/// (`LowerAndToBTST`) tests the bit: `btst %dn, %dx`. Both outcomes run. A
/// constant one-bit mask is left to `and`/`tst`: `btst #15` saved
/// instructions in isolation but cost crc16 40 more (see `RESULTS-library.md`).
#[test]
fn a_shifted_one_against_zero_is_a_btst() {
    use llrm_m68k::kernels::Arg::Imm;
    let ir = "define i32 @k(i32 %x, i32 %n) {\n  %s = shl i32 1, %n\n  %a = and i32 %s, %x\n  %c = icmp eq i32 %a, 0\n  %r = select i1 %c, i32 7, i32 9\n  ret i32 %r\n}";
    let options = Options { mid_end: false, ..Options::default() };
    let asm = text_with(&module(ir), "k", Cpu::M68000, options).unwrap();
    assert!(asm.contains("btst\t%d") && !asm.contains("and.l") && !asm.contains("lsl"), "{asm}");
    for (x, n, want) in
        [(0x8000, 15, 9), (0x7FFF, 15, 7), (1, 0, 9), (0x80000000u32 as i64, 31, 9), (0x7FFFFFFF, 31, 7)]
    {
        if let Some(code) = status_with_data(ir, &[Imm(x), Imm(n)], options) {
            assert_eq!(code, want, "x = {x:#x}, n = {n}\n{asm}");
        }
    }
}

/// Programs of one length run in parallel each ran another's executable (the
/// path was the pid and the IR's length): a test got the exit status of a
/// different program.
#[test]
fn programs_of_one_length_run_in_parallel_without_sharing_an_executable() {
    if support::qemu().is_none() {
        return;
    }
    let ir = |value: i32| format!("define i32 @f() {{\n  ret i32 {value}\n}}");
    assert_eq!(ir(1).len(), ir(2).len());
    let runs: Vec<_> =
        (0..8).map(|i| std::thread::spawn(move || (i, status(&ir(1 + i % 2), &[], Options::default())))).collect();
    for run in runs {
        let (i, code) = run.join().unwrap();
        assert_eq!(code, Some(1 + i % 2), "thread {i}");
    }
}

/// `timeout` ends a run with status 124, which the tests read as the program's
/// answer.
#[test]
#[should_panic(expected = "did not finish")]
fn a_run_cut_off_by_the_timeout_is_not_a_status() {
    let Some(qemu) = support::qemu() else { panic!("did not finish: no qemu, the test passes") };
    let ir = "define i32 @f() {\nentry:\n  br label %l\nl:\n  br label %l\n}";
    let mut all = start("f", &[]);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("results-timeout");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run_for(1, &qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    panic!("returned {code:?}");
}

/// A pointer stepped in another block than the load through it (copies in
/// between, the phi's) must still be stepped once: the sum is right.
#[test]
fn a_pointer_stepped_in_another_block_is_stepped_once() {
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
    if let Some(code) = status_with_data(ir, &[Data(0), Imm(3)], options) {
        assert_eq!(code, (313 * 3) & 0xFF, "{asm}");
    }
}
