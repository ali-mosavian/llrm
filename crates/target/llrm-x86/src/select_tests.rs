//! Ports of tests/test_select.py and the select-only tests elsewhere.
//! Python's `str(insn)` checks read iced's decoded fields instead, or the
//! bytes Python printed: this crate builds iced without a formatter.

use llrm_lir::Addr;
use llrm_lir::registers::RegId;

use super::sweep_support::{a, h, rg, sem};
use super::*;

/// The instruction an encoding decodes to, in real mode.
struct Seen {
    insn: Instruction,
}

fn seen(
    code: &[u8],
    at: u64,
) -> Option<Seen> {
    Some(Seen { insn: decoded(code, at) })
}
use iced_x86::{Mnemonic, OpKind};

fn decoded(
    code: &[u8],
    ip: u64,
) -> Instruction {
    Decoder::with_ip(BITNESS, code, ip, DecoderOptions::NONE).decode()
}

fn hex(code: &[u8]) -> String {
    code.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn made(what: Option<Emitted>) -> Emitted {
    what.expect("the emitter returned None")
}

fn emitted(what: &Semantics) -> Option<Emitted> {
    emit(what, 0, None, false, false, None)
}

fn frame(
    disp: i64,
    width: u32,
) -> ir::Mem {
    ir::Mem::new(Some(Addr::new(Space::Frame, disp)), width)
}

fn at(
    space: Space,
    disp: i64,
    base: RegId,
    segment: RegId,
) -> Addr {
    a(space, disp, 0, base, segment)
}

fn imm(
    value: i64,
    width: u32,
) -> Loc {
    Loc::Imm(ir::Imm { value, width, address: None })
}

fn st(index: u32) -> Loc {
    Loc::st(index)
}

const ROOTS: [RegId; 6] = [RegId::EAX, RegId::ECX, RegId::EDX, RegId::EBX, RegId::ESI, RegId::EDI];

#[test]
fn test_pl_move_exchange_with_frame_memory() {
    for (register, width) in [(RegId::AL, 1), (RegId::AX, 2), (RegId::EAX, 4)] {
        for memory_first in [false, true] {
            let cell = Loc::Mem(frame(-28, width));
            let held = rg(register, width);
            let dests = if memory_first { vec![cell, held] } else { vec![held, cell] };
            let sources = dests.iter().rev().cloned().collect();
            let emitted = made(emitted(&sem(Operation::Exchange, Some("xchg"), dests, sources, None, false)));
            let instruction = decoded(&emitted.code, 0);
            assert_eq!(Some(instruction.code()), _code(&format!("XCHG_RM{}_R{}", width * 8, width * 8)));
            assert_eq!(instruction.memory_base(), (RegId::BP).iced());
            assert_eq!(instruction.memory_displacement64() & 0xFFFF, 0xFFE4);
            assert_eq!(instruction.op1_register(), register.iced());
        }
    }
}

#[test]
fn test_byte_copy_for_nbody_timer() {
    for source in [RegId::CL, RegId::AH, RegId::BL, RegId::DH] {
        let what = sem(Operation::Move, Some("mov"), vec![rg(RegId::AL, 1)], vec![rg(source, 1)], None, false);
        let instruction = decoded(&made(emitted(&what)).code, 0);
        assert_eq!(instruction.code(), Code::Mov_r8_rm8);
        assert_eq!(instruction.op0_register(), (RegId::AL).iced());
        assert_eq!(instruction.op1_register(), source.iced());
    }
}

#[test]
fn test_signed_word_extension_uses_explicit_operands() {
    let what = sem(Operation::Extend, Some("movsx"), vec![rg(RegId::EBX, 4)], vec![rg(RegId::SI, 2)], None, false);
    let instruction = decoded(&made(emitted(&what)).code, 0);
    assert_eq!(instruction.code(), Code::Movsx_r32_rm16);
    assert!(instruction.op0_register() == (RegId::EBX).iced() && instruction.op1_register() == (RegId::SI).iced());
}

#[test]
fn test_load_accepts_unsigned_dword_bit_pattern() {
    assert_eq!(hex(&made(load(RegId::ESI, 0xBFFFFFF9, At::bits16(0))).code), "66bef9ffffbf");
}

#[test]
fn test_push_accepts_unsigned_dword_bit_patterns() {
    for value in [0x80000000_i64, 0xEDCBA987, 0xFFFFFFFF] {
        let instruction = decoded(&made(push_imm(value, 4, At::bits16(0), false)).code, 0);
        assert_eq!(instruction.stack_pointer_increment(), -4);
        assert_eq!(instruction.immediate(0) & 0xFFFFFFFF, value as u64);
    }
}

#[test]
fn test_store_accepts_unsigned_dword_bit_pattern() {
    assert_eq!(hex(&made(store_imm(&frame(-4, 4), 0xC1747C23, At::bits16(0))).code), "66c746fc237c74c1");
}

#[test]
fn test_a_move_decodes_back_to_the_move_that_was_asked_for() {
    for into in ROOTS {
        for outof in ROOTS {
            if into == outof {
                continue;
            }
            let back = decoded(&made(r#move(into, outof, At::bits16(0))).code, 0);
            assert_eq!(back.code(), Code::Mov_r32_rm32);
            assert_eq!(back.op0_register(), into.iced());
            assert_eq!(back.op1_register(), outof.iced());
        }
    }
}

#[test]
fn test_a_sixteen_bit_move_is_shorter_than_a_thirty_two_bit_one() {
    let wide = made(r#move(RegId::ECX, RegId::EAX, At::bits16(0)));
    let narrow = made(r#move(RegId::CX, RegId::AX, At::bits16(0)));
    assert!(wide.code.len() == 3 && narrow.code.len() == 2);
    assert_eq!(wide.code[0], 0x66);
}

#[test]
fn test_a_move_to_itself_is_no_instruction() {
    assert!(made(r#move(RegId::EAX, RegId::EAX, At::bits16(0))).code.is_empty());
}

#[test]
fn test_mixed_widths_are_refused_rather_than_guessed() {
    assert!(r#move(RegId::ECX, RegId::AX, At::bits16(0)).is_none());
    assert!(r#move(RegId::AX, RegId::ECX, At::bits16(0)).is_none());
}

#[test]
fn test_registers_this_does_not_name_are_refused() {
    assert!(r#move(RegId::EAX, RegId::ES, At::bits16(0)).is_none());
    assert!(r#move(RegId::ES, RegId::EAX, At::bits16(0)).is_none());
    assert!(r#move(RegId::EBP, RegId::ESP, At::bits16(0)).is_some());
}

#[test]
fn test_a_wide_push_is_not_a_narrow_one() {
    let (narrow, wide) = (made(push_imm(3, 2, At::bits16(0), false)), made(push_imm(3, 4, At::bits16(0), false)));
    assert_eq!(decoded(&narrow.code, 0).stack_pointer_increment(), -2);
    assert_eq!(decoded(&wide.code, 0).stack_pointer_increment(), -4);
}

#[test]
fn test_a_register_is_not_an_immediate() {
    let one = made(push(RegId::EAX, At::bits16(0)));
    assert_eq!(one.code, [0x66, 0x50]);
    let other = made(push_imm((RegId::EAX).index() as i64, 2, At::bits16(0), false));
    assert_ne!(other.code, one.code);
}

#[test]
fn test_a_mixed_width_operation_is_refused() {
    assert!(arith("add", RegId::AX, RegId::ECX, At::bits16(0)).is_none());
    assert!(arith("add", RegId::EAX, RegId::CX, At::bits16(0)).is_none());
}

#[test]
fn test_an_operation_not_in_the_table_is_refused() {
    for name in ["rol", "shl", "imul", "xchg", ""] {
        assert!(arith(name, RegId::AX, RegId::CX, At::bits16(0)).is_none());
    }
    for name in ["bswap", "shr", ""] {
        assert!(unary(name, RegId::AX, At::bits16(0)).is_none());
    }
}

#[test]
fn test_a_relocated_address_is_emitted_as_zero_and_says_where() {
    let cell = ir::Mem::new(Some(a(Space::Segment, 0x1234, 5, RegId::None, RegId::None)), 2);
    let made = made(move_from(RegId::AX, &cell, At::bits16(0)));
    let at = made.displacement_at.expect("a displacement");
    assert_eq!(made.code[at..at + 2], [0, 0], "a relocated displacement is not a number");
}

#[test]
fn test_a_frame_slot_keeps_its_displacement() {
    let made = made(move_from(RegId::AX, &frame(-0x18, 2), At::bits16(0)));
    assert_eq!(hex(&made.code), "8b46e8"); // mov ax,[bp-18h]
    let at = made.displacement_at.expect("a displacement");
    assert_eq!(made.code[at..], [(-0x18_i64 & 0xFF) as u8]);
}

#[test]
fn test_the_spaces_that_cannot_be_encoded_are_refused() {
    for space in [Space::Far, Space::Group, Space::Stack] {
        assert!(operand_of(&ir::Mem::new(Some(Addr::new(space, 4)), 2), 16).is_none(), "{space:?}");
    }
    assert!(operand_of(&ir::Mem::new(None, 2), 16).is_none());
    let literal = operand_of(&ir::Mem::new(Some(Addr::new(Space::Literal, 4)), 2), 16);
    assert!(literal.is_some_and(|(_, relocated)| !relocated), "a literal address relocates nothing");
}

#[test]
fn test_a_cell_of_the_wrong_width_is_refused() {
    let cell = frame(-4, 4);
    assert!(move_from(RegId::AX, &cell, At::bits16(0)).is_none());
    assert!(move_from(RegId::EAX, &cell, At::bits16(0)).is_some());
}

#[test]
fn test_a_near_call_and_a_far_call_are_told_apart_by_the_target() {
    let near = made(call_near(0x32, At::bits16(0x4E)));
    let far = made(call_far(At::bits16(0x4E)));
    assert!(near.displacement_at.is_none());
    assert_eq!(far.displacement_at, Some(1));
    assert_eq!(far.code, [0x9A, 0, 0, 0, 0]);
    let back = decoded(&near.code, 0x4E); // call 0032h
    assert_eq!(back.mnemonic(), Mnemonic::Call);
    assert_eq!(back.near_branch16(), 0x32);
}

#[test]
fn test_a_store_of_an_immediate_takes_the_cell_s_width() {
    // `word ptr` and `dword ptr`, as the bytes Python printed.
    for (width, want) in [(2, "c746fc0000"), (4, "66c746fc00000000")] {
        assert_eq!(hex(&made(store_imm(&frame(-4, width), 0, At::bits16(0))).code), want);
    }
}

#[test]
fn test_a_narrow_push_of_a_small_literal_takes_the_byte_form() {
    for (value, want) in [(0, "6a00"), (1, "6a01"), (0x31, "6a31"), (-1, "6aff"), (127, "6a7f"), (-128, "6a80")] {
        assert_eq!(hex(&made(push_imm(value, 2, At::bits16(0), false)).code), want);
    }
}

#[test]
fn test_a_narrow_push_that_does_not_fit_a_byte_stays_wide() {
    for value in [128, -129, 1000, -1000, 0x7FFF, -0x8000] {
        let made = made(push_imm(value, 2, At::bits16(0), false));
        assert_eq!(made.code[0], 0x68, "{value} took the byte form and does not fit one");
        assert_eq!(made.code.len(), 3);
    }
}

#[test]
fn test_a_word_immediate_written_unsigned_still_fits_a_byte() {
    assert_eq!(hex(&made(push_imm(0xFFFF, 2, At::bits16(0), false)).code), "6aff");
    assert_eq!(hex(&made(push_imm(0xFF80, 2, At::bits16(0), false)).code), "6a80");
    assert_eq!(hex(&made(arith_imm("add", RegId::BX, 0xFFFF, At::bits16(0), false)).code), "83c3ff");
}

#[test]
fn test_a_literal_address_through_a_register_uses_the_byte_displacement() {
    let cell = ir::Mem {
        through: RegId::SI,
        offset: 0x0A,
        ..ir::Mem::new(Some(at(Space::Literal, 0x0A, RegId::SI, RegId::None)), 2)
    };
    assert_eq!(hex(&made(arith_mem("add", RegId::BX, &cell, At::bits16(0))).code), "035c0a");
}

#[test]
fn test_a_bare_literal_address_keeps_two_bytes_however_small_it_is() {
    for value in [0, 1, 0x10, 0x7F] {
        let cell = ir::Mem::new(Some(Addr::new(Space::Literal, value)), 2);
        let made = made(arith_mem("add", RegId::BX, &cell, At::bits16(0)));
        assert_eq!(made.code.len(), 4, "{value:#x} came back {}", hex(&made.code));
        assert_eq!(made.code[1] & 0xC7, 0x06, "{value:#x} is not the direct-address form");
    }
}

#[test]
fn test_a_relocated_push_keeps_the_wide_immediate() {
    assert_eq!(hex(&made(push_imm(0, 2, At::bits16(0), true)).code), "680000");
    assert_eq!(hex(&made(push_imm(0, 2, At::bits16(0), false)).code), "6a00");
    assert_eq!(hex(&made(push_imm(3, 4, At::bits16(0), true)).code), "666803000000");
    assert_eq!(hex(&made(push_imm(3, 4, At::bits16(0), false)).code), "666a03");
}

#[test]
fn test_a_shift_by_one_takes_its_own_opcode() {
    for (name, reg, want) in [("shl", RegId::AX, "d1e0"), ("shl", RegId::BX, "d1e3"), ("sar", RegId::AX, "d1f8")] {
        assert_eq!(hex(&made(shift(name, RegisterOrCell::Reg(reg), Some(1), At::bits16(0))).code), want);
    }
}

#[test]
fn test_a_shift_by_more_than_one_keeps_the_immediate_form() {
    assert_eq!(hex(&made(shift("shl", RegisterOrCell::Reg(RegId::AX), Some(3), At::bits16(0))).code), "c1e003");
}

#[test]
fn test_spilled_shift_is_encodable() {
    for (count, hex_bytes) in [(Some(1), "d166de"), (Some(3), "c166de03"), (None, "d366de")] {
        let cell = Loc::Mem(frame(-0x22, 2));
        let source = match count {
            None => rg(RegId::CL, 1),
            Some(count) => imm(count, 1),
        };
        let what = sem(Operation::Binary, Some("shl"), vec![cell.clone()], vec![cell, source], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), hex_bytes);
    }
}

#[test]
fn test_an_accumulator_immediate_takes_the_short_opcode() {
    for (name, value, want) in [("add", 0x1286, "058612"), ("cmp", 0x1234, "3d3412"), ("sub", 0x4000, "2d0040")] {
        assert_eq!(hex(&made(arith_imm(name, RegId::AX, value, At::bits16(0), false)).code), want);
    }
}

#[test]
fn test_a_byte_immediate_still_beats_the_accumulator_form() {
    assert_eq!(hex(&made(arith_imm("add", RegId::AX, 3, At::bits16(0), false)).code), "83c003");
    assert_eq!(hex(&made(arith_imm("add", RegId::BX, 3, At::bits16(0), false)).code), "83c303");
    assert_eq!(hex(&made(arith_imm("add", RegId::BX, 0x1286, At::bits16(0), false)).code), "81c38612");
}

#[test]
fn test_a_compare_of_memory_against_a_small_literal_takes_the_byte_form() {
    for (value, want) in [(0x32, "837ee232"), (0, "837ee200"), (-1, "837ee2ff")] {
        assert_eq!(hex(&made(compare(&Loc::Mem(frame(-0x1E, 2)), value, At::bits16(0), false)).code), want);
    }
}

#[test]
fn test_a_compare_of_a_byte_cell_against_a_literal() {
    for (value, want) in [(0, "807efc00"), (200, "807efcc8"), (-1, "807efcff")] {
        assert_eq!(hex(&made(compare(&Loc::Mem(frame(-4, 1)), value, At::bits16(0), false)).code), want);
    }
}

#[test]
fn test_an_extension_encodes_from_a_byte_or_a_cell() {
    let cell = |width| Loc::Mem(ir::Mem { through: RegId::BP, disp_width: 2, ..frame(-4, width) });
    for (name, dest, operand, want) in [
        ("movzx", RegId::BX, cell(1), "0fb65efc"),
        ("movsx", RegId::AX, cell(1), "0fbe46fc"),
        ("movzx", RegId::EBX, cell(2), "660fb75efc"),
        ("movzx", RegId::EBX, rg(RegId::BX, 2), "660fb7db"),
    ] {
        let width = WIDTHS[&dest] as u32;
        let what = sem(Operation::Extend, Some(name), vec![rg(dest, width)], vec![operand], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), want);
    }
}

#[test]
fn test_a_compare_of_memory_against_a_large_literal_stays_wide() {
    assert_eq!(hex(&made(compare(&Loc::Mem(frame(-0x1E, 2)), 0x1234, At::bits16(0), false)).code), "817ee23412");
}

#[test]
fn test_a_relocated_arithmetic_immediate_keeps_its_width() {
    for name in ["add", "sub", "cmp", "and", "or", "xor"] {
        let wide = made(arith_imm(name, RegId::AX, 0, At::bits16(0), true));
        assert_eq!(wide.code.len(), 3, "{name} ax,0 relocated came back {}", hex(&wide.code));
        assert!(wide.immediate_at.is_some());
        assert_ne!(wide.code[0], 0x83);
        let narrow = made(arith_imm(name, RegId::AX, 0, At::bits16(0), false));
        assert_eq!(narrow.code[0], 0x83);
    }
}

#[test]
fn test_a_relocated_immediate_against_memory_keeps_its_width() {
    let cell = frame(-4, 2);
    let wide = made(arith_into_imm("add", &cell, 0, At::bits16(0), true));
    assert_eq!(wide.code[0], 0x81, "came back {}", hex(&wide.code));
    let narrow = made(arith_into_imm("add", &cell, 0, At::bits16(0), false));
    assert_eq!(narrow.code[0], 0x83);
}

#[test]
fn test_a_comparison_against_zero_takes_the_test_form() {
    for (reg, want) in [(RegId::EAX, "6685c0"), (RegId::ECX, "6685c9"), (RegId::AX, "85c0"), (RegId::BX, "85db")] {
        let width = if matches!(reg, RegId::EAX | RegId::ECX) { 4 } else { 2 };
        assert_eq!(hex(&made(compare(&rg(reg, width), 0, At::bits16(0), false)).code), want);
    }
}

#[test]
fn test_a_comparison_against_zero_stays_a_compare_when_relocated() {
    let made = made(compare(&rg(RegId::AX, 2), 0, At::bits16(0), true));
    assert_ne!(made.code[0], 0x85, "a relocated compare became a test: {}", hex(&made.code));
}

/// A cell's index register was left out of a remap an address's was given: the
/// same register, renamed in one operand and not the other.
#[test]
fn test_a_remap_renames_a_cells_index_register_as_an_addresss() {
    let r#where: RegisterMap = [(RegId::SI, RegId::DI)].into_iter().collect();
    let cell = ir::Mem { through: RegId::BX, index_through: RegId::SI, ..ir::Mem::new(None, 2) };
    let address = ir::AddressRef { through: RegId::BX, index_through: RegId::SI, ..ir::AddressRef::new(None) };
    for place in [Loc::Mem(cell), Loc::Address(address)] {
        let moved = _operand(&place, Some(&r#where), None);
        assert_eq!(moved.address().map(|one| one.index_through), Some(RegId::DI), "{moved:?}");
    }
}

#[test]
fn test_a_remap_reaches_inside_a_memory_operand() {
    let r#where: RegisterMap = [(RegId::SI, RegId::DI), (RegId::ESI, RegId::EDI)].into_iter().collect();
    let cell = ir::Mem {
        through: RegId::SI,
        offset: 0x0A,
        disp_width: 1,
        ..ir::Mem::new(Some(at(Space::Literal, 0x0A, RegId::SI, RegId::None)), 2)
    };
    let what = sem(
        Operation::Binary,
        Some("add"),
        vec![rg(RegId::BX, 2)],
        vec![rg(RegId::BX, 2), Loc::Mem(cell)],
        None,
        false,
    );
    let plain = made(emitted(&what));
    let moved = made(emit(&what, 0, Some(Where::One(&r#where)), false, false, None));
    assert_ne!(plain.code, moved.code, "the remap never reached the operand");
    // "di" in the text and "si" not: the registers named are bx and di.
    let shown = decoded(&moved.code, 0);
    assert_eq!(
        (shown.op0_register(), shown.memory_base(), shown.memory_index()),
        (iced_x86::Register::BX, iced_x86::Register::DI, iced_x86::Register::None)
    );
}

#[test]
fn test_a_held_operand_names_a_value_and_not_a_register() {
    let held: HeldMap = [(7, RegId::EBX)].into_iter().collect();
    let got = _operand(&Loc::Held(h(7, 2)), None, Some(&held));
    assert_eq!(got, rg(RegId::BX, 2), "resolved at the width asked for");
    let wide = _operand(&Loc::Held(h(7, 4)), None, Some(&held));
    assert_eq!(wide, rg(RegId::EBX, 4));
}

#[test]
fn test_an_unresolved_held_is_refused_rather_than_guessed() {
    let what = sem(Operation::Move, Some("mov"), vec![Loc::Held(h(9, 2))], vec![imm(1, 2)], None, false);
    assert!(emit(&what, 0, None, false, false, Some(&HeldMap::default())).is_none(), "no register for value 9");
    let held: HeldMap = [(9, RegId::EBX)].into_iter().collect();
    assert!(emit(&what, 0, None, false, false, Some(&held)).is_some(), "and it emits once there is");
}

fn funnel_of(
    count: Loc,
    name: &str,
) -> Semantics {
    let low = rg(RegId::EAX, 4);
    sem(Operation::Funnel, Some(name), vec![low.clone()], vec![low, rg(RegId::EDX, 4), count], None, false)
}

#[test]
fn test_a_funnel_shift_emits_the_form_its_count_asks_for() {
    for (count, want) in [(imm(16, 1), "660facd010"), (rg(RegId::CL, 1), "660fadd0")] {
        assert_eq!(hex(&made(emitted(&funnel_of(count, "shrd"))).code), want);
    }
}

#[test]
fn test_a_left_funnel_shift_emits_shld() {
    assert_eq!(hex(&made(emitted(&funnel_of(imm(16, 1), "shld"))).code), "660fa4d010");
}

fn restoring_of(
    wide: RegId,
    low: RegId,
    high: RegId,
) -> Semantics {
    ir::restoring(rg(wide, 4), rg(low, 2), rg(high, 2))
}

#[test]
fn test_a_restore_says_which_value_it_splits_and_into_which_halves() {
    let what = restoring_of(RegId::EAX, RegId::AX, RegId::DX);
    assert_eq!(what.op, Operation::Restore);
    assert_eq!(what.sources, [rg(RegId::EAX, 4)]);
    assert_eq!(what.dests, [rg(RegId::AX, 2), rg(RegId::DX, 2)]);
}

#[test]
fn test_a_restore_encodes_the_registers_the_allocation_chose() {
    for (wide, low, high, want) in [
        (RegId::EAX, RegId::AX, RegId::DX, "6650585a"),
        (RegId::ECX, RegId::CX, RegId::BX, "6651595b"),
        (RegId::ESI, RegId::SI, RegId::DI, "66565e5f"),
    ] {
        assert_eq!(hex(&made(emitted(&restoring_of(wide, low, high))).code), want);
    }
}

fn placed(
    r#where: Addr,
    through: RegId,
    offset: i64,
    disp_width: u32,
    value: u32,
) -> ir::Mem {
    ir::Mem { through, offset, disp_width, base: Some(h(value, 2)), ..ir::Mem::new(Some(r#where), 2) }
}

#[test]
fn test_a_relocated_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Segment, 0x2, RegId::SI, RegId::None);
    let cell = placed(r#where, RegId::BX, 2, 1, 17);
    let got = decoded(&made(move_from(RegId::AX, &cell, At::bits16(0))).code, 0);
    assert_eq!(got.memory_base(), (RegId::BX).iced());
    assert!(operand_of(&cell, 16).expect("encodable").1, "a segment address still needs its fixup moved");

    let plain = made(move_from(RegId::AX, &ir::Mem::new(Some(r#where), 2), At::bits16(0)));
    assert_eq!(decoded(&plain.code, 0).memory_base(), (RegId::SI).iced());
    assert!(operand_of(&ir::Mem::new(Some(r#where), 2), 16).expect("encodable").1);
}

#[test]
fn test_a_literal_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Literal, 0x2, RegId::SI, RegId::None);
    let got = decoded(&made(move_from(RegId::AX, &placed(r#where, RegId::BX, 2, 1, 17), At::bits16(0))).code, 0);
    assert_eq!(got.memory_base(), (RegId::BX).iced());
    assert_eq!(got.memory_displacement64(), 2);

    let back = decoded(&made(move_from(RegId::AX, &ir::Mem::new(Some(r#where), 2), At::bits16(0))).code, 0);
    assert_eq!(back.memory_base(), (RegId::SI).iced());
    assert_eq!(back.memory_displacement64(), 2);
}

#[test]
fn test_a_far_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Far, 0, RegId::BX, RegId::ES);
    let got = decoded(&made(move_from(RegId::AX, &placed(r#where, RegId::DI, 0, 0, 21), At::bits16(0))).code, 0);
    assert_eq!(got.memory_base(), (RegId::DI).iced());
    assert_eq!(got.memory_segment(), (RegId::ES).iced());

    let back = decoded(&made(move_from(RegId::AX, &ir::Mem::new(Some(r#where), 2), At::bits16(0))).code, 0);
    assert_eq!(back.memory_base(), (RegId::BX).iced());
    assert_eq!(back.memory_segment(), (RegId::ES).iced());
}

#[test]
fn test_a_frame_slots_displacement_is_not_a_relocatable_field() {
    let slot = ir::Mem { through: RegId::BP, offset: -0x22, disp_width: 2, ..ir::Mem::new(None, 2) };
    let store = sem(Operation::Move, Some("mov"), vec![Loc::Mem(slot)], vec![rg(RegId::BX, 2)], None, false);
    let store = made(emitted(&store));
    assert!(store.places().is_empty(), "the frame slot offered a field at {:?}", store.places());

    let array =
        ir::Mem { through: RegId::SI, ..ir::Mem::new(Some(at(Space::Segment, 0x6, RegId::SI, RegId::None)), 2) };
    let indexed = sem(Operation::Move, Some("mov"), vec![Loc::Mem(array)], vec![rg(RegId::BX, 2)], None, false);
    assert!(!made(emitted(&indexed)).places().is_empty(), "an array reached through si still carries its own address");
}

/// Lowering folds `base + index` into a global array's cell, and nothing
/// could encode it: "emits" failed on `[seg_x+v91+v117]`.
#[test]
fn test_a_global_cell_reached_through_base_and_index_keeps_its_relocation() {
    let cell = ir::Mem {
        through: RegId::BX,
        base: Some(ir::Held { value: 1, width: 2 }),
        index: Some(ir::Held { value: 2, width: 2 }),
        index_through: RegId::DI,
        scale: 1,
        ..ir::Mem::new(Some(at(Space::Segment, 0x6, RegId::None, RegId::None)), 2)
    };
    let load = sem(Operation::Move, Some("mov"), vec![rg(RegId::DX, 2)], vec![Loc::Mem(cell)], None, false);
    let load = made(emitted(&load));
    let decoded = decoded(&load.code, 0);
    assert_eq!((decoded.memory_base(), decoded.memory_index()), (iced_x86::Register::BX, iced_x86::Register::DI));
    assert!(!load.places().is_empty(), "the relocation lost its field");
}

// tests/test_arithmetic_immediates.py
#[test]
fn test_arithmetic_encodes_the_same_32_bit_pattern_in_either_signed_notation() {
    for number in [0xfffffffe_i64, 0x80000000] {
        for name in ["add", "sub"] {
            for memory in [false, true] {
                let cell = frame(-4, 4);
                let encode = |value| {
                    if memory {
                        arith_into_imm(name, &cell, value, At::bits16(0), false)
                    } else {
                        arith_imm(name, RegId::EDX, value, At::bits16(0), false)
                    }
                };
                let (positive, negative) = (made(encode(number)), made(encode(number - (1 << 32))));
                assert_eq!(positive.code, negative.code);
            }
        }
    }
}

// tests/test_multiply_select.py
#[test]
fn test_a_product_into_another_register_keeps_its_operand() {
    let memory = ir::Mem { through: RegId::BP, disp_width: 2, ..frame(-0x0E, 2) };
    for (source, operand) in [
        (Loc::Mem(memory), ("memory", RegId::BP, 0xFFF2_u64)),
        (rg(RegId::BX, 2), ("register", RegId::BX, 0)),
        (rg(RegId::CX, 2), ("register", RegId::CX, 0)),
    ] {
        let what = sem(Operation::Multiply, Some("imul"), vec![rg(RegId::CX, 2)], vec![source, imm(2, 2)], None, false);
        let code = made(emitted(&what)).code;
        let mut decoder = Decoder::new(BITNESS, &code, DecoderOptions::NONE);
        let insn = decoder.decode();
        assert!(!decoder.can_decode(), "one instruction");
        let read = if insn.op1_kind() == OpKind::Memory {
            ("memory", insn.memory_base(), insn.memory_displacement64())
        } else {
            ("register", insn.op1_register(), 0)
        };
        assert_eq!(
            (insn.op0_register(), read, insn.immediate(2)),
            (iced_x86::Register::CX, (operand.0, operand.1.iced(), operand.2), 2)
        );
    }
}

// tests/test_postallocation.py
#[test]
fn test_discarded_x87_result_has_a_register_pop_encoding() {
    let what = sem(Operation::FloatStore, Some("fstp"), vec![st(0)], vec![st(0)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "ddd8");
}

// tests/test_select_float_stack.py
#[test]
fn test_register_arithmetic_preserves_operand_direction() {
    for (name, base) in
        [("fadd", 0xc0_u8), ("fmul", 0xc8), ("fsub", 0xe0), ("fsubr", 0xe8), ("fdiv", 0xf0), ("fdivr", 0xf8)]
    {
        for index in [1_u8, 3, 7] {
            for top in [true, false] {
                let (dest, source) = if top { (st(0), st(u32::from(index))) } else { (st(u32::from(index)), st(0)) };
                let what = sem(Operation::FloatArith, Some(name), vec![dest.clone()], vec![dest, source], None, false);
                let result = made(emitted(&what));
                let byte = if top || name == "fadd" || name == "fmul" { base } else { base ^ 8 };
                assert_eq!(result.code, [if top { 0xd8 } else { 0xdc }, byte + index]);
                assert_eq!(format!("{:?}", decoded(&result.code, 0).mnemonic()).to_lowercase(), name);
            }
        }
    }
}

#[test]
fn test_stack_duplicate_and_exchange() {
    for index in [0_u8, 1, 7] {
        let load =
            emitted(&sem(Operation::FloatLoad, Some("fld"), vec![st(0)], vec![st(u32::from(index))], None, false));
        let both = vec![st(0), st(u32::from(index))];
        let exchange = emitted(&sem(Operation::Exchange, Some("fxch"), both.clone(), both, None, false));
        assert_eq!(made(load).code, [0xd9, 0xc0 + index]);
        assert_eq!(made(exchange).code, [0xd9, 0xc8 + index]);
    }
}

#[test]
fn test_unencodable_stack_arithmetic_is_refused() {
    // Python's (-1, 0) has no u32 St to say it with.
    for (dest, source) in [(1, 2), (0, 8)] {
        let what = sem(Operation::FloatArith, Some("fsub"), vec![st(dest)], vec![st(dest), st(source)], None, false);
        assert!(emitted(&what).is_none());
    }
}

// tests/test_stack_segment.py
#[test]
fn test_a_frame_derived_indirect_cell_keeps_its_stack_segment() {
    let cell = ir::Mem {
        through: RegId::BX,
        base: Some(h(1, 2)),
        ..ir::Mem::new(Some(at(Space::Literal, 0, RegId::None, RegId::SS)), 4)
    };
    let got = decoded(&made(move_into(&cell, RegId::EAX, At::bits16(0))).code, 0);
    assert_eq!(got.memory_base(), (RegId::BX).iced());
    assert_eq!(got.memory_segment(), (RegId::SS).iced());
}

// tests/test_test_immediate.py
#[test]
fn test_test_immediate_keeps_mask_and_flags() {
    let reference = seen(&[0xf7, 0x46, 0x06, 0x00, 0x80], 0).expect("decodes");
    for (width, value) in [(1_u32, 0x80_u64), (2, 0x8000), (4, 0x80000000), (2, 1)] {
        for memory in [false, true] {
            let operand = if memory {
                Loc::Mem(ir::Mem { through: RegId::BP, disp_width: 1, ..frame(6, width) })
            } else {
                rg([RegId::AL, RegId::AX, RegId::None, RegId::EAX][width as usize - 1], width)
            };
            let what =
                sem(Operation::Compare, Some("test"), vec![], vec![operand, imm(value as i64, width)], None, false);
            let made = made(emitted(&what));
            let decoded = seen(&made.code, 0).expect("decodes");
            assert_eq!(decoded.insn.mnemonic(), Mnemonic::Test);
            assert_eq!(decoded.insn.immediate(1) & ((1_u64 << (width * 8)) - 1), value);
            assert_eq!(decoded.insn.rflags_written(), reference.insn.rflags_written());
            assert_eq!(decoded.insn.rflags_cleared(), reference.insn.rflags_cleared());
            if memory && width == 2 && value == 0x8000 {
                assert_eq!(hex(&made.code), "f746060080");
            }
        }
    }
}

// tests/test_scaled_addressing.py: the formatter's text, as the bytes
// Python emitted for it.
#[test]
fn test_a_far_cell_is_encoded_with_its_scaled_index() {
    let cell = ir::Mem {
        through: RegId::ESI,
        base: Some(h(1, 4)),
        index: Some(h(2, 4)),
        scale: 2,
        index_through: RegId::ECX,
        ..ir::Mem::new(Some(at(Space::Far, 0, RegId::None, RegId::ES)), 2)
    };
    let what = sem(Operation::Move, Some("mov"), vec![rg(RegId::AX, 2)], vec![Loc::Mem(cell)], None, false);
    let emitted = made(emitted(&what));
    assert_eq!(emitted.code[..2], [0x26, 0x67]);
    assert_eq!(hex(&emitted.code), "26678b044e"); // mov ax,es:[esi+ecx*2]
}

#[test]
fn test_a_word_is_zero_extended_into_a_dword_register() {
    let what = sem(Operation::Extend, Some("movzx"), vec![rg(RegId::ECX, 4)], vec![rg(RegId::CX, 2)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "660fb7c9"); // movzx ecx,cx
}

#[test]
fn test_a_word_index_is_encoded_with_word_addressing() {
    let cell = ir::Mem {
        through: RegId::BX,
        base: Some(h(1, 2)),
        index: Some(h(2, 2)),
        index_through: RegId::SI,
        ..ir::Mem::new(Some(at(Space::Far, 0, RegId::None, RegId::FS)), 1)
    };
    let what = sem(Operation::Move, Some("mov"), vec![Loc::Mem(cell)], vec![rg(RegId::CL, 1)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "648808"); // mov fs:[bx+si],cl
}

// tests/test_addressforms.py
#[test]
fn test_selected_indexed_frame_cell_keeps_bp_and_its_dynamic_index() {
    let cell = ir::Mem { through: RegId::BP, base: Some(h(1, 2)), index_through: RegId::SI, ..frame(-96, 4) };
    let code = made(move_from(RegId::EAX, &cell, At::bits16(0))).code;
    let instruction = seen(&code, 0).expect("decodes");
    assert_eq!(instruction.insn.memory_base(), (RegId::BP).iced());
    assert_eq!(instruction.insn.memory_index(), (RegId::SI).iced());
    assert_eq!(instruction.insn.memory_displacement64() & 65535, (-96_i64 & 65535) as u64);
}

// tests/test_allocation.py: `_based_cell().what`, without the lir.Insn
// around it, which select never sees.
fn based_cell(through: RegId) -> Semantics {
    let r#where = at(Space::Segment, 0x10, RegId::SI, RegId::None);
    let cell = ir::Mem { through, disp_width: 2, base: Some(h(21, 2)), ..ir::Mem::new(Some(r#where), 2) };
    sem(Operation::Move, Some("mov"), vec![Loc::Held(h(30, 2))], vec![Loc::Mem(cell)], None, false)
}

#[test]
fn test_a_cell_whose_address_nothing_placed_is_refused() {
    assert!(emitted(&based_cell(RegId::None)).is_none());
}

#[test]
fn test_a_placed_cell_emits_the_register_it_was_given() {
    let cell = based_cell(RegId::SI).sources[0].clone();
    let what = sem(Operation::Move, Some("mov"), vec![rg(RegId::AX, 2)], vec![cell], None, false);
    let made = made(emitted(&what));
    // mov ax,[si+disp]
    assert!(hex(&made.code).starts_with("8b84"), "{}", hex(&made.code));
}

// tests/test_lir.py
#[test]
fn test_a_byte_wide_held_is_the_low_byte() {
    for (root, low) in
        [(RegId::EAX, RegId::AL), (RegId::EBX, RegId::BL), (RegId::ECX, RegId::CL), (RegId::EDX, RegId::DL)]
    {
        assert_eq!(AT_WIDTH[&root][&1], low, "{root:?} at one byte is not its low half");
    }
    assert_eq!(ir::root(RegId::AH), RegId::EAX, "the high byte still roots to eax");
}

// tests/test_layout.py
#[test]
fn test_an_operation_may_carry_a_fixup_for_each_instruction_it_stands_for() {
    let plain = Emitted { displacement_at: Some(2), ..Emitted::new(vec![0x8b, 0x06, 0x00, 0x00]) };
    assert!(plain.places() == [2] && plain.relocated_at() == Some(2));

    let idiom = Emitted { fields: vec![2, 6], ..Emitted::new(vec![0x66, 0xa1, 0x00, 0x00, 0x66, 0xb9, 0x00, 0x00]) };
    assert_eq!(idiom.places(), [2, 6]);
    assert_eq!(idiom.relocated_at(), Some(2), "the first is what a caller with one wants");

    let silent = Emitted::new(vec![0x99]);
    assert!(silent.places().is_empty() && silent.relocated_at().is_none());
}

/// A u16 count held in cx selected nothing: `sar eax,cx` had no encoding. The
/// shift reads only cl.
#[test]
fn test_a_shift_counts_from_cl_whatever_width_holds_the_count() {
    for (count, width) in [(RegId::CL, 1), (RegId::CX, 2), (RegId::ECX, 4)] {
        let eax = rg(RegId::EAX, 4);
        let what = sem(Operation::Binary, Some("sar"), vec![eax.clone()], vec![eax, rg(count, width)], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), "66d3f8", "{count:?}");
    }
}

#[test]
fn test_a_count_in_ch_is_not_one_in_cl() {
    let eax = rg(RegId::EAX, 4);
    let what = sem(Operation::Binary, Some("sar"), vec![eax.clone()], vec![eax, rg(RegId::CH, 1)], None, false);
    assert!(emitted(&what).is_none());
}

/// A near procedure that pops its arguments returns with `ret n`. The
/// selector spelled every counted return `retf n`, so a QuickrBASIC PRIVATE
/// SUB with a parameter returned far and the program hung.
#[test]
fn near_counted_return() {
    use super::sweep_support::*;
    check(
        sem(Op::Return, Some("ret"), vec![], vec![im(2, 2, None)], None, false),
        0,
        false,
        false,
        None,
        None,
        Some(("c20200", None, Some(1), vec![1], true)),
    );
    check(
        sem(Op::Return, Some("ret"), vec![], vec![im(0, 2, None)], None, false),
        0,
        false,
        false,
        None,
        None,
        Some(("c3", None, None, vec![], true)),
    );
}

/// sweep_07 failed on rustc 1.99.0: an immediate too wide for i32 raised std's
/// own text, which that release reworded. The error is ours, and says one
/// thing.
#[test]
fn test_an_immediate_too_wide_raises_the_same_text_on_every_compiler() {
    assert_eq!(i32_of(1 << 40), Err("out of range integral type conversion attempted".to_owned()));
}

fn decoded_over(segment: RegId) -> (RegId, usize) {
    let code = made(copy("movsw", segment, At::bits16(0), false)).code;
    (RegId::from(decoded(&code, 0).segment_prefix()), code.len())
}

/// A string move is `movs` with the width in its name, and `rep` where it
/// repeats; 16-bit code spells the dword form with the operand-size prefix.
#[test]
fn test_a_string_move_encodes_by_width_and_repeat() {
    let decoded = |name: &str, repeated: bool| {
        let code = made(copy(name, RegId::None, At::bits16(0), repeated)).code;
        let one = decoded(&code, 0);
        (one.mnemonic(), one.has_rep_prefix(), code.len())
    };
    assert_eq!(decoded("movsb", false), (Mnemonic::Movsb, false, 1));
    assert_eq!(decoded("movsw", false), (Mnemonic::Movsw, false, 1));
    assert_eq!(decoded("movsd", false), (Mnemonic::Movsd, false, 2));
    assert_eq!(decoded("movsw", true), (Mnemonic::Movsw, true, 2));
    assert_eq!(decoded("movsd", true), (Mnemonic::Movsd, true, 3));
    assert!(copy("movsq", RegId::None, At::bits16(0), false).is_none());
    // The source read through another segment is an override on the move.
    let over = decoded_over(RegId::SS);
    assert_eq!(over, (RegId::SS, 2));
    assert_eq!(decoded_over(RegId::FS), (RegId::FS, 2));
}

/// An extension of a register into its own wider register has a shorter form:
/// `movzx ax,al` (0f b6 c0, 3 bytes) is `mov ah,0` (b4 00), `movsx ax,al` is
/// `cbw` (98) and `movsx eax,ax` `cwde` (66 98, not 66 0f bf c0). QCport -Os
/// had some 200; neither writes a flag.
#[test]
fn test_an_extension_in_place_takes_its_shortest_form() {
    for (name, into, from, expected) in [
        ("movzx", rg(RegId::AX, 2), rg(RegId::AL, 1), "b400"),
        ("movzx", rg(RegId::DX, 2), rg(RegId::DL, 1), "b600"),
        ("movsx", rg(RegId::AX, 2), rg(RegId::AL, 1), "98"),
        ("movsx", rg(RegId::EAX, 4), rg(RegId::AX, 2), "6698"),
        // Not in place: a different register, or one with no high byte.
        ("movzx", rg(RegId::AX, 2), rg(RegId::BL, 1), "0fb6c3"),
        ("movzx", rg(RegId::SI, 2), rg(RegId::AL, 1), "0fb6f0"),
        ("movsx", rg(RegId::EBX, 4), rg(RegId::BX, 2), "660fbfdb"),
        ("movzx", rg(RegId::EAX, 4), rg(RegId::AX, 2), "660fb7c0"),
    ] {
        let what = sem(Operation::Extend, Some(name), vec![into], vec![from], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), expected, "{name}");
    }
}

/// `enter N,0` is c8 N N 00: the frame `push bp; mov bp,sp; sub sp,N` in 4
/// bytes of 6.
#[test]
fn test_enter_encodes_its_size_and_nesting_level() {
    let what = sem(Operation::Nothing, Some("enter"), vec![], vec![imm(300, 2), imm(0, 1)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "c82c0100");
}
