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
    let t = |opcode, of: &str| Inst { opcode, operands: vec![register(of)] };
    let cases = [
        (t("TST8d", "d0"), "4a00"),
        (t("TST16d", "d5"), "4a45"),
        (t("TST32d", "d1"), "4a81"),
        (t("NOT16d", "d5"), "4645"),
        (Inst { opcode: "SWAP", operands: vec![register("d3"), register("d3")] }, "4843"),
    ];
    for (inst, golden) in cases {
        assert_eq!(hex(&encode(&inst).unwrap().bytes), golden, "{inst:?}");
    }
}

/// The pin's source-order scheduler. Two tests pin the code-saving fallback of
/// `flags.rs`, which only a schedule that separates a compare from its uses
/// reaches; the ILP list scheduler (the default) keeps them together and
/// retests.
const SOURCE_ORDER: Options = Options { mid_end: true, size: false, source_order: true };

fn compiled(
    ir: &str,
    options: Options,
) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    text_with(&module, "f", Cpu::M68000, options).expect("compiles")
}

fn branches(body: &str) -> String {
    let ir = format!(
        "define i32 @f(i32 %a, i32 %b) {{\nentry:\n  {body}\n  br i1 %c, label %y, label %n\ny:\n  ret i32 1\nn:\n  ret i32 2\n}}"
    );
    compiled(&ir, Options::default())
}

/// `lsr` sets Z from its result, but the pin has no `optimizeCompareInstr` for
/// M68k: the popcount loop ends `lsr.l #1, %d1; cmpi.l #0, %d1; bne`, the
/// compare stays (the pin has no `tst` either).
#[test]
fn a_test_right_after_the_instruction_that_set_the_flags_stays_as_the_pin_has_it() {
    let asm = branches("%s = lshr i32 %a, 1\n  %c = icmp ne i32 %s, 0");
    assert!(asm.contains("lsr.l") && asm.contains("cmpi.l\t#0,"), "{asm}");
}

/// The pin compares a sum with zero by `cmpi.l #0` for an ordering, but reads
/// the codes of the `add` itself for equality and for `slt 0` (`N` alone, the
/// sign of the wrapped sum: `add; bmi`).
#[test]
fn a_compare_with_zero_after_an_add_is_the_pins_cmpi_for_an_ordering() {
    let signed = branches("%s = add i32 %a, %b\n  %c = icmp sgt i32 %s, 0");
    assert!(signed.contains("cmpi.l\t#0,"), "{signed}");
    let sign = branches("%s = add i32 %a, %b\n  %c = icmp slt i32 %s, 0");
    assert!(!sign.contains("cmpi") && sign.contains("add.l"), "{sign}");
    let equal = branches("%s = add i32 %a, %b\n  %c = icmp ne i32 %s, 0");
    assert!(!equal.contains("cmpi"), "{equal}");
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

fn loop_moves(asm: &str) -> usize {
    let body = asm.split(".LBB0_1:").nth(1).expect("a loop");
    body.lines().take_while(|line| !line.contains("blt")).filter(|line| line.contains("move.l")).count()
}

/// The loop that carries `(a, b) <- (b, a + b)` is three moves as the pin has
/// it: the sum takes `a`'s register and the pair rotates through a temporary.
/// Tying the sum to `b` (commuting `ADD32dr`, which the pin does not mark
/// commutable) left two.
#[test]
fn a_loop_carrying_a_sum_rotates_its_pair_in_three_moves_as_the_pin_does() {
    let ir = "define i32 @f(i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %a = phi i32 [0, %entry], [%b, %l]\n  %b = phi i32 [1, %entry], [%s, %l]\n  %s = add i32 %a, %b\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %s\n}";
    assert_eq!(loop_moves(&compiled(ir, Options::PINNED)), 3, "{}", compiled(ir, Options::PINNED));
    let ours = compiled(ir, Options::default());
    assert_eq!(loop_moves(&ours), 3, "{ours}");
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

/// A load read twice is loaded once: swapping would load it again.
#[test]
fn a_loaded_value_read_again_is_not_swapped_into_the_compare() {
    let ir = "define i32 @f(ptr %p, i32 %t) {\n  %v = load i32, ptr %p\n  %c = icmp sgt i32 %v, %t\n  %z = zext i1 %c to i32\n  %r = add i32 %z, %v\n  ret i32 %r\n}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("cmp.l\t(%a0)"), "{ours}");
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

/// A frame of saved registers and an outgoing-argument area is the pin's
/// `suba.l #20, %sp; movem.l regs, (8,%sp)` and back. Pushing the registers and
/// taking only the rest off the stack pointer (12 B instead of 24) was a
/// different prologue that the pin's `M68kFrameLowering` does not emit.
#[test]
fn saved_registers_are_stored_into_the_carved_frame_as_the_pin_does() {
    let ir = llrm_m68k::kernels::source_in("library", "fnptr");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let ours = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(ours.contains("suba.l\t#20, %sp\n\tmovem.l\t%d2-%d3/%a2, (8,%sp)"), "{ours}");
    assert!(!ours.contains("-(%sp)"), "{ours}");
}

fn library(name: &str) -> String {
    let text = std::fs::read_to_string(format!("{}/bench/library/{name}.ll", env!("CARGO_MANIFEST_DIR"))).unwrap();
    text.lines()
        .filter(|line| !line.starts_with("target datalayout"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("@k(", "@f(")
}

/// muldiv reads `b` three times, as the outgoing arguments of three calls:
/// folded, each is a 6 B memory-to-memory `move` where the register form is 4,
/// and the function grew from 68 to 78 B.
#[test]
fn an_argument_read_three_times_keeps_its_register() {
    let text = compiled(&library("muldiv"), Options::default());
    assert!(!text.contains(",%sp), ("), "{text}");
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
    assert!(text.contains("move.l\t(12,%sp), %d"), "{text}");
    assert!(!text.contains("(15,%sp)"), "{text}");
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

/// A counting loop adds and compares with the pin's `add.l #1` and `cmpi.l`.
/// `addq`, `subq`, `tst` and `cmpa` came from a rewrite of the expanded
/// instructions; the pin marks `ADDQ` and `SUBQ` as not implemented and has no
/// `TST`.
#[test]
fn a_counting_loop_uses_no_quick_or_test_forms_as_the_pin_has_none() {
    let ir = "define i32 @f(i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %i1 = add i32 %i, 1\n  %e = icmp ult i32 %i1, %n\n  br i1 %e, label %l, label %x\nx:\n  ret i32 %i1\n}";
    let ours = compiled(ir, Options::default());
    assert!(
        ours.contains("add.l\t#1,") && !ours.contains("addq") && !ours.contains("subq") && !ours.contains("tst."),
        "{ours}"
    );
}

/// An argument read by a compare is loaded into a register and compared from
/// there, as the pin's selection does. Reading it from its slot (`cmp.l
/// (16,%sp), %d1`) was a fold of the load after expansion that the pin's M68k,
/// with no `foldMemoryOperandImpl`, does not make.
#[test]
fn an_argument_read_by_a_compare_is_loaded_first_as_the_pin_does() {
    let size = Options { size: true, ..Options::default() };
    let text = compiled(&library("bsearch"), size);
    assert!(!text.contains("cmp.l\t(16,%sp), %d"), "{text}");
}

/// A compare keeps its operands as the IR gives them, as the pin's `LowerSETCC`
/// does: a load on the left is loaded into a register first. Swapping the
/// operands (and the condition) to fold the load into the `cmp` was a
/// deviation.
#[test]
fn a_loaded_value_compared_with_a_register_is_loaded_first_as_the_pin_does() {
    let ir = "define i32 @f(ptr %p, i32 %t) {\nentry:\n  %v = load i32, ptr %p\n  %u = add i32 %t, 100000\n  %c = icmp sgt i32 %v, %u\n  br i1 %c, label %y, label %n\ny:\n  ret i32 1\nn:\n  ret i32 2\n}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("cmp.l\t(%a0)"), "{ours}");
}
