//! The overlay's encodings against binutils `m68k-linux-gnu-as` (unpacked
//! without root under `$GCC_M68K`, default `~/scratch/gcc-m68k/root`), and its
//! effect on size.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `d1` and so on, as the view of `bits` width the instruction names.
fn register(name: &str) -> Operand {
    let root = match name {
        "d0" => reg::D0,
        "d1" => reg::D1,
        "d2" => reg::D2,
        "d3" => reg::D3,
        "d5" => reg::D5,
        "a1" => reg::A1,
        "a3" => reg::A3,
        "sp" => reg::SP,
        _ => panic!("register {name}"),
    };
    Operand::Reg(root)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Each overlay instruction's bytes, from binutils on the same text:
/// `addq.l #1,%d1` 5281, `addq.l #8,%d1` 5081 (8 is the field 0), `subq.l
/// #4,%sp` 598f, `addq.w #3,%d2` 5642, `addq.b #2,%d3` 5403, `subq.w #1,%d0`
/// 5340, `tst.b %d0` 4a00, `tst.w %d5` 4a45, `tst.l %d1` 4a81, `addq.l #2,%a1`
/// 5489, `subq.l #7,%a3` 5f8b.
#[test]
fn overlay_instructions_encode_as_binutils_does() {
    let q = |opcode, to: &str, amount| Inst { opcode, operands: vec![register(to), Operand::Imm(amount)] };
    let t = |opcode, of: &str| Inst { opcode, operands: vec![register(of)] };
    let cases = [
        (q("ADDQ32d", "d1", 1), "5281"),
        (q("ADDQ32d", "d1", 8), "5081"),
        (q("SUBQ32a", "sp", 4), "598f"),
        (q("ADDQ16d", "d2", 3), "5642"),
        (q("ADDQ8d", "d3", 2), "5403"),
        (q("SUBQ16d", "d0", 1), "5340"),
        (t("TST8d", "d0"), "4a00"),
        (t("TST16d", "d5"), "4a45"),
        (t("TST32d", "d1"), "4a81"),
        (q("ADDQ32a", "a1", 2), "5489"),
        (q("SUBQ32a", "a3", 7), "5f8b"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

fn compiled(
    ir: &str,
    options: Options,
) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    text_with(&module, "f", Cpu::M68000, options).expect("compiles")
}

/// `add.l #1` was 6 bytes and `cmpi.l #0` 6; as `addq.l` and `tst.l` they
/// are 2 each. The pin's text keeps the long forms.
#[test]
fn a_counting_loop_uses_addq_and_tst_where_the_pin_does_not() {
    let ir = "define i32 @f(i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %i1 = add i32 %i, 1\n  %c = icmp ne i32 %i1, 0\n  %d = icmp slt i32 %i1, %n\n  br i1 %d, label %l, label %x\nx:\n  ret i32 %i1\n}";
    let pinned = compiled(ir, Options::PINNED);
    let ours = compiled(ir, Options::default());
    assert!(pinned.contains("add.l\t#1,"), "{pinned}");
    assert!(ours.contains("addq.l\t#1,") && !ours.contains("add.l\t#1,"), "{ours}");
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).unwrap();
    let size = |options| {
        let items = llrm_m68k::compile::items_with(&module, "f", Cpu::M68000, options).unwrap();
        llrm_m68k::object::assemble(&items, 0).unwrap().code.len()
    };
    assert!(size(Options::default()) < size(Options::PINNED));
}

/// Push and pop of saved registers, bytes from binutils: `move.l %d2,-(%sp)`
/// 2f02, `move.l (%sp)+,%d2` 241f, `movem.l %d2-%d3,-(%sp)` 48e73000,
/// `movem.l (%sp)+,%d2-%d3` 4cdf000c, `movem.l %d2-%d7/%a2,-(%sp)` 48e73f20,
/// `movem.l (%sp)+,%d2-%d7/%a2` 4cdf04fc, `move.l %a2,-(%sp)` 2f0a, `move.l
/// (%sp)+,%a2` 245f.
#[test]
fn push_and_pop_encode_as_binutils_does() {
    let one = |opcode, to| Inst { opcode, operands: vec![Operand::Reg(to)] };
    let many = |opcode, mask| Inst { opcode, operands: vec![Operand::Imm(mask)] };
    let d = |n: u32| 1i64 << n;
    let cases = [
        (one("PUSH32", reg::D2), "2f02"),
        (one("POP32", reg::D2), "241f"),
        (many("PUSHM32", d(2) | d(3)), "48e73000"),
        (many("POPM32", d(2) | d(3)), "4cdf000c"),
        (many("PUSHM32", 0xFC | d(10)), "48e73f20"),
        (many("POPM32", 0xFC | d(10)), "4cdf04fc"),
        (one("PUSH32", reg::A2), "2f0a"),
        (one("POP32", reg::A2), "245f"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// A leaf that only saves registers opens its frame with a push: the pin's
/// `suba.l #8, %sp` and `movem.l %d2-%d3, (0,%sp)` are 12 bytes, the push 4.
#[test]
fn a_frame_of_saved_registers_is_pushed_not_carved() {
    let ir = "define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [%a, %entry], [%s1, %l]\n  %t = phi i32 [%b, %entry], [%t1, %l]\n  %u = phi i32 [%c, %entry], [%u1, %l]\n  %v = phi i32 [%d, %entry], [%v1, %l]\n  %s1 = add i32 %s, %t\n  %t1 = add i32 %t, %u\n  %u1 = add i32 %u, %v\n  %v1 = add i32 %v, %i\n  %i1 = add i32 %i, 1\n  %e = icmp slt i32 %i1, 100\n  br i1 %e, label %l, label %x\nx:\n  %r = add i32 %s1, %u1\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    let ours = compiled(ir, Options::default());
    assert!(pinned.contains("suba.l") && pinned.contains("movem.l\t%d2"), "{pinned}");
    assert!(ours.contains("-(%sp)") && ours.contains("(%sp)+") && !ours.contains("suba.l"), "{ours}");
}

fn branches(body: &str) -> String {
    let ir = format!(
        "define i32 @f(i32 %a, i32 %b) {{\nentry:\n  {body}\n  br i1 %c, label %y, label %n\ny:\n  ret i32 1\nn:\n  ret i32 2\n}}"
    );
    compiled(&ir, Options::default())
}

/// `lsr` sets Z from its result, so the `tst.l` after it only repeats it: the
/// popcount loop ended `lsr.l #1, %d1; tst.l %d1; bne`.
#[test]
fn a_test_right_after_the_instruction_that_set_the_flags_is_dropped() {
    let asm = branches("%s = lshr i32 %a, 1\n  %c = icmp ne i32 %s, 0");
    assert!(asm.contains("lsr.l") && !asm.contains("tst.l"), "{asm}");
}

/// `add` can overflow, so after it `lt` (N xor V) is not what `tst` gives:
/// the `tst.l` stays for a signed ordering, and goes for `ne`.
#[test]
fn a_test_after_an_add_stays_for_a_signed_ordering_and_goes_for_equality() {
    let signed = branches("%s = add i32 %a, %b\n  %c = icmp slt i32 %s, 0");
    assert!(signed.contains("tst.l"), "{signed}");
    let equal = branches("%s = add i32 %a, %b\n  %c = icmp ne i32 %s, 0");
    assert!(!equal.contains("tst.l"), "{equal}");
}

/// Post-increment forms derived from the pin's `(An)` rows, bytes from
/// binutils: `add.l (%a1)+,%d0` d099, `add.b (%a1)+,%d3` d619, `and.l
/// (%a1)+,%d1` c299.
#[test]
fn post_increment_forms_encode_as_binutils_does() {
    let load =
        |opcode, to: &str, from: &str| Inst { opcode, operands: vec![register(to), register(to), register(from)] };
    let cases = [
        (load("ADD32dj+", "d0", "a1"), "d099"),
        (load("ADD8dj+", "d3", "a1"), "d619"),
        (load("AND32dj+", "d1", "a1"), "c299"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

fn sum_loop(step: &str) -> String {
    format!(
        "define i32 @f(ptr %p, i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n  %q = phi ptr [%p, %entry], [%q1, %l]\n  %v = load i32, ptr %q\n  %s1 = add i32 %s, %v\n  %q1 = getelementptr i32, ptr %q, i32 {step}\n  %i1 = add i32 %i, 1\n  %e = icmp slt i32 %i1, %n\n  br i1 %e, label %l, label %x\nx:\n  ret i32 %s1\n}}"
    )
}

/// A load through a pointer that steps by the load's size was `add.l (%a0),
/// %d0` and `addq.l #4, %a0`, two instructions for `add.l (%a0)+, %d0`.
#[test]
fn a_load_and_the_step_of_its_pointer_are_one_post_increment() {
    let pinned = compiled(&sum_loop("1"), Options::PINNED);
    let ours = compiled(&sum_loop("1"), Options::default());
    assert!(!pinned.contains(")+"), "{pinned}");
    assert!(ours.contains("add.l\t(%a0)+, %d0"), "{ours}");
    assert!(!ours.contains("addq.l\t#4"), "{ours}");
}

/// A step that is not the size of the access cannot be folded.
#[test]
fn a_step_of_another_size_is_not_a_post_increment() {
    let ours = compiled(&sum_loop("2"), Options::default());
    assert!(!ours.contains(")+"), "{ours}");
}
