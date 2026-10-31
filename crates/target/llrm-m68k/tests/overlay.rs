//! The overlay's encodings against binutils `m68k-linux-gnu-as` (unpacked
//! without root under `$GCC_M68K`), and its
//! effect on size.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::encode::encode;
use llrm_m68k::mc::{Inst, Operand};
use llrm_m68k::reg;

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

/// `d1` and so on, as the view of `bits` width the instruction names.
fn register(name: &str) -> Operand {
    let root = match name {
        "d0" => reg::D0,
        "d1" => reg::D1,
        "d2" => reg::D2,
        "d3" => reg::D3,
        "d4" => reg::D4,
        "d5" => reg::D5,
        "a0" => reg::A0,
        "a1" => reg::A1,
        "a2" => reg::A2,
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
        (t("NOT16d", "d5"), "4645"),
        (Inst { opcode: "SWAP", operands: vec![register("d3"), register("d3")] }, "4843"),
        (q("ADDQ32a", "a1", 2), "5489"),
        (q("SUBQ32a", "a3", 7), "5f8b"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// The pin's source-order scheduler. Two tests pin the code-saving fallback of
/// `flags.rs`, which only a schedule that separates a compare from its uses
/// reaches; the ILP list scheduler (the default) keeps them together and
/// retests.
const SOURCE_ORDER: Options = Options { overlay: true, mid_end: true, size: false, source_order: true };

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

/// `add` can overflow, so after it `gt` (Z clear and N = V) is not what `tst`
/// gives: the `tst.l` stays for it, and goes for `ne`. `slt 0` is `N` alone
/// (the sign of the wrapped sum, as the pin selects it: `add; bmi`), which
/// needs none.
#[test]
fn a_test_after_an_add_stays_for_a_signed_ordering_and_goes_for_equality() {
    let signed = branches("%s = add i32 %a, %b\n  %c = icmp sgt i32 %s, 0");
    assert!(signed.contains("tst.l"), "{signed}");
    let sign = branches("%s = add i32 %a, %b\n  %c = icmp slt i32 %s, 0");
    assert!(!sign.contains("tst.l") && sign.contains("add.l"), "{sign}");
    let equal = branches("%s = add i32 %a, %b\n  %c = icmp ne i32 %s, 0");
    assert!(!equal.contains("tst.l"), "{equal}");
}

/// `__modsi3` ended each way of a negate with `move.l %d2, %d0` and one of them
/// with `bra`: both ways began with the copy. It runs ahead of the compare once
/// (`HoistCommonCodeInSuccs`).
#[test]
fn the_copy_both_ways_begin_with_runs_before_the_branch() {
    let ir = "declare i32 @g(i32)
define i32 @f(i32 %a) {
  %an = icmp slt i32 %a, 0
  %na = sub i32 0, %a
  %ua = select i1 %an, i32 %na, i32 %a
  %m = call i32 @g(i32 %ua)
  %nm = sub i32 0, %m
  %r = select i1 %an, i32 %nm, i32 %m
  ret i32 %r
}";
    let text = compiled(ir, Options::PINNED);
    assert_eq!(text.matches("move.l\t%d2, %d0").count(), 1, "{text}");
    assert!(!text.contains("bra"), "{text}");
}

/// The copy that hoisting put before a `tst.l` of its source left the codes
/// `tst.l` gives: the `tst.l %d2` after `move.l %d2, %d0` was kept, an
/// instruction on the path that had none.
#[test]
fn a_test_after_a_copy_of_the_tested_register_is_dropped() {
    use llrm_m68k::expand::Line;
    use llrm_m68k::overlay::drop_redundant_tests;
    let inst = |opcode, operands| Line::Inst(Inst { opcode, operands });
    let copy = inst("MOV32rr", vec![register("d0"), register("d2")]);
    let branch = inst("Bpl8", vec![]);
    let lines = vec![copy.clone(), inst("TST32d", vec![register("d2")]), branch.clone()];
    assert_eq!(opcodes(&drop_redundant_tests(lines)), ["MOV32rr", "Bpl8"]);
    let other = vec![copy, inst("TST32d", vec![register("d1")]), branch];
    assert_eq!(opcodes(&drop_redundant_tests(other)), ["MOV32rr", "TST32d", "Bpl8"]);
}

/// An add of a constant to a zero-extended byte went to an address register,
/// because a copy of the sum reached one (`adda`), and gave the extension a
/// destination with no byte view: "MOVZXd32d8: no 8-bit view" (differential
/// seeds 495 and 532). The extension's result stays in a data register.
#[test]
fn the_result_of_an_extension_is_not_made_an_address_register() {
    let ir = "@zero = external global i8\n\
@buf = external global [16 x i8]\n\
define i16 @f() {\n\
entry:\n\
  %v8 = load volatile i8, ptr @zero, align 1\n\
  br label %v25\n\
v25:\n\
  %v26 = zext i8 %v8 to i32\n\
  %v27 = add i32 %v26, 1\n\
  %v29 = add i16 0, 1\n\
  %v30 = load i8, ptr getelementptr (i8, ptr @buf, i32 13), align 1\n\
  %v31 = zext i8 %v30 to i32\n\
  %v32 = add i32 %v27, %v31\n\
  %v33 = trunc i32 %v27 to i8\n\
  %v34 = icmp eq i8 0, %v33\n\
  %v35 = select i1 %v34, i32 0, i32 4\n\
  %v36 = getelementptr i8, ptr null, i32 %v35\n\
  %v37 = load i32, ptr %v36, align 2\n\
  %v38 = and i32 %v37, 254\n\
  %v39 = icmp eq i32 %v38, 0\n\
  %v40 = shl i32 %v32, 11\n\
  %v41 = select i1 %v39, i32 %v40, i32 0\n\
  %v43 = add i32 0, -2147483647\n\
  %v44 = xor i32 %v41, %v43\n\
  %v45 = icmp eq i32 %v44, 2147483647\n\
  %v46 = zext i1 %v45 to i16\n\
  %v47 = shl i16 %v29, %v46\n\
  %v53 = xor i16 %v47, 0\n\
  %v54 = and i16 %v53, 3\n\
  ret i16 %v54\n\
}";
    compiled(ir, Options { mid_end: false, ..Options::default() });
}

/// A second select on the codes of a compare, after the first one's join set
/// them again (and a call between), stayed a `CMOV` pseudo: "CMOV16d is a
/// pseudo-instruction: it has no encoding" (differential seeds 719 and 847).
/// The join of the first restores the codes the head kept.
#[test]
fn a_second_select_on_the_codes_of_one_compare_is_expanded() {
    let ir = "@zero = external global i8\n\
@buf = external global [16 x i8]\n\
declare void @llvm.lifetime.start.p0(ptr captures(none))\n\
declare void @llvm.lifetime.end.p0(ptr captures(none))\n\
declare void @llvm.memset.p0.i32(ptr writeonly captures(none), i8, i32, i1 immarg)\n\
declare i8 @h0()\n\
define i16 @f() {\n\
entry:\n\
  %v2 = load volatile i8, ptr @zero, align 1\n\
  %v7 = add i16 0, -7609\n\
  %v11 = add i8 %v2, -1\n\
  %v12 = icmp ult i8 %v11, -2\n\
  %v17 = load i32, ptr null, align 2\n\
  %v18 = lshr i32 %v17, 14\n\
  %v19 = select i1 %v12, i32 %v18, i32 0\n\
  %v20 = select i1 %v12, i16 129, i16 1\n\
  %v22 = shl i16 0, 1\n\
  %v23 = call fastcc i8 @h0()\n\
  %v24 = icmp eq i8 %v23, 0\n\
  %v25 = or i16 %v7, 1\n\
  %v26 = select i1 %v24, i16 %v25, i16 1\n\
  %v27 = udiv i16 %v22, %v26\n\
  %v28 = and i16 %v27, %v20\n\
  %v29 = shl i16 %v28, 2\n\
  %v30 = select i1 %v12, i16 2, i16 0\n\
  %v31 = or disjoint i16 %v29, %v30\n\
  %v32 = xor i16 %v31, -15536\n\
  %v33 = zext i16 %v32 to i32\n\
  %v34 = lshr i32 %v19, 8\n\
  %v35 = xor i32 %v34, %v33\n\
  %v36 = xor i32 %v35, %v19\n\
  %v37 = trunc i32 %v36 to i16\n\
  ret i16 %v37\n\
}";
    let text = compiled(ir, SOURCE_ORDER);
    assert!(!text.contains("should not be emitted"), "{text}");
    assert!(text.contains("%ccr") || text.contains("%sr"), "{text}");
}

/// `divu.l %d2,%d1` and `divs.l %d2,%d1`, from binutils. The pin's rows encoded
/// the remainder register as `%d0` (`divul.l %d2,%d0:%d1`), which overwrote the
/// dividend in `%d0`: 68020 differential seeds 608, 636 (about one in five
/// programs) gave wrong results.
#[test]
fn a_long_divide_stores_no_remainder() {
    let divide =
        |opcode, dq: &str, src: &str| Inst { opcode, operands: vec![register(dq), register(dq), register(src)] };
    for (inst, golden) in
        [(divide("UDIVd32d32", "d1", "d2"), "4c421001"), (divide("SDIVd32d32", "d3", "d0"), "4c403803")]
    {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// `cmpa.l %d0,%a1` b3c0 and `cmpa.l %a2,%a3` b7ca, from binutils.
#[test]
fn cmpa_encodes_as_binutils_does() {
    let cmpa = |lhs: &str, rhs: &str| Inst { opcode: "CMPA32", operands: vec![register(lhs), register(rhs)] };
    for (inst, golden) in [(cmpa("d0", "a1"), "b3c0"), (cmpa("a2", "a3"), "b7ca")] {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// The loaded value of the running maximum took `d2`, because the compare
/// wants a data register, and so a push and a pop (4 bytes). It compares
/// from an address register with `cmpa.l`, as gcc does.
#[test]
fn a_loaded_value_only_compared_may_sit_in_an_address_register() {
    let marked = arrmax_loop().replace(") {\nentry", ") optsize {\nentry");
    let ours = compiled(&marked, Options::default());
    assert!(ours.contains("cmpa.l") && !ours.contains("%d2"), "{ours}");
    let pinned = compiled(arrmax_loop(), Options::PINNED);
    assert!(!pinned.contains("cmpa.l"), "{pinned}");
}

fn sum_loop(step: &str) -> String {
    format!(
        "define i32 @f(ptr %p, i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n  %q = phi ptr [%p, %entry], [%q1, %l]\n  %v = load i32, ptr %q\n  %s1 = add i32 %s, %v\n  %q1 = getelementptr i32, ptr %q, i32 {step}\n  %i1 = add i32 %i, 1\n  %e = icmp slt i32 %i1, %n\n  br i1 %e, label %l, label %x\nx:\n  ret i32 %s1\n}}"
    )
}

/// A step that is not the size of the access cannot be folded.
#[test]
fn a_step_of_another_size_is_not_a_post_increment() {
    let ours = compiled(&sum_loop("2"), Options::default());
    assert!(!ours.contains("%a0)+") && !ours.contains("%a1)+") && !ours.contains("%a2)+"), "{ours}");
}

/// `add.l #-1` took 6 bytes where `subq.l #1` takes 2: strlen ended with it.
#[test]
fn adding_a_small_negative_is_a_subq() {
    let ir = "define i32 @f(i32 %a) {\nentry:\n  %r = add i32 %a, -1\n  %c = add i32 %r, -8\n  %d = sub i32 %c, -3\n  ret i32 %d\n}";
    let pinned = compiled(ir, Options::PINNED);
    let ours = compiled(ir, Options::default());
    assert!(pinned.contains("add.l\t#-1") || pinned.contains("add.l\t#-"), "{pinned}");
    assert!(ours.contains("subq.l") && !ours.contains("add.l\t#-"), "{ours}");
}

fn loop_moves(asm: &str) -> usize {
    let body = asm.split(".LBB0_1:").nth(1).expect("a loop");
    body.lines().take_while(|line| !line.contains("blt")).filter(|line| line.contains("move.l")).count()
}

/// The loop that carries `(a, b) <- (b, a + b)` was three moves: the sum took
/// `a`'s register and the pair rotated through a temporary. Tying the sum to
/// `b`, whose old value the carried copy keeps, leaves two (gcc's count).
#[test]
fn a_loop_carrying_a_sum_rotates_its_pair_in_two_moves() {
    let ir = "define i32 @f(i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %a = phi i32 [0, %entry], [%b, %l]\n  %b = phi i32 [1, %entry], [%s, %l]\n  %s = add i32 %a, %b\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %s\n}";
    assert_eq!(loop_moves(&compiled(ir, Options::PINNED)), 3, "{}", compiled(ir, Options::PINNED));
    let ours = compiled(ir, Options::default());
    assert_eq!(loop_moves(&ours), 2, "{ours}");
}

/// The rotation's temporary was a copy of `a` narrowed to a data register by
/// the add's tied slot it no longer sat in, so the pair took `d0`, `d1`, the
/// counter took callee-saved `d2`, and the function pushed and popped it (4
/// bytes). The temporary may take an address register.
#[test]
fn the_rotation_temporary_does_not_cost_a_saved_data_register() {
    let ir = "define i32 @f(i32 %n) optsize {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %a = phi i32 [0, %entry], [%b, %l]\n  %b = phi i32 [1, %entry], [%s, %l]\n  %s = add i32 %a, %b\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %s\n}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("%d2") && !ours.contains("(%sp)+"), "{ours}");
}

/// `s += p[i] > t` ends `sgt %d1; and.l #255, %d1; and.l #1, %d1` in the pin's
/// patterns; merging the masks was a rewrite of them.
#[test]
fn two_masks_of_one_register_in_a_row_stay_as_the_pin_has_them() {
    let ir = "define i32 @f(i32 %a, i32 %b, i32 %s) {\n  %c = icmp sgt i32 %a, %b\n  %z = zext i1 %c to i32\n  %r = add i32 %s, %z\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("and.l\t#255") && pinned.contains("and.l\t#1,"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("and.l\t#1,") && ours.contains("and.l\t#255"), "{ours}");
}

fn arrmax_loop() -> &'static str {
    "define i32 @f(ptr %p, i32 %n) {\nentry:\n  %f = load i32, ptr %p\n  br label %l\nl:\n  %i = phi i32 [1, %entry], [%i1, %l]\n  %m = phi i32 [%f, %entry], [%m1, %l]\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  %c = icmp sgt i32 %v, %m\n  %m1 = select i1 %c, i32 %v, i32 %m\n  %i1 = add i32 %i, 1\n  %d = icmp slt i32 %i1, %n\n  br i1 %d, label %l, label %x\nx:\n  ret i32 %m1\n}"
}

/// Register-to-register moves between the loop label and its last branch.
fn loop_register_moves(asm: &str) -> usize {
    let body = asm.split(".LBB0_1:").nth(1).expect("a loop");
    let end = body.rfind("\tblt").expect("the loop branch");
    body[..end].lines().filter(|line| line.starts_with("\tmove.l\t%")).count()
}

/// The running maximum was copied on both arms of the select and once more at
/// the loop end: the copy of the loaded value joined its register first and
/// kept the copy that runs every trip from joining the maximum's. Now the
/// maximum stays in its register and only the arm that takes a new one moves.
#[test]
fn a_select_in_a_loop_leaves_one_move_on_the_arm_that_changes_the_value() {
    let pinned = compiled(arrmax_loop(), Options::PINNED);
    assert!(loop_register_moves(&pinned) >= 3, "{pinned}");
    let ours = compiled(arrmax_loop(), Options::default());
    assert_eq!(loop_register_moves(&ours), 1, "{ours}");
}

/// `p[i + 1]` through `(d8,An,Xn)`: the base class's `isLegalAddressingMode`
/// refuses a base, an index and a displacement together, the hardware does
/// not. The pin's M68k overrides no hook, so the refusal stays; a byte
/// displacement priced legal was a deviation (`M68k::indexed_displacement`).
#[test]
fn a_base_an_index_and_a_displacement_are_refused_as_the_pin_does() {
    use llrm_m68k::machine::M68k;
    use llrm_mir::tti::Tti;
    let tti = M68k::default();
    assert!(!tti.is_legal_addressing_mode(false, 1, true, 1));
    assert!(!tti.is_legal_addressing_mode(false, -128, true, 1) && !tti.is_legal_addressing_mode(false, 127, true, 1));
    assert!(tti.is_legal_addressing_mode(false, 0, true, 1));
}

fn counted_sum() -> &'static str {
    "define i32 @f(ptr %p, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  %s1 = add i32 %s, %v\n  %i1 = add i32 %i, 1\n  %e = icmp slt i32 %i1, %n\n  br i1 %e, label %l, label %x\nx:\n  ret i32 %s1\n}"
}

fn code_size(
    ir: &str,
    options: Options,
) -> usize {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).unwrap();
    let items = llrm_m68k::compile::items_with(&module, "f", Cpu::M68000, options).unwrap();
    llrm_m68k::object::assemble(&items, 0).unwrap().code.len()
}

/// `n` was loaded into `a1` once and compared with in the register: 2 bytes
/// more than comparing with the argument slot. In the loop that costs clocks
/// on every trip, so only where size is wanted (`-Os`, or `optsize`).
#[test]
fn an_argument_read_once_in_a_loop_is_read_from_its_slot_where_size_is_wanted() {
    let speed = compiled(counted_sum(), Options::default());
    assert!(!speed.contains("cmp.l\t(8,%sp)"), "{speed}");
    let size = compiled(counted_sum(), Options { size: true, ..Options::default() });
    assert!(size.contains("cmp.l\t(8,%sp), %d"), "{size}");
    assert!(
        code_size(counted_sum(), Options { size: true, ..Options::default() })
            < code_size(counted_sum(), Options::default())
    );
    let marked = counted_sum().replace(") {\nentry", ") optsize {\nentry");
    assert!(compiled(&marked, Options::default()).contains("cmp.l\t(8,%sp), %d"));
}

/// Outside a loop the memory form is smaller and no slower: always.
#[test]
fn an_argument_read_once_outside_a_loop_is_read_from_its_slot() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\nentry:\n  %c = icmp slt i32 %a, 5\n  br i1 %c, label %y, label %n\ny:\n  %r = add i32 %a, %b\n  ret i32 %r\nn:\n  ret i32 0\n}";
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("add.l\t(8,%sp), %d"), "{ours}");
    assert!(!ours.contains("move.l\t(8,%sp)"), "{ours}");
}

/// popcount's `x & 1` of a copy is the pin's `move.l %d1, %d2; and.l #1, %d2`;
/// `moveq #1, %d2; and.l %d1, %d2` (4 bytes against 8) was a rewrite of it.
#[test]
fn a_small_constant_mask_of_a_copy_keeps_the_pins_and_immediate() {
    let ir = "define i32 @f(i32 %a) {\n  %m = and i32 %a, 1\n  %r = add i32 %m, %a\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("and.l\t#1,"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("and.l\t#1,") && !ours.contains("moveq\t#1,"), "{ours}");
}

/// `p[i] > t` loaded the element into a register to compare it: the load was
/// the right side of the `cmp`, which has no memory form. Swapped (with the
/// condition) the load is the source and the `cmp` reads it from memory.
#[test]
fn a_loaded_value_compared_with_a_register_is_read_by_the_compare() {
    let ir = "define i32 @f(ptr %p, i32 %t) {\nentry:\n  %v = load i32, ptr %p\n  %u = add i32 %t, 100000\n  %c = icmp sgt i32 %v, %u\n  br i1 %c, label %y, label %n\ny:\n  ret i32 1\nn:\n  ret i32 2\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("move.l\t(%a0), %d") && pinned.contains("cmp.l\t%d"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("cmp.l\t(%a0), %d") && !ours.contains("move.l\t(%a0)"), "{ours}");
    assert!(code_size(ir, Options::default()) <= code_size(ir, Options::PINNED));
    // where size is wanted the argument is read from its slot instead, which
    // the swap would turn into a register: 4 bytes more on countgt
    let sized = compiled(ir, Options { size: true, ..Options::default() });
    assert!(sized.contains("move.l\t(%a0)"), "{sized}");
}

/// A load read twice is loaded once: swapping would load it again.
#[test]
fn a_loaded_value_read_again_is_not_swapped_into_the_compare() {
    let ir = "define i32 @f(ptr %p, i32 %t) {\n  %v = load i32, ptr %p\n  %c = icmp sgt i32 %v, %t\n  %z = zext i1 %c to i32\n  %r = add i32 %z, %v\n  ret i32 %r\n}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("cmp.l\t(%a0)"), "{ours}");
}

/// `tst` of memory, bytes from binutils: `tst.b (0,%a0,%d1.l)` 4a301800,
/// `tst.w (4,%a1)` 4a690004, `tst.l (%a2)` 4a92, `tst.l (8,%a0,%d2.l)`
/// 4ab02808.
#[test]
fn tst_of_memory_encodes_as_binutils_does() {
    let tst = |opcode, operands| Inst { opcode, operands };
    let imm = Operand::Imm;
    let cases = [
        (tst("TST8f", vec![imm(0), register("a0"), register("d1")]), "4a301800"),
        (tst("TST16p", vec![imm(4), register("a1")]), "4a690004"),
        (tst("TST32j", vec![register("a2")]), "4a92"),
        (tst("TST32f", vec![imm(8), register("a0"), register("d2")]), "4ab02808"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// strlen's test of the byte was `cmpi.b #0, (0,%a0,%d1)`, 6 bytes; `tst.b` of
/// the same operand is 4.
#[test]
fn a_compare_of_memory_with_zero_is_a_tst() {
    let ir = "define i32 @f(ptr %p, i32 %i) {\n  %q = getelementptr i8, ptr %p, i32 %i\n  %v = load i8, ptr %q\n  %z = icmp eq i8 %v, 0\n  %r = zext i1 %z to i32\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("cmpi.b\t#0,"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("tst.b\t(") && !ours.contains("cmpi.b"), "{ours}");
    assert!(code_size(ir, Options::default()) < code_size(ir, Options::PINNED));
}

/// The mid-end with the pin's addressing mode (none): `post_indexed` off.
fn after_pinned_mid_end(ir: &str) -> String {
    use std::rc::Rc;
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let tti = llrm_m68k::machine::M68k::default();
    let done = llrm_m68k::compile::mid_end_described_by(&module, &tti, Some(Rc::new(llrm_m68k::machine::M68kMachine)))
        .expect("the mid-end runs");
    llrm_mir::print::module(&done)
}

/// LSR ranked solutions by instruction count before the target's order, though
/// the pin does so only when `-lsr-insns-cost` is given on the command line. On
/// m68k (registers first) a two-pointer copy then traded a pointer for a shared
/// index: 2 instructions and 4 registers where the pin's 3 and 3. Expected: the
/// loop keeps one stepping pointer per array (`opt -passes=loop-reduce`).
#[test]
fn copy_loops_keep_a_pointer_per_array_when_registers_rank_first() {
    for (step, ty, align) in [(2, "i16", 2), (4, "i32", 4)] {
        let done = after_pinned_mid_end(&format!(
            "define void @k(ptr %d, ptr %s, i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %sp = getelementptr {ty}, ptr %s, i32 %i
  %dp = getelementptr {ty}, ptr %d, i32 %i
  %v = load {ty}, ptr %sp, align {align}
  store {ty} %v, ptr %dp, align {align}
  %next = add nuw i32 %i, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret void
}}"
        ));
        let pointer_phis = done.lines().filter(|line| line.contains("phi ptr")).count();
        assert_eq!(pointer_phis, 2, "step {step}: a stepping pointer per array\n{done}");
    }
    // bench/library/memcpy.ll: pointers and a down counter, and the pin keeps
    // them
    let done = after_pinned_mid_end(
        "define i32 @k(ptr %0, ptr %1, i32 %2) {
  %4 = icmp sgt i32 %2, 0
  br i1 %4, label %5, label %14
5:
  %6 = phi ptr [ %12, %5 ], [ %0, %3 ]
  %7 = phi i32 [ %9, %5 ], [ %2, %3 ]
  %8 = phi ptr [ %10, %5 ], [ %1, %3 ]
  %9 = add i32 %7, -1
  %10 = getelementptr i8, ptr %8, i32 1
  %11 = load i8, ptr %8, align 1
  %12 = getelementptr i8, ptr %6, i32 1
  store i8 %11, ptr %6, align 1
  %13 = icmp ugt i32 %7, 1
  br i1 %13, label %5, label %14
14:
  ret i32 0
}",
    );
    assert_eq!(done.lines().filter(|line| line.contains("phi ptr")).count(), 2, "{done}");
}

/// The pre-indexed addressing of the pin's LSR (`AMK_PreIndexed`) is not
/// translated (see `lsr_solve.rs`): a pointer that steps down before each
/// access stays a step and an access, not `move.b -(%a0), ...`. This pins the
/// limit; when it is lifted the test changes with it.
#[test]
fn a_pointer_stepped_down_before_each_access_is_not_pre_decremented_yet() {
    let ir = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %q1, %loop ]
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %q1 = getelementptr i8, ptr %q, i32 -1
  %v = load i8, ptr %q1
  %z = zext i8 %v to i32
  %s1 = add i32 %s, %z
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %loop, label %done
done:
  ret i32 %s1
}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("-(%a"), "{ours}");
}

/// `&a[n - 1]` read in two later blocks was `add.l %d1, %d3` in a callee-saved
/// data register and a `move.l %d3, %a1` per access (partition: 5 data
/// registers saved, 2 instructions and 4 bytes per access more than gcc's
/// `lea -4(%a3,%d0.l),%a4`). A sum that is only an address is one `lea`.
#[test]
fn a_sum_that_is_only_an_address_is_a_lea() {
    let ir = llrm_m68k::kernels::source_in("library", "partition");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let ours = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(ours.contains("\tlea\t(0,%a") && !ours.contains("add.l\t%d1, %d3"), "{ours}");
    let pinned = text_with(&module, "k", Cpu::M68000, Options::PINNED).expect("compiles");
    assert!(!pinned.contains("\tlea\t(0,%a"), "{pinned}");
}

mod repeated_compares {
    use llrm_m68k::expand::Line;
    use llrm_m68k::overlay::drop_repeated_compares;

    use super::*;

    fn cmp() -> Line {
        Line::Inst(Inst { opcode: "CMP32dr", operands: vec![register("d2"), register("a1")] })
    }

    fn branch(opcode: &'static str) -> Line {
        Line::Inst(Inst { opcode, operands: vec![Operand::Symbol(".LBB0_9".into(), 0)] })
    }

    /// `a[m] == key`, then `a[m] < key` on the next block: one `cmp` for both
    /// branches (bsearch ran `cmp; beq; cmp; bge` each trip).
    #[test]
    fn a_compare_again_after_a_branch_is_dropped() {
        let lines = vec![cmp(), branch("Beq8"), cmp(), branch("Bge8")];
        assert_eq!(drop_repeated_compares(lines), vec![cmp(), branch("Beq8"), branch("Bge8")]);
    }

    /// A label in between has another way in; a write between changes the
    /// operands; a different compare is a different compare.
    #[test]
    fn a_compare_after_a_label_a_write_or_other_operands_stays() {
        let with_label = vec![cmp(), branch("Beq8"), Line::Label(3), cmp(), branch("Bge8")];
        assert_eq!(drop_repeated_compares(with_label.clone()), with_label);
        let write =
            Line::Inst(Inst { opcode: "ADDQ32d", operands: vec![register("d2"), register("d2"), Operand::Imm(1)] });
        let after_write = vec![cmp(), branch("Beq8"), write, cmp(), branch("Bge8")];
        assert_eq!(drop_repeated_compares(after_write.clone()), after_write);
        let other = Line::Inst(Inst { opcode: "CMP32dr", operands: vec![register("d3"), register("a1")] });
        let different = vec![cmp(), branch("Beq8"), other, branch("Bge8")];
        assert_eq!(drop_repeated_compares(different.clone()), different);
    }
}

/// A frame of saved registers and an outgoing-argument area was `suba.l #20,
/// %sp; movem.l regs, (8,%sp)` and back: 24 B of prologue and epilogue. The
/// saved registers are the top of the frame, so pushing them and taking the
/// rest off the stack pointer opens the same frame in 12 B (fnptr 52 to 40 B,
/// muldiv 80 to 68).
#[test]
fn saved_registers_are_pushed_and_the_rest_of_the_frame_follows() {
    let ir = llrm_m68k::kernels::source_in("library", "fnptr");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let ours = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(ours.contains("movem.l\t%d2-%d3/%a2, -(%sp)\n\tsubq.l\t#8, %sp\n"), "{ours}");
    assert!(ours.contains("\taddq.l\t#8, %sp\n\tmovem.l\t(%sp)+, %d2-%d3/%a2\n\trts"), "{ours}");
    let pinned = text_with(&module, "k", Cpu::M68000, Options::PINNED).expect("compiles");
    assert!(pinned.contains("suba.l\t#20, %sp\n\tmovem.l\t%d2-%d3/%a2, (8,%sp)"), "{pinned}");
}

fn library(name: &str) -> String {
    let text = std::fs::read_to_string(format!("{}/bench/library/{name}.ll", env!("CARGO_MANIFEST_DIR"))).unwrap();
    text.lines()
        .filter(|line| !line.starts_with("target datalayout"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("@k(", "@f(")
}

/// bsearch's key is compared twice per trip (`==`, then `<`): two readers, so
/// the argument fold (one reader only) kept the load, the register and, at
/// `-Os`, a second callee-saved register to save (+4 B over gcc, 70 B).
/// Expected: both compares read the slot, 64 B.
#[test]
fn an_argument_compared_twice_in_a_loop_is_read_from_its_slot_at_the_size_level() {
    let size = Options { size: true, ..Options::default() };
    assert!(compiled(&library("bsearch"), size).contains("cmp.l\t(16,%sp), %d"));
    assert!(code_size(&library("bsearch"), size) <= 64);
}

/// muldiv reads `b` three times, as the outgoing arguments of three calls:
/// folded, each is a 6 B memory-to-memory `move` where the register form is 4,
/// and the function grew from 68 to 78 B.
#[test]
fn an_argument_read_three_times_keeps_its_register() {
    let text = compiled(&library("muldiv"), Options::default());
    assert!(!text.contains(",%sp), ("), "{text}");
}

/// memset stored the low byte of its `c` argument from a register: the 4 B
/// load, the register and, at `-Os`, the callee-saved register the index then
/// needed (46 B against gcc's 44). A byte read of an argument is the last byte
/// of its slot (big-endian): `move.b (11,%sp), (%a1)+`.
#[test]
fn a_byte_of_an_argument_is_read_from_the_end_of_its_slot() {
    let size = Options { size: true, ..Options::default() };
    let text = compiled(&library("memset"), size);
    assert!(text.contains("move.b\t(11,%sp), "), "{text}");
}

/// The same argument also read as a whole value keeps its register: the load is
/// not given up for the byte store alone.
#[test]
fn an_argument_also_read_whole_keeps_its_register() {
    let ir = "define i32 @f(ptr %p, i32 %c, i32 %n) {
entry:
  br label %l
l:
  %i = phi i32 [0, %entry], [%i1, %l]
  %q = getelementptr i8, ptr %p, i32 %i
  %b = trunc i32 %c to i8
  store i8 %b, ptr %q
  %i1 = add i32 %i, 1
  %d = icmp ne i32 %i1, %n
  br i1 %d, label %l, label %x
x:
  %r = shl i32 %c, %i1
  ret i32 %r
}";
    let text = compiled(ir, Options { size: true, ..Options::default() });
    assert!(text.contains("move.l\t(8,%sp), %d"), "{text}");
    assert!(!text.contains("(11,%sp)"), "{text}");
}

fn constant(value: i64) -> String {
    compiled(&format!("define i32 @f() {{\n  ret i32 {value}\n}}"), Options::default())
}

/// crc16 loaded 65535 with `move.l #65535` (6 B) twice, where gcc has `moveq
/// #0; not.w` (4 B). A constant of 0xFF80..0xFFFF is a `moveq` and a `not.w`;
/// 0xFF7F, one short, and 0x10000 are not.
#[test]
fn a_constant_just_under_64k_is_a_moveq_and_not_w() {
    let text = constant(65535);
    assert!(text.contains("moveq\t#0, %d0") && text.contains("not.w\t%d0"), "{text}");
    assert!(constant(65408).contains("moveq\t#127, %d0"));
    assert!(!constant(65407).contains("not.w"));
    assert!(!constant(65536 * 128).contains("swap"));
}

/// A `moveq` value shifted up 16 is a `moveq` and a `swap` (4 B for 6); a value
/// that is not a multiple of 64 KiB is not.
#[test]
fn a_constant_shifted_up_16_is_a_moveq_and_swap() {
    let text = constant(65536 * 3);
    assert!(text.contains("moveq\t#3, %d0") && text.contains("swap\t%d0"), "{text}");
    assert!(!constant(65536 * 3 + 1).contains("swap"));
    assert!(!constant(0).contains("swap"));
}

fn opcodes(lines: &[llrm_m68k::expand::Line]) -> Vec<&'static str> {
    lines
        .iter()
        .filter_map(|line| match line {
            llrm_m68k::expand::Line::Inst(inst) => Some(inst.opcode),
            _ => None,
        })
        .collect()
}

#[test]
fn word_immediates_encode_as_binutils_does() {
    let word = |opcode, value| Inst { opcode, operands: vec![register("d0"), register("d0"), Operand::Imm(value)] };
    // eori.w #4129,%d0; or.w #40000,%d0 (the pin's `or` with a source of #imm:
    // binutils assembles the same text as `ori`, 0040 9c40, as long and as
    // quick); and.w #4660,%d0 (c07c 1234)
    for (inst, golden) in [
        (word("XOR16di", 4129), "0a401021"),
        (word("OR16di", 40000), "807c9c40"),
        (word("AND16di", 0x1234), "c07c1234"),
    ] {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

const GLOBALS: &str = "@g = global i32 5\n@t = constant [4 x i8] c\"\\01\\02\\03\\04\"\n@a = global [4 x i32] zeroinitializer\n\
    define i32 @load() {\n  %v = load i32, ptr @g\n  ret i32 %v\n}\n\
    define void @store(i32 %x) {\n  store i32 %x, ptr @g\n  ret void\n}\n\
    define ptr @addr() {\n  ret ptr @g\n}\n\
    define i32 @fold() {\n  %p = getelementptr [4 x i32], ptr @a, i32 0, i32 2\n  %v = load i32, ptr %p\n  ret i32 %v\n}\n\
    define i32 @index(i32 %i) {\n  %p = getelementptr i8, ptr @t, i32 %i\n  %b = load i8, ptr %p\n  %z = zext i8 %b to i32\n  ret i32 %z\n}";

fn global_function(name: &str) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{GLOBALS}")).expect("parses");
    text_with(&module, name, Cpu::M68000, Options::PINNED).expect("compiles")
}

/// A load of a global was `move.l (g,%a0), %d0`: the matcher did not know `PC`
/// (no register id), took the symbol for an address-register displacement and
/// read `%a0`, which nothing set. The pin's output (`llc -mtriple=m68k`) is
/// pc-relative: `move.l (g,%pc), %d0`.
#[test]
fn a_global_is_loaded_pc_relative() {
    assert_eq!(global_function("load"), "load:\n\tmove.l\t(g,%pc), %d0\n\trts\n");
    assert_eq!(global_function("fold"), "fold:\n\tmove.l\t(a+8,%pc), %d0\n\trts\n");
}

/// Stores cannot go to a pc-relative destination: the pin takes the address
/// into a register.
#[test]
fn a_global_is_stored_through_its_address() {
    assert_eq!(global_function("store"), "store:\n\tlea\t(g,%pc), %a0\n\tmove.l\t(4,%sp), (%a0)\n\trts\n");
    assert_eq!(global_function("addr"), "addr:\n\tlea\t(g,%pc), %a0\n\trts\n");
}

/// An array in read-only data indexed by a register was `CannotSelect(Load)`;
/// the pin's output is `lea (t,%pc), %a0; move.b (0,%a0,%d0), %d0`.
#[test]
fn a_constant_array_is_indexed_through_its_address() {
    let text = global_function("index");
    assert!(text.contains("lea\t(t,%pc), %a0") && text.contains("move.b\t(0,%a0,%d0), %d0"), "{text}");
}

const NARROW_GLOBALS: &str = "@buf = constant [16 x i8] zeroinitializer\n@w = constant [4 x i16] zeroinitializer\n\
    define i16 @zb() {\n  %v = load i8, ptr getelementptr (i8, ptr @buf, i32 6)\n  %z = zext i8 %v to i16\n  ret i16 %z\n}\n\
    define i32 @zw() {\n  %v = load i16, ptr getelementptr (i16, ptr @w, i32 1)\n  %z = zext i16 %v to i32\n  ret i32 %z\n}\n\
    define i32 @zbi(i32 %i) {\n  %p = getelementptr i8, ptr @buf, i32 %i\n  %v = load i8, ptr %p\n  %z = zext i8 %v to i32\n  ret i32 %z\n}";

fn narrow_global(name: &str) -> Result<String, String> {
    let module =
        llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{NARROW_GLOBALS}")).expect("parses");
    text_with(&module, name, Cpu::M68000, Options::PINNED)
}

/// A byte or word of a global, zero-extended, was `MOVZXd16q8: addressing mode
/// q is not translated` (the generated programs read `buf` and `tab` this way).
/// The pin: `move.b (buf+6,%pc), %d0` after a clear, and through an index
/// register `lea (buf,%pc), %a0; move.b (0,%a0,%d0), %d0`.
#[test]
fn a_narrow_global_is_zero_extended_from_its_pc_relative_address() {
    let byte = narrow_global("zb").expect("compiles");
    assert!(byte.contains("move.b\t(buf+6,%pc), %d0"), "{byte}");
    let word = narrow_global("zw").expect("compiles");
    assert!(word.contains("move.w\t(w+2,%pc), %d0"), "{word}");
    assert!(narrow_global("zbi").expect("compiles").contains("move.b\t(0,%a0,%d0), %d0"));
}

/// `MOVZXd32d16` defines an `XR32` register in the pin, so the allocator could
/// give it an address register: the expansion cleared it with `moveq #0, %a1`
/// (a `moveq` into `d1` in the bits) and `move.w %d4, %a1` is `movea.w`, which
/// sign-extends. A generated program (a `zext i16` kept in a register) came out
/// 4 where gcc has 3. The result of a zero extension is a data register.
#[test]
fn a_zero_extension_defines_a_data_register() {
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"{DATALAYOUT}\"\ndefine void @f(i16 %a, i16 %b, ptr %p) {{\n  %s = add i16 %a, %b\n  %e = zext i16 %s to i32\n  store i32 %e, ptr %p\n  ret void\n}}"
    ))
    .expect("parses");
    let blocks = llrm_m68k::dagisel::function::select_function(&module, "f", Cpu::M68000).expect("selects");
    let function = llrm_m68k::mfunc::emit("f", &blocks).expect("emits");
    let mut seen = 0;
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if let (llrm_spike_mf::Kind::Op(name), Some(llrm_spike_mf::Operand::Def(llrm_spike_mf::Reg::Virt(v)))) =
            (inst.kind, inst.operands.first())
            && name.starts_with("MOVZX")
        {
            seen += 1;
            assert_eq!(function.classes[*v as usize], llrm_m68k::mfunc::Class::Dr32, "{name}");
        }
    }
    assert!(seen > 0, "no zero extension was selected");
}

/// `shl x, 16` is selected as `SUBREG_TO_REG (CLR16dd (EXTRACT_SUBREG (SWAP
/// x)))`, which the emitter had no case for ("no encoding for SUBREG_TO_REG"):
/// every generated program with a shift by 16 was rejected. It is a `swap` and
/// a mask of the low half; a shift by 15 stays a shift.
#[test]
fn a_shift_left_by_16_is_swap_and_clear_word() {
    let text = compiled("define i32 @f(i32 %x) {\n  %r = shl i32 %x, 16\n  ret i32 %r\n}", Options::default());
    assert!(text.contains("swap\t%d0\n"), "{text}");
    let other = compiled("define i32 @f(i32 %x) {\n  %r = shl i32 %x, 15\n  ret i32 %r\n}", Options::default());
    assert!(!other.contains("swap"), "{other}");
}

/// `widen_compare_operands` let a compare's second operand take an address
/// register where nothing else restricts it; it read the table's `XR32` for the
/// result of `MOVZXd32d16` as free, so a zero-extended word was given `a0`
/// again (`moveq #0, %a0; move.w %d3, %a0`: a sign extension),
/// and a sign-extended one got `ext.l %a0`, which does not exist. Seeds 12 and
/// 75 of the differential driver.
#[test]
fn a_compare_does_not_widen_an_extension_to_an_address_register() {
    for (extension, mnemonic) in [("zext", "MOVZX"), ("sext", "MOVSX")] {
        let module = llrm_mir::parse::module(&format!(
            "target datalayout = \"{DATALAYOUT}\"\ndefine i32 @f(i16 %a, i16 %b, i32 %y) {{\n  %x = lshr i16 %a, %b\n  %e = {extension} i16 %x to i32\n  %n = sub i32 0, %y\n  %c = icmp sgt i32 %e, %n\n  %r = select i1 %c, i32 1, i32 2\n  ret i32 %r\n}}"
        ))
        .expect("parses");
        let blocks = llrm_m68k::dagisel::function::select_function(&module, "f", Cpu::M68000).expect("selects");
        let mut function = llrm_m68k::mfunc::emit("f", &blocks).expect("emits");
        llrm_m68k::overlay::widen_compare_operands(&mut function);
        let compared = function
            .blocks
            .iter()
            .flat_map(|block| &block.insts)
            .filter_map(|inst| match (inst.kind, &inst.operands[..]) {
                (
                    llrm_spike_mf::Kind::Op("CMP32dr"),
                    [_, llrm_spike_mf::Operand::Use(llrm_spike_mf::Reg::Virt(rhs))],
                ) => Some(*rhs),
                _ => None,
            })
            .count();
        assert_eq!(compared, 1, "the compare is the premise");
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            if let (llrm_spike_mf::Kind::Op(name), Some(llrm_spike_mf::Operand::Def(llrm_spike_mf::Reg::Virt(v)))) =
                (inst.kind, inst.operands.first())
                && name.starts_with(&format!("{mnemonic}d32d16"))
            {
                assert_eq!(function.classes[*v as usize], llrm_m68k::mfunc::Class::Dr32, "{extension}: {name}");
            }
        }
    }
}

/// The mid-end handed out an id for a declaration a pass made (`llvm.umin.i1`)
/// and never added the declaration to the module, so selecting the call
/// panicked with an index out of range (`Module::global`). Seed 57 of the
/// differential driver.
#[test]
fn a_declaration_the_mid_end_makes_is_in_the_module() {
    let ir = r#"@zero = external constant i8, align 1
define i16 @f() {
  %1 = load volatile i8, ptr @zero, align 1
  %2 = load volatile i8, ptr @zero, align 1
  %3 = load volatile i8, ptr @zero, align 1
  %4 = load volatile i8, ptr @zero, align 1
  %5 = load volatile i8, ptr @zero, align 1
  %6 = load volatile i8, ptr @zero, align 1
  %7 = add i8 %4, -41
  %8 = zext i8 %5 to i16
  %9 = add i16 %8, 31052
  %10 = add i8 %6, -66
  %11 = icmp eq i8 %2, -1
  %12 = icmp eq i8 %1, -57
  %13 = select i1 %11, i1 true, i1 %12
  %14 = zext i1 %13 to i8
  %15 = add i8 %7, %14
  %16 = select i1 %13, i16 8, i16 0
  %17 = xor i16 %9, %16
  %18 = select i1 %13, i8 %15, i8 0
  %19 = add i8 %10, %18
  br label %20

20:                                               ; preds = %0, %20
  %21 = phi i8 [ %25, %20 ], [ 4, %0 ]
  %22 = phi i8 [ %26, %20 ], [ %15, %0 ]
  %23 = phi i8 [ %28, %20 ], [ %19, %0 ]
  %24 = phi i16 [ %27, %20 ], [ %17, %0 ]
  %25 = add i8 %21, -1
  %26 = add i8 %22, -1
  %27 = xor i16 %24, 2040
  %28 = add i8 %26, %23
  %29 = icmp eq i8 %25, 0
  br i1 %29, label %30, label %20

30:                                               ; preds = %20
  %31 = xor i8 %26, %28
  %32 = zext i8 %31 to i16
  %33 = xor i16 %27, %32
  ret i16 %33
}
"#;
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let module = llrm_m68k::compile::mid_end(&module).expect("mid-end");
    llrm_m68k::dagisel::function::select_function(&module, "f", Cpu::M68000).expect("selects");
}

/// `movea.w #imm, %an` sign-extends, and the pin's `isUInt<16>` sent
/// 32768..65535 through it: an address register got 0xFFFF8000.. for 0x8000..
/// Seed 59 of the differential driver (a select of 49080 kept in an address
/// register).
#[test]
fn a_constant_above_32k_is_not_moved_into_an_address_register_by_a_word_move() {
    use llrm_m68k::reg::A0;
    for value in [32768, 49080, 65535] {
        let moves = llrm_m68k::expand::expand_movi(A0, value).expect("expands");
        assert!(moves.iter().all(|one| one.opcode != "MOV16ai"), "{value}: {moves:?}");
    }
    let moves = llrm_m68k::expand::expand_movi(A0, 32767).expect("expands");
    assert!(moves.iter().any(|one| one.opcode == "MOV16ai"), "{moves:?}");
}

/// `ir` (a function `f()` that reads the volatile byte `@zero`) built for the
/// 68000 and run under qemu with `zero` at offset 14 of the data block, as the
/// status byte it returns.
fn run_reading_zero(ir: &str) -> Option<i32> {
    use llrm_m68k::kernels::{Arg, program};
    use llrm_m68k::object::{executable, write_executable};
    if !std::path::Path::new("/usr/bin/qemu-m68k").exists() {
        return None;
    }
    let ir = ir.replace("ptr @zero", "ptr %z").replace("define i16 @f()", "define i16 @k(ptr %z)");
    let ir: String = ir
        .lines()
        .filter(|line| !line.starts_with("@zero =") && !line.starts_with("@buf ="))
        .collect::<Vec<_>>()
        .join("\n");
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let code = llrm_m68k::compile::items(&module, "k", Cpu::M68000).expect("compiles");
    let exe = support::unique_path("zero");
    write_executable(&exe, &executable(&program(&[Arg::Data(14)], code)).unwrap()).unwrap();
    let code = support::run(std::path::Path::new("qemu-m68k"), &["-cpu", "m68000"], &exe);
    std::fs::remove_file(&exe).ok();
    code
}

/// Two compares, each read by two `scc`, with the second compare between them:
/// the codes of the first were gone when its second reader came (the compare
/// has a folded volatile load, so it cannot run again), and the function was
/// rejected ("the condition codes of an arithmetic result are read past an
/// instruction that sets them"). The pin saves the codes as `copyPhysReg` does.
/// Seeds 23, 30, 41, 57, 62, 66 of the differential driver.
#[test]
fn the_codes_of_a_compare_that_cannot_run_again_are_saved_across_the_next_one() {
    let ir = r#"@zero = external constant i8, align 1
define i16 @f() {
  %1 = load volatile i8, ptr @zero, align 1
  %2 = load volatile i8, ptr @zero, align 1
  %3 = load volatile i8, ptr @zero, align 1
  %4 = load volatile i8, ptr @zero, align 1
  %5 = load volatile i8, ptr @zero, align 1
  %6 = load volatile i8, ptr @zero, align 1
  %7 = add i8 %4, -41
  %8 = zext i8 %5 to i16
  %9 = add i16 %8, 31052
  %10 = add i8 %6, -66
  %11 = icmp eq i8 %2, -1
  %12 = icmp eq i8 %1, -57
  %13 = select i1 %11, i1 true, i1 %12
  %14 = zext i1 %13 to i8
  %15 = add i8 %7, %14
  %16 = select i1 %13, i16 8, i16 0
  %17 = xor i16 %9, %16
  %18 = select i1 %13, i8 %15, i8 0
  %19 = add i8 %10, %18
  br label %20

20:                                               ; preds = %0, %20
  %21 = phi i8 [ %25, %20 ], [ 4, %0 ]
  %22 = phi i8 [ %26, %20 ], [ %15, %0 ]
  %23 = phi i8 [ %28, %20 ], [ %19, %0 ]
  %24 = phi i16 [ %27, %20 ], [ %17, %0 ]
  %25 = add i8 %21, -1
  %26 = add i8 %22, -1
  %27 = xor i16 %24, 2040
  %28 = add i8 %26, %23
  %29 = icmp eq i8 %25, 0
  br i1 %29, label %30, label %20

30:                                               ; preds = %20
  %31 = xor i8 %26, %28
  %32 = zext i8 %31 to i16
  %33 = xor i16 %27, %32
  ret i16 %33
}
"#;
    let text = compiled(ir, Options::default());
    assert!(text.contains("move.w\t%sr, %d") && text.contains("move.w\t%d") && text.contains(", %ccr"), "{text}");
    // the same function built for x86 by the pin's clang returns 143
    if let Some(got) = run_reading_zero(ir) {
        assert_eq!(got, 143);
    }
}

/// A compare read by a branch with a phi's copy between them that overwrites a
/// register the compare reads: the compare cannot run again, and `tst` of its
/// result is not the same codes. It was rejected ("no way to keep the flags of
/// CMP past a copy"); the pin saves the codes as `copyPhysReg` does. Seeds 23,
/// 30, 41, 181 of the differential driver.
#[test]
fn the_codes_of_a_compare_are_pushed_round_a_copy_that_cannot_move_or_be_repeated() {
    let ir = r#"@zero = external constant i8, align 1
@buf = external constant [16 x i8], align 1
define i16 @f() {
  %1 = load volatile i8, ptr @zero, align 1
  %2 = load volatile i8, ptr @zero, align 1
  %3 = load volatile i8, ptr @zero, align 1
  %4 = load volatile i8, ptr @zero, align 1
  %5 = load volatile i8, ptr @zero, align 1
  %6 = load volatile i8, ptr @zero, align 1
  %7 = load volatile i8, ptr @zero, align 1
  %8 = zext i8 %3 to i32
  %9 = add i32 %8, -1
  %10 = add i8 %5, -95
  %11 = add i8 %1, 29
  %12 = icmp ult i8 %11, -2
  %13 = sext i1 %12 to i8
  %14 = trunc i32 %9 to i8
  %15 = and i8 %10, %14
  %16 = add i8 %15, %13
  %17 = icmp eq i8 %2, -64
  %18 = zext i1 %17 to i32
  %19 = zext i8 %16 to i32
  %20 = zext i8 %10 to i32
  %21 = add i8 %7, -99
  %22 = zext i8 %6 to i32
  %23 = add i32 %22, 38068
  %24 = and i32 %9, 65535
  %25 = or i32 %24, %18
  %26 = shl i32 %25, 1
  %27 = lshr i32 %24, 15
  %28 = and i32 %26, 65534
  %29 = or disjoint i32 %28, %27
  %30 = xor i32 %29, %19
  %31 = shl i32 %30, 1
  %32 = lshr i32 %28, 15
  %33 = or disjoint i32 %31, %32
  %34 = zext i8 %21 to i32
  %35 = xor i32 %23, %20
  %36 = xor i32 %35, %34
  %37 = xor i32 %36, %33
  %38 = trunc i32 %37 to i16
  ret i16 %38
}
"#;
    let text = compiled(ir, SOURCE_ORDER);
    assert!(text.contains("move.w\t%sr, -(%sp)") && text.contains("move.w\t(%sp)+, %ccr"), "{text}");
    // the same function built for x86 by the pin's clang returns 55
    if let Some(got) = run_reading_zero(ir) {
        assert_eq!(got, 55);
    }
}

/// A copy loop steps its pointers with `addq` or `lea` as the pin's selection
/// does. `(An)+` came from rewrites of the expanded text and the loop pass's
/// preference, neither of which the pin has: every copy loop carried it.
#[test]
fn a_copy_loop_has_no_post_increment_as_the_pin_has_none() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source("copy")).expect("parses");
    let ours = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(!ours.contains("%a0)+") && !ours.contains("%a1)+") && !ours.contains("%a2)+"), "{ours}");
}
