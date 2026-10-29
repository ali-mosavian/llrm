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

/// `add.l #-1` took 6 bytes where `subq.l #1` takes 2: strlen ended with it.
#[test]
fn adding_a_small_negative_is_a_subq() {
    let ir = "define i32 @f(i32 %a) {\nentry:\n  %r = add i32 %a, -1\n  %c = add i32 %r, -8\n  %d = sub i32 %c, -3\n  ret i32 %d\n}";
    let pinned = compiled(ir, Options::PINNED);
    let ours = compiled(ir, Options::default());
    assert!(pinned.contains("add.l\t#-1") || pinned.contains("add.l\t#-"), "{pinned}");
    assert!(ours.contains("subq.l") && !ours.contains("add.l\t#-"), "{ours}");
}

/// strlen's loop ended `move.l %d0, %d1; move.l %d1, %d0; addq.l #1, %d0`: the
/// second move put the value back where it already was. Then `move.l %d0,
/// %d1; addq.l #1, %d0; tst.b (1,%a0,%d1)` kept the counter and its successor
/// apart, a move more every trip than gcc's: the access reads the successor
/// with the step taken from the displacement, and only the argument is loaded.
#[test]
fn a_loop_counter_is_not_copied_out_and_back() {
    let ir = "define i32 @f(ptr %p) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %q = getelementptr i8, ptr %p, i32 %i\n  %v = load i8, ptr %q\n  %z = icmp eq i8 %v, 0\n  %i1 = add i32 %i, 1\n  br i1 %z, label %x, label %l\nx:\n  ret i32 %i\n}";
    let asm = compiled(ir, Options::default());
    assert_eq!(asm.matches("move.l").count(), 1, "{asm}");
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

/// `s += p[i] > t` ended `sgt %d1; and.l #255, %d1; and.l #1, %d1`: the second
/// mask makes the first one dead.
#[test]
fn two_masks_of_one_register_in_a_row_are_one() {
    let ir = "define i32 @f(i32 %a, i32 %b, i32 %s) {\n  %c = icmp sgt i32 %a, %b\n  %z = zext i1 %c to i32\n  %r = add i32 %s, %z\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("and.l\t#255") && pinned.contains("and.l\t#1,"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("and.l\t#1,") && !ours.contains("and.l\t#255"), "{ours}");
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

/// The load of the running maximum's array stepped its pointer after the
/// select's join (`move.l (%a0), %d2 ... addq.l #4, %a0`, 2 bytes more than
/// `(%a0)+`), because the scan stopped at the branch over the select's arm.
#[test]
fn a_step_past_a_branch_over_one_arm_is_still_a_post_increment() {
    let ours = compiled(arrmax_loop(), Options::default());
    assert!(ours.contains("move.l\t(%a0)+, %d") && !ours.contains("addq.l\t#4"), "{ours}");
}

/// A block inside the span that something outside also jumps to would reach
/// the step with no access before it: the step stays.
#[test]
fn a_step_after_a_label_entered_from_outside_the_span_stays() {
    use llrm_m68k::expand::Line;
    let inst = |opcode, operands| Line::Inst(Inst { opcode, operands });
    let lines = vec![
        inst("Beq8", vec![Operand::Symbol(".LBB0_3".into(), 0)]),
        Line::Label(1),
        inst("MOV32rj", vec![register("d2"), register("a0")]),
        Line::Label(3),
        inst("ADDQ32a", vec![register("a0"), register("a0"), Operand::Imm(4)]),
    ];
    let folded = llrm_m68k::overlay::fold_post_increment(lines);
    assert!(matches!(&folded[2], Line::Inst(access) if access.opcode == "MOV32rj"));
    assert_eq!(folded.len(), 5);
}

/// `p[i + 1]` through `(d8,An,Xn)`: the base class's `isLegalAddressingMode`
/// refuses a base, an index and a displacement together, the hardware does
/// not. The description of the pin's output keeps the refusal; the overlay's
/// takes the byte displacement, and only that range.
#[test]
fn the_indexed_displacement_is_priced_only_in_the_overlay_description() {
    use llrm_m68k::machine::M68k;
    use llrm_mir::tti::Tti;
    let pinned = M68k::default();
    let ours = M68k { indexed_displacement: true, ..M68k::default() };
    assert!(!pinned.is_legal_addressing_mode(false, 1, true, 1));
    assert!(ours.is_legal_addressing_mode(false, 1, true, 1));
    assert!(ours.is_legal_addressing_mode(false, -128, true, 1) && ours.is_legal_addressing_mode(false, 127, true, 1));
    assert!(
        !ours.is_legal_addressing_mode(false, 128, true, 1) && !ours.is_legal_addressing_mode(false, -129, true, 1)
    );
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

/// popcount's `x & 1` of a copy was `move.l %d1, %d2; and.l #1, %d2`, 8 bytes;
/// `moveq #1, %d2; and.l %d1, %d2` is 4 with the same value and codes.
#[test]
fn a_small_constant_mask_of_a_copy_is_a_moveq_and_a_register_and() {
    let ir = "define i32 @f(i32 %a) {\n  %m = and i32 %a, 1\n  %r = add i32 %m, %a\n  ret i32 %r\n}";
    let pinned = compiled(ir, Options::PINNED);
    assert!(pinned.contains("and.l\t#1,"), "{pinned}");
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("moveq\t#1,") && !ours.contains("and.l\t#1,"), "{ours}");
    assert!(code_size(ir, Options::default()) < code_size(ir, Options::PINNED));
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

/// The loop strength reduction priced `(d8, An, Xn)` with a displacement as
/// illegal, as the pin's base class does, so strlen's counter was fixed up
/// after the loop (`subq.l #1`) and the test read `(0,%a0,%d1)` from a copy.
/// With the byte displacement priced the fix-up folds into the address.
#[test]
fn an_index_with_a_byte_displacement_is_priced_legal_for_the_loop_pass() {
    let ir = "define i32 @f(ptr %p) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %q = getelementptr i8, ptr %p, i32 %i\n  %v = load i8, ptr %q\n  %z = icmp eq i8 %v, 0\n  %i1 = add i32 %i, 1\n  br i1 %z, label %x, label %l\nx:\n  ret i32 %i\n}";
    let ours = compiled(ir, Options::default());
    assert!(!ours.contains("subq.l\t#1"), "{ours}");
    let pinned_pricing = {
        let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).unwrap();
        let reduced = llrm_m68k::compile::mid_end_described_by(&module, &llrm_m68k::machine::M68k::default(), None);
        llrm_mir::print::module(&reduced.unwrap())
    };
    assert!(pinned_pricing.contains("add i32") && pinned_pricing.contains("-1"), "{pinned_pricing}");
}

/// A byte copy walked two pointers by one shared index register and a `move.b
/// (%a0,%d1), (%a1,%d1)`, 46 bytes; the loop is `move.b (%a0)+, (%a2)+` with
/// neither pointer stepped apart (once the text printed `(%a2)+` while the
/// code stepped it again).
#[test]
fn a_byte_copy_loop_steps_both_pointers_in_the_move() {
    let ir = "define i32 @f(ptr %d, ptr %s, i32 %n) {
entry:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %loop, label %done
loop:
  %p = phi ptr [ %pn, %loop ], [ %d, %entry ]
  %i = phi i32 [ %in, %loop ], [ %n, %entry ]
  %q = phi ptr [ %qn, %loop ], [ %s, %entry ]
  %in = add i32 %i, -1
  %qn = getelementptr i8, ptr %q, i32 1
  %v = load i8, ptr %q
  %pn = getelementptr i8, ptr %p, i32 1
  store i8 %v, ptr %p
  %again = icmp ugt i32 %i, 1
  br i1 %again, label %loop, label %done
done:
  %e = getelementptr i8, ptr %d, i32 12
  %r = load i8, ptr %e
  %x = sext i8 %r to i32
  ret i32 %x
}";
    let ours = compiled(ir, Options::default());
    assert!(ours.contains("move.b\t(%a0)+, (%a2)+"), "{ours}");
    assert!(!ours.contains("addq.l\t#1, %a"), "{ours}");
}

fn counted_scan(ty: &str, step: &str) -> String {
    format!(
        "define i32 @f(ptr %p) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %q = getelementptr {ty}, ptr %p, i32 %i
  %v = load {ty}, ptr %q
  %i1 = add i32 %i, {step}
  %c = icmp ne {ty} %v, 0
  br i1 %c, label %loop, label %done
done:
  ret i32 %i
}}"
    )
}

/// A loop that counts and reads through a pointer it also steps: the step
/// came before the load of the old pointer (a copy of it), then the compare
/// that reads it, so the step stayed apart (`addq.l #1, %a0` and a copy) and
/// the loop ran four instructions a trip. The step goes after the access and
/// folds into it, for a byte and for a word.
#[test]
fn a_pointer_stepped_before_the_compare_that_reads_it_is_post_incremented() {
    for (ty, access, bytes) in [("i8", "tst.b\t(%a0)+", 1), ("i16", "tst.w\t(%a0)+", 2)] {
        let ours = compiled(&counted_scan(ty, "1"), Options::default());
        assert!(ours.contains(access), "{ty}: {ours}");
        assert!(!ours.contains(&format!("addq.l\t#{bytes}, %a")), "{ty}: {ours}");
    }
}

fn after_mid_end(ir: &str) -> String {
    use std::rc::Rc;
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let tti = llrm_m68k::machine::M68k { indexed_displacement: true, post_indexed: true, ..Default::default() };
    let done = llrm_m68k::compile::mid_end_described_by(&module, &tti, Some(Rc::new(llrm_m68k::machine::M68kMachine)))
        .expect("the mid-end runs");
    llrm_mir::print::module(&done)
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
    // bench/library/memcpy.ll: pointers and a down counter, and the pin keeps them
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

fn walk_with_call(ty: &str) -> String {
    format!(
        "declare void @g(i32)
define i32 @f(ptr %p, i32 %n) {{
entry:
  br label %head
head:
  %i = phi i32 [ 0, %entry ], [ %i1, %latch ]
  %q = getelementptr {ty}, ptr %p, i32 %i
  %v = load {ty}, ptr %q
  %z = zext {ty} %v to i32
  call void @g(i32 %z)
  br label %latch
latch:
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %head, label %done
done:
  ret i32 %i1
}}"
    )
}

/// The step of a new pointer for a post-indexed load goes in the load's block
/// (`getFixupInsertPos`), not the latch's, where the loop's counter steps; for
/// a byte and for a word.
#[test]
fn the_step_of_a_post_indexed_pointer_is_made_beside_its_load() {
    for ty in ["i8", "i16"] {
        let done = after_mid_end(&walk_with_call(ty));
        let head = done.split("head:").nth(1).and_then(|rest| rest.split("latch:").next()).expect("a head block");
        assert!(head.contains("getelementptr"), "{ty}: {done}");
        assert!(head.contains("load"), "{ty}: {done}");
    }
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
