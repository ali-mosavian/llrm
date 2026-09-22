//! Ports of tests/test_select.py and the select-only tests elsewhere.
//! Python's `str(insn)` checks read iced's decoded fields instead, or the
//! bytes Python printed: this crate builds iced without a formatter.

use super::sweep_support::{a, h, rg, sem};
use super::*;
use crate::model::ir::Addr;
use iced_x86::{Mnemonic, OpKind};

fn decoded(code: &[u8], ip: u64) -> Instruction {
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

fn frame(disp: i64, width: u32) -> ir::Mem {
    ir::Mem::new(Some(Addr::new(Space::Frame, disp)), width)
}

fn at(space: Space, disp: i64, base: Register, segment: Register) -> Addr {
    a(space, disp, 0, base, segment)
}

fn imm(value: i64, width: u32) -> Loc {
    Loc::Imm(ir::Imm { value, width, address: None })
}

fn st(index: u32) -> Loc {
    Loc::St(ir::St { index })
}

const ROOTS: [Register; 6] = [Register::EAX, Register::ECX, Register::EDX, Register::EBX, Register::ESI, Register::EDI];

#[test]
fn test_pl_move_exchange_with_frame_memory() {
    for (register, width) in [(Register::AL, 1), (Register::AX, 2), (Register::EAX, 4)] {
        for memory_first in [false, true] {
            let cell = Loc::Mem(frame(-28, width));
            let held = rg(register, width);
            let dests = if memory_first { vec![cell, held] } else { vec![held, cell] };
            let sources = dests.iter().rev().cloned().collect();
            let emitted = made(emitted(&sem(Operation::Exchange, Some("xchg"), dests, sources, None, false)));
            let instruction = decoded(&emitted.code, 0);
            assert_eq!(Some(instruction.code()), _code(&format!("XCHG_RM{}_R{}", width * 8, width * 8)));
            assert_eq!(instruction.memory_base(), Register::BP);
            assert_eq!(instruction.memory_displacement64() & 0xFFFF, 0xFFE4);
            assert_eq!(instruction.op1_register(), register);
        }
    }
}

#[test]
fn test_byte_copy_for_nbody_timer() {
    for source in [Register::CL, Register::AH, Register::BL, Register::DH] {
        let what = sem(Operation::Move, Some("mov"), vec![rg(Register::AL, 1)], vec![rg(source, 1)], None, false);
        let instruction = decoded(&made(emitted(&what)).code, 0);
        assert_eq!(instruction.code(), Code::Mov_r8_rm8);
        assert_eq!(instruction.op0_register(), Register::AL);
        assert_eq!(instruction.op1_register(), source);
    }
}

#[test]
fn test_signed_word_extension_uses_explicit_operands() {
    let what = sem(Operation::Extend, Some("movsx"), vec![rg(Register::EBX, 4)], vec![rg(Register::SI, 2)], None, false);
    let instruction = decoded(&made(emitted(&what)).code, 0);
    assert_eq!(instruction.code(), Code::Movsx_r32_rm16);
    assert!(instruction.op0_register() == Register::EBX && instruction.op1_register() == Register::SI);
    assert!(target::requirements(&what).is_empty());
}

#[test]
fn test_load_accepts_unsigned_dword_bit_pattern() {
    assert_eq!(hex(&made(load(Register::ESI, 0xBFFFFFF9, 0)).code), "66bef9ffffbf");
}

#[test]
fn test_push_accepts_unsigned_dword_bit_patterns() {
    for value in [0x80000000_i64, 0xEDCBA987, 0xFFFFFFFF] {
        let instruction = decoded(&made(push_imm(value, 4, 0, false)).code, 0);
        assert_eq!(instruction.stack_pointer_increment(), -4);
        assert_eq!(instruction.immediate(0) & 0xFFFFFFFF, value as u64);
    }
}

#[test]
fn test_store_accepts_unsigned_dword_bit_pattern() {
    assert_eq!(hex(&made(store_imm(&frame(-4, 4), 0xC1747C23, 0)).code), "66c746fc237c74c1");
}

#[test]
fn test_a_move_decodes_back_to_the_move_that_was_asked_for() {
    for into in ROOTS {
        for outof in ROOTS {
            if into == outof {
                continue;
            }
            let back = decoded(&made(r#move(into, outof, 0)).code, 0);
            assert_eq!(back.code(), Code::Mov_r32_rm32);
            assert_eq!(back.op0_register(), into);
            assert_eq!(back.op1_register(), outof);
        }
    }
}

#[test]
fn test_a_sixteen_bit_move_is_shorter_than_a_thirty_two_bit_one() {
    let wide = made(r#move(Register::ECX, Register::EAX, 0));
    let narrow = made(r#move(Register::CX, Register::AX, 0));
    assert!(wide.code.len() == 3 && narrow.code.len() == 2);
    assert_eq!(wide.code[0], 0x66);
}

#[test]
fn test_a_move_to_itself_is_no_instruction() {
    assert!(made(r#move(Register::EAX, Register::EAX, 0)).code.is_empty());
}

#[test]
fn test_mixed_widths_are_refused_rather_than_guessed() {
    assert!(r#move(Register::ECX, Register::AX, 0).is_none());
    assert!(r#move(Register::AX, Register::ECX, 0).is_none());
}

#[test]
fn test_registers_this_does_not_name_are_refused() {
    assert!(r#move(Register::EAX, Register::ES, 0).is_none());
    assert!(r#move(Register::ES, Register::EAX, 0).is_none());
    assert!(r#move(Register::EBP, Register::ESP, 0).is_some());
}

#[test]
fn test_a_wide_push_is_not_a_narrow_one() {
    let (narrow, wide) = (made(push_imm(3, 2, 0, false)), made(push_imm(3, 4, 0, false)));
    assert_eq!(decoded(&narrow.code, 0).stack_pointer_increment(), -2);
    assert_eq!(decoded(&wide.code, 0).stack_pointer_increment(), -4);
}

#[test]
fn test_a_register_is_not_an_immediate() {
    let one = made(push(Register::EAX, 0));
    assert_eq!(one.code, [0x66, 0x50]);
    let other = made(push_imm(Register::EAX as i64, 2, 0, false));
    assert_ne!(other.code, one.code);
}

#[test]
fn test_a_mixed_width_operation_is_refused() {
    assert!(arith("add", Register::AX, Register::ECX, 0).is_none());
    assert!(arith("add", Register::EAX, Register::CX, 0).is_none());
}

#[test]
fn test_an_operation_not_in_the_table_is_refused() {
    for name in ["rol", "shl", "imul", "xchg", ""] {
        assert!(arith(name, Register::AX, Register::CX, 0).is_none());
    }
    for name in ["bswap", "shr", ""] {
        assert!(unary(name, Register::AX, 0).is_none());
    }
}

#[test]
fn test_a_relocated_address_is_emitted_as_zero_and_says_where() {
    let cell = ir::Mem::new(Some(a(Space::Segment, 0x1234, 5, Register::None, Register::None)), 2);
    let made = made(move_from(Register::AX, &cell, 0));
    let at = made.displacement_at.expect("a displacement");
    assert_eq!(made.code[at..at + 2], [0, 0], "a relocated displacement is not a number");
}

#[test]
fn test_a_frame_slot_keeps_its_displacement() {
    let made = made(move_from(Register::AX, &frame(-0x18, 2), 0));
    assert_eq!(hex(&made.code), "8b46e8"); // mov ax,[bp-18h]
    let at = made.displacement_at.expect("a displacement");
    assert_eq!(made.code[at..], [(-0x18_i64 & 0xFF) as u8]);
}

#[test]
fn test_the_spaces_that_cannot_be_encoded_are_refused() {
    for space in [Space::Far, Space::Group, Space::Stack] {
        assert!(operand_of(&ir::Mem::new(Some(Addr::new(space, 4)), 2)).is_none(), "{space:?}");
    }
    assert!(operand_of(&ir::Mem::new(None, 2)).is_none());
    let literal = operand_of(&ir::Mem::new(Some(Addr::new(Space::Literal, 4)), 2));
    assert!(literal.is_some_and(|(_, relocated)| !relocated), "a literal address relocates nothing");
}

#[test]
fn test_a_cell_of_the_wrong_width_is_refused() {
    let cell = frame(-4, 4);
    assert!(move_from(Register::AX, &cell, 0).is_none());
    assert!(move_from(Register::EAX, &cell, 0).is_some());
}

#[test]
fn test_a_near_call_and_a_far_call_are_told_apart_by_the_target() {
    let near = made(call_near(0x32, 0x4E));
    let far = made(call_far(0x4E));
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
        assert_eq!(hex(&made(store_imm(&frame(-4, width), 0, 0)).code), want);
    }
}

#[test]
fn test_a_narrow_push_of_a_small_literal_takes_the_byte_form() {
    for (value, want) in [(0, "6a00"), (1, "6a01"), (0x31, "6a31"), (-1, "6aff"), (127, "6a7f"), (-128, "6a80")] {
        assert_eq!(hex(&made(push_imm(value, 2, 0, false)).code), want);
    }
}

#[test]
fn test_a_narrow_push_that_does_not_fit_a_byte_stays_wide() {
    for value in [128, -129, 1000, -1000, 0x7FFF, -0x8000] {
        let made = made(push_imm(value, 2, 0, false));
        assert_eq!(made.code[0], 0x68, "{value} took the byte form and does not fit one");
        assert_eq!(made.code.len(), 3);
    }
}

#[test]
fn test_a_word_immediate_written_unsigned_still_fits_a_byte() {
    assert_eq!(hex(&made(push_imm(0xFFFF, 2, 0, false)).code), "6aff");
    assert_eq!(hex(&made(push_imm(0xFF80, 2, 0, false)).code), "6a80");
    assert_eq!(hex(&made(arith_imm("add", Register::BX, 0xFFFF, 0, false)).code), "83c3ff");
}

#[test]
fn test_a_literal_address_through_a_register_uses_the_byte_displacement() {
    let cell = ir::Mem {
        through: Register::SI,
        offset: 0x0A,
        ..ir::Mem::new(Some(at(Space::Literal, 0x0A, Register::SI, Register::None)), 2)
    };
    assert_eq!(hex(&made(arith_mem("add", Register::BX, &cell, 0)).code), "035c0a");
}

#[test]
fn test_a_bare_literal_address_keeps_two_bytes_however_small_it_is() {
    for value in [0, 1, 0x10, 0x7F] {
        let cell = ir::Mem::new(Some(Addr::new(Space::Literal, value)), 2);
        let made = made(arith_mem("add", Register::BX, &cell, 0));
        assert_eq!(made.code.len(), 4, "{value:#x} came back {}", hex(&made.code));
        assert_eq!(made.code[1] & 0xC7, 0x06, "{value:#x} is not the direct-address form");
    }
}

#[test]
fn test_a_relocated_push_keeps_the_wide_immediate() {
    assert_eq!(hex(&made(push_imm(0, 2, 0, true)).code), "680000");
    assert_eq!(hex(&made(push_imm(0, 2, 0, false)).code), "6a00");
    assert_eq!(hex(&made(push_imm(3, 4, 0, true)).code), "666803000000");
    assert_eq!(hex(&made(push_imm(3, 4, 0, false)).code), "666a03");
}

#[test]
fn test_a_shift_by_one_takes_its_own_opcode() {
    for (name, reg, want) in [("shl", Register::AX, "d1e0"), ("shl", Register::BX, "d1e3"), ("sar", Register::AX, "d1f8")] {
        assert_eq!(hex(&made(shift(name, RegisterOrCell::Reg(reg), Some(1), 0)).code), want);
    }
}

#[test]
fn test_a_shift_by_more_than_one_keeps_the_immediate_form() {
    assert_eq!(hex(&made(shift("shl", RegisterOrCell::Reg(Register::AX), Some(3), 0)).code), "c1e003");
}

#[test]
fn test_spilled_shift_is_encodable() {
    for (count, hex_bytes) in [(Some(1), "d166de"), (Some(3), "c166de03"), (None, "d366de")] {
        let cell = Loc::Mem(frame(-0x22, 2));
        let source = match count {
            None => rg(Register::CL, 1),
            Some(count) => imm(count, 1),
        };
        let what = sem(Operation::Binary, Some("shl"), vec![cell.clone()], vec![cell, source], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), hex_bytes);
    }
}

#[test]
fn test_an_accumulator_immediate_takes_the_short_opcode() {
    for (name, value, want) in [("add", 0x1286, "058612"), ("cmp", 0x1234, "3d3412"), ("sub", 0x4000, "2d0040")] {
        assert_eq!(hex(&made(arith_imm(name, Register::AX, value, 0, false)).code), want);
    }
}

#[test]
fn test_a_byte_immediate_still_beats_the_accumulator_form() {
    assert_eq!(hex(&made(arith_imm("add", Register::AX, 3, 0, false)).code), "83c003");
    assert_eq!(hex(&made(arith_imm("add", Register::BX, 3, 0, false)).code), "83c303");
    assert_eq!(hex(&made(arith_imm("add", Register::BX, 0x1286, 0, false)).code), "81c38612");
}

#[test]
fn test_a_compare_of_memory_against_a_small_literal_takes_the_byte_form() {
    for (value, want) in [(0x32, "837ee232"), (0, "837ee200"), (-1, "837ee2ff")] {
        assert_eq!(hex(&made(compare(&Loc::Mem(frame(-0x1E, 2)), value, 0, false)).code), want);
    }
}

#[test]
fn test_a_compare_of_a_byte_cell_against_a_literal() {
    for (value, want) in [(0, "807efc00"), (200, "807efcc8"), (-1, "807efcff")] {
        assert_eq!(hex(&made(compare(&Loc::Mem(frame(-4, 1)), value, 0, false)).code), want);
    }
}

#[test]
fn test_an_extension_encodes_from_a_byte_or_a_cell() {
    let cell = |width| Loc::Mem(ir::Mem { through: Register::BP, disp_width: 2, ..frame(-4, width) });
    for (name, dest, operand, want) in [
        ("movzx", Register::BX, cell(1), "0fb65efc"),
        ("movsx", Register::AX, cell(1), "0fbe46fc"),
        ("movzx", Register::EBX, cell(2), "660fb75efc"),
        ("movzx", Register::EBX, rg(Register::BX, 2), "660fb7db"),
    ] {
        let width = WIDTHS[&dest] as u32;
        let what = sem(Operation::Extend, Some(name), vec![rg(dest, width)], vec![operand], None, false);
        assert_eq!(hex(&made(emitted(&what)).code), want);
    }
}

#[test]
fn test_a_compare_of_memory_against_a_large_literal_stays_wide() {
    assert_eq!(hex(&made(compare(&Loc::Mem(frame(-0x1E, 2)), 0x1234, 0, false)).code), "817ee23412");
}

#[test]
fn test_a_relocated_arithmetic_immediate_keeps_its_width() {
    for name in ["add", "sub", "cmp", "and", "or", "xor"] {
        let wide = made(arith_imm(name, Register::AX, 0, 0, true));
        assert_eq!(wide.code.len(), 3, "{name} ax,0 relocated came back {}", hex(&wide.code));
        assert!(wide.immediate_at.is_some());
        assert_ne!(wide.code[0], 0x83);
        let narrow = made(arith_imm(name, Register::AX, 0, 0, false));
        assert_eq!(narrow.code[0], 0x83);
    }
}

#[test]
fn test_a_relocated_immediate_against_memory_keeps_its_width() {
    let cell = frame(-4, 2);
    let wide = made(arith_into_imm("add", &cell, 0, 0, true));
    assert_eq!(wide.code[0], 0x81, "came back {}", hex(&wide.code));
    let narrow = made(arith_into_imm("add", &cell, 0, 0, false));
    assert_eq!(narrow.code[0], 0x83);
}

#[test]
fn test_a_comparison_against_zero_takes_the_test_form() {
    for (reg, want) in [(Register::EAX, "6685c0"), (Register::ECX, "6685c9"), (Register::AX, "85c0"), (Register::BX, "85db")] {
        let width = if matches!(reg, Register::EAX | Register::ECX) { 4 } else { 2 };
        assert_eq!(hex(&made(compare(&rg(reg, width), 0, 0, false)).code), want);
    }
}

#[test]
fn test_a_comparison_against_zero_stays_a_compare_when_relocated() {
    let made = made(compare(&rg(Register::AX, 2), 0, 0, true));
    assert_ne!(made.code[0], 0x85, "a relocated compare became a test: {}", hex(&made.code));
}

#[test]
fn test_a_remap_reaches_inside_a_memory_operand() {
    let r#where: RegisterMap = [(Register::SI, Register::DI), (Register::ESI, Register::EDI)].into_iter().collect();
    let cell = ir::Mem {
        through: Register::SI,
        offset: 0x0A,
        disp_width: 1,
        ..ir::Mem::new(Some(at(Space::Literal, 0x0A, Register::SI, Register::None)), 2)
    };
    let what = sem(Operation::Binary, Some("add"), vec![rg(Register::BX, 2)], vec![rg(Register::BX, 2), Loc::Mem(cell)], None, false);
    let plain = made(emitted(&what));
    let moved = made(emit(&what, 0, Some(Where::One(&r#where)), false, false, None));
    assert_ne!(plain.code, moved.code, "the remap never reached the operand");
    // "di" in the text and "si" not: the registers named are bx and di.
    let shown = decoded(&moved.code, 0);
    assert_eq!(
        (shown.op0_register(), shown.memory_base(), shown.memory_index()),
        (Register::BX, Register::DI, Register::None)
    );
}

#[test]
fn test_a_held_operand_names_a_value_and_not_a_register() {
    let held: HeldMap = [(7, Register::EBX)].into_iter().collect();
    let got = _operand(&Loc::Held(h(7, 2)), None, Some(&held));
    assert_eq!(got, rg(Register::BX, 2), "resolved at the width asked for");
    let wide = _operand(&Loc::Held(h(7, 4)), None, Some(&held));
    assert_eq!(wide, rg(Register::EBX, 4));
}

#[test]
fn test_an_unresolved_held_is_refused_rather_than_guessed() {
    let what = sem(Operation::Move, Some("mov"), vec![Loc::Held(h(9, 2))], vec![imm(1, 2)], None, false);
    assert!(emit(&what, 0, None, false, false, Some(&HeldMap::new())).is_none(), "no register for value 9");
    let held: HeldMap = [(9, Register::EBX)].into_iter().collect();
    assert!(emit(&what, 0, None, false, false, Some(&held)).is_some(), "and it emits once there is");
}

fn funnel_of(count: Loc, name: &str) -> Semantics {
    let low = rg(Register::EAX, 4);
    sem(Operation::Funnel, Some(name), vec![low.clone()], vec![low, rg(Register::EDX, 4), count], None, false)
}

#[test]
fn test_a_funnel_shift_is_two_address_in_its_low_half() {
    assert_eq!(target::tied(&funnel_of(imm(16, 1), "shrd")), Some(Register::EAX));
}

#[test]
fn test_a_funnel_shift_by_a_register_takes_its_count_in_cl() {
    let dynamic = target::reads(&funnel_of(rg(Register::CL, 1), "shrd"));
    assert!(dynamic.get(&Register::ECX).is_some_and(|need| need.fixed() == Some(Register::ECX)));
    assert!(!target::reads(&funnel_of(imm(16, 1), "shrd")).contains_key(&Register::EAX));
    assert!(target::writes(&funnel_of(imm(16, 1), "shrd")).is_empty(), "shrd writes only what it names");
}

#[test]
fn test_a_funnel_shift_emits_the_form_its_count_asks_for() {
    for (count, want) in [(imm(16, 1), "660facd010"), (rg(Register::CL, 1), "660fadd0")] {
        assert_eq!(hex(&made(emitted(&funnel_of(count, "shrd"))).code), want);
    }
}

#[test]
fn test_a_left_funnel_shift_emits_shld() {
    assert_eq!(hex(&made(emitted(&funnel_of(imm(16, 1), "shld"))).code), "660fa4d010");
}

fn restoring_of(wide: Register, low: Register, high: Register) -> Semantics {
    ir::restoring(rg(wide, 4), rg(low, 2), rg(high, 2))
}

#[test]
fn test_a_restore_says_which_value_it_splits_and_into_which_halves() {
    let what = restoring_of(Register::EAX, Register::AX, Register::DX);
    assert_eq!(what.op, Operation::Restore);
    assert_eq!(what.sources, [rg(Register::EAX, 4)]);
    assert_eq!(what.dests, [rg(Register::AX, 2), rg(Register::DX, 2)]);
}

#[test]
fn test_a_restore_encodes_the_registers_the_allocation_chose() {
    for (wide, low, high, want) in [
        (Register::EAX, Register::AX, Register::DX, "6650585a"),
        (Register::ECX, Register::CX, Register::BX, "6651595b"),
        (Register::ESI, Register::SI, Register::DI, "66565e5f"),
    ] {
        assert_eq!(hex(&made(emitted(&restoring_of(wide, low, high))).code), want);
    }
}

fn placed(r#where: Addr, through: Register, offset: i64, disp_width: u32, value: u32) -> ir::Mem {
    ir::Mem { through, offset, disp_width, base: Some(h(value, 2)), ..ir::Mem::new(Some(r#where), 2) }
}

#[test]
fn test_a_relocated_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Segment, 0x2, Register::SI, Register::None);
    let cell = placed(r#where, Register::BX, 2, 1, 17);
    let got = decoded(&made(move_from(Register::AX, &cell, 0)).code, 0);
    assert_eq!(got.memory_base(), Register::BX);
    assert!(operand_of(&cell).expect("encodable").1, "a segment address still needs its fixup moved");

    let plain = made(move_from(Register::AX, &ir::Mem::new(Some(r#where), 2), 0));
    assert_eq!(decoded(&plain.code, 0).memory_base(), Register::SI);
    assert!(operand_of(&ir::Mem::new(Some(r#where), 2)).expect("encodable").1);
}

#[test]
fn test_a_literal_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Literal, 0x2, Register::SI, Register::None);
    let got = decoded(&made(move_from(Register::AX, &placed(r#where, Register::BX, 2, 1, 17), 0)).code, 0);
    assert_eq!(got.memory_base(), Register::BX);
    assert_eq!(got.memory_displacement64(), 2);

    let back = decoded(&made(move_from(Register::AX, &ir::Mem::new(Some(r#where), 2), 0)).code, 0);
    assert_eq!(back.memory_base(), Register::SI);
    assert_eq!(back.memory_displacement64(), 2);
}

#[test]
fn test_a_far_cell_is_reached_through_the_register_it_was_placed_in() {
    let r#where = at(Space::Far, 0, Register::BX, Register::ES);
    let got = decoded(&made(move_from(Register::AX, &placed(r#where, Register::DI, 0, 0, 21), 0)).code, 0);
    assert_eq!(got.memory_base(), Register::DI);
    assert_eq!(got.memory_segment(), Register::ES);

    let back = decoded(&made(move_from(Register::AX, &ir::Mem::new(Some(r#where), 2), 0)).code, 0);
    assert_eq!(back.memory_base(), Register::BX);
    assert_eq!(back.memory_segment(), Register::ES);
}

#[test]
fn test_a_frame_slots_displacement_is_not_a_relocatable_field() {
    let slot = ir::Mem { through: Register::BP, offset: -0x22, disp_width: 2, ..ir::Mem::new(None, 2) };
    let store = sem(Operation::Move, Some("mov"), vec![Loc::Mem(slot)], vec![rg(Register::BX, 2)], None, false);
    let store = made(emitted(&store));
    assert!(store.places().is_empty(), "the frame slot offered a field at {:?}", store.places());

    let array = ir::Mem { through: Register::SI, ..ir::Mem::new(Some(at(Space::Segment, 0x6, Register::SI, Register::None)), 2) };
    let indexed = sem(Operation::Move, Some("mov"), vec![Loc::Mem(array)], vec![rg(Register::BX, 2)], None, false);
    assert!(!made(emitted(&indexed)).places().is_empty(), "an array reached through si still carries its own address");
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
                        arith_into_imm(name, &cell, value, 0, false)
                    } else {
                        arith_imm(name, Register::EDX, value, 0, false)
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
    let memory = ir::Mem { through: Register::BP, disp_width: 2, ..frame(-0x0E, 2) };
    for (source, operand) in [
        (Loc::Mem(memory), ("memory", Register::BP, 0xFFF2_u64)),
        (rg(Register::BX, 2), ("register", Register::BX, 0)),
        (rg(Register::CX, 2), ("register", Register::CX, 0)),
    ] {
        let what = sem(Operation::Multiply, Some("imul"), vec![rg(Register::CX, 2)], vec![source, imm(2, 2)], None, false);
        let code = made(emitted(&what)).code;
        let mut decoder = Decoder::new(BITNESS, &code, DecoderOptions::NONE);
        let insn = decoder.decode();
        assert!(!decoder.can_decode(), "one instruction");
        let read = if insn.op1_kind() == OpKind::Memory {
            ("memory", insn.memory_base(), insn.memory_displacement64())
        } else {
            ("register", insn.op1_register(), 0)
        };
        assert_eq!((insn.op0_register(), read, insn.immediate(2)), (Register::CX, operand, 2));
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
    for (name, base) in [("fadd", 0xc0_u8), ("fmul", 0xc8), ("fsub", 0xe0), ("fsubr", 0xe8), ("fdiv", 0xf0), ("fdivr", 0xf8)] {
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
        let load = emitted(&sem(Operation::FloatLoad, Some("fld"), vec![st(0)], vec![st(u32::from(index))], None, false));
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
        through: Register::BX,
        base: Some(h(1, 2)),
        ..ir::Mem::new(Some(at(Space::Literal, 0, Register::None, Register::SS)), 4)
    };
    let got = decoded(&made(move_into(&cell, Register::EAX, 0)).code, 0);
    assert_eq!(got.memory_base(), Register::BX);
    assert_eq!(got.memory_segment(), Register::SS);
}

// tests/test_test_immediate.py
#[test]
fn test_test_immediate_keeps_mask_and_flags() {
    let reference = crate::frontend::declen::decode(&[0xf7, 0x46, 0x06, 0x00, 0x80], 0).expect("decodes");
    for (width, value) in [(1_u32, 0x80_u64), (2, 0x8000), (4, 0x80000000), (2, 1)] {
        for memory in [false, true] {
            let operand = if memory {
                Loc::Mem(ir::Mem { through: Register::BP, disp_width: 1, ..frame(6, width) })
            } else {
                rg([Register::AL, Register::AX, Register::None, Register::EAX][width as usize - 1], width)
            };
            let what = sem(Operation::Compare, Some("test"), vec![], vec![operand, imm(value as i64, width)], None, false);
            let made = made(emitted(&what));
            let decoded = crate::frontend::declen::decode(&made.code, 0).expect("decodes");
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
        through: Register::ESI,
        base: Some(h(1, 4)),
        index: Some(h(2, 4)),
        scale: 2,
        index_through: Register::ECX,
        ..ir::Mem::new(Some(at(Space::Far, 0, Register::None, Register::ES)), 2)
    };
    let what = sem(Operation::Move, Some("mov"), vec![rg(Register::AX, 2)], vec![Loc::Mem(cell)], None, false);
    let emitted = made(emitted(&what));
    assert_eq!(emitted.code[..2], [0x26, 0x67]);
    assert_eq!(hex(&emitted.code), "26678b044e"); // mov ax,es:[esi+ecx*2]
}

#[test]
fn test_a_word_is_zero_extended_into_a_dword_register() {
    let what = sem(Operation::Extend, Some("movzx"), vec![rg(Register::ECX, 4)], vec![rg(Register::CX, 2)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "660fb7c9"); // movzx ecx,cx
}

#[test]
fn test_a_word_index_is_encoded_with_word_addressing() {
    let cell = ir::Mem {
        through: Register::BX,
        base: Some(h(1, 2)),
        index: Some(h(2, 2)),
        index_through: Register::SI,
        ..ir::Mem::new(Some(at(Space::Far, 0, Register::None, Register::FS)), 1)
    };
    let what = sem(Operation::Move, Some("mov"), vec![Loc::Mem(cell)], vec![rg(Register::CL, 1)], None, false);
    assert_eq!(hex(&made(emitted(&what)).code), "648808"); // mov fs:[bx+si],cl
}

// tests/test_addressforms.py
#[test]
fn test_selected_indexed_frame_cell_keeps_bp_and_its_dynamic_index() {
    let cell = ir::Mem { through: Register::BP, base: Some(h(1, 2)), index_through: Register::SI, ..frame(-96, 4) };
    let code = made(move_from(Register::EAX, &cell, 0)).code;
    let instruction = crate::frontend::declen::decode(&code, 0).expect("decodes");
    assert_eq!(instruction.insn.memory_base(), Register::BP);
    assert_eq!(instruction.insn.memory_index(), Register::SI);
    assert_eq!(instruction.insn.memory_displacement64() & 65535, (-96_i64 & 65535) as u64);
}

// tests/test_allocation.py: `_based_cell().what`, without the lir.Insn
// around it, which select never sees.
fn based_cell(through: Register) -> Semantics {
    let r#where = at(Space::Segment, 0x10, Register::SI, Register::None);
    let cell = ir::Mem { through, disp_width: 2, base: Some(h(21, 2)), ..ir::Mem::new(Some(r#where), 2) };
    sem(Operation::Move, Some("mov"), vec![Loc::Held(h(30, 2))], vec![Loc::Mem(cell)], None, false)
}

#[test]
fn test_a_cell_whose_address_nothing_placed_is_refused() {
    assert!(emitted(&based_cell(Register::None)).is_none());
}

#[test]
fn test_a_placed_cell_emits_the_register_it_was_given() {
    let cell = based_cell(Register::SI).sources[0].clone();
    let what = sem(Operation::Move, Some("mov"), vec![rg(Register::AX, 2)], vec![cell], None, false);
    let made = made(emitted(&what));
    assert!(hex(&made.code).starts_with("8b84"), "{}", hex(&made.code)); // mov ax,[si+disp]
}

// tests/test_lir.py
#[test]
fn test_a_byte_wide_held_is_the_low_byte() {
    for (root, low) in [
        (Register::EAX, Register::AL),
        (Register::EBX, Register::BL),
        (Register::ECX, Register::CL),
        (Register::EDX, Register::DL),
    ] {
        assert_eq!(AT_WIDTH[&root][&1], low, "{root:?} at one byte is not its low half");
    }
    assert_eq!(ir::root(Register::AH), Register::EAX, "the high byte still roots to eax");
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

/// `divides` has no Python test of its own; these are Python's answers.
#[test]
fn divides_matches_python() {
    use crate::model::mir::{self, Arg, Kind, MemRef, OpCode};

    let cell = |disp, base| Arg::Cell(mir::Cell { r#ref: MemRef::new(Some(a(Space::Segment, disp, 1, base, Register::None)), 4) });
    let constant = |n: i64| Arg::Const(mir::Const::new(n, 4));
    let args = |key| match key {
        "cc" => vec![constant(100), constant(7)],
        "mm" => vec![cell(0x10, Register::None), cell(0x20, Register::None)],
        "mb" => vec![cell(0x10, Register::SI), cell(0x20, Register::BX)],
        "cm" => vec![constant(-5), cell(0x20, Register::None)],
        _ => vec![Arg::Held(mir::Held { value: mir::Value::new(1, 1), width: 4 }), constant(7)],
    };
    let (eax, ecx, edx, ebx) = (Register::EAX, Register::ECX, Register::EDX, Register::EBX);
    let held = "Held(value=v1, width=4) is not an operand a divide can read yet";
    let rows: [(&str, (Register, Register), bool, Result<(&str, Vec<usize>), &str>); 16] = [
        ("cc", (eax, edx), true, Ok(("66b86400000066b907000000669966f7f96650585a", vec![]))),
        ("cc", (edx, eax), false, Ok(("66b86400000066b907000000669966f7f96687c2", vec![]))),
        ("cc", (ebx, eax), true, Ok(("66b86400000066b907000000669966f7f9668bd8668bc26650585a", vec![]))),
        ("cc", (eax, ebx), false, Ok(("66b86400000066b907000000669966f7f9668bda", vec![]))),
        ("cc", (ecx, edx), true, Ok(("66b86400000066b907000000669966f7f9668bc86650585a", vec![]))),
        ("mm", (eax, edx), true, Ok(("66a10000668b0e0000669966f7f96650585a", vec![2, 7]))),
        ("mm", (edx, eax), false, Ok(("66a10000668b0e0000669966f7f96687c2", vec![2, 7]))),
        ("mm", (ebx, eax), false, Ok(("66a10000668b0e0000669966f7f9668bd8668bc2", vec![2, 7]))),
        ("mb", (eax, edx), false, Ok(("668b840000668b8f0000669966f7f9", vec![3, 8]))),
        ("mb", (eax, ebx), true, Ok(("668b840000668b8f0000669966f7f9668bda6650585a", vec![3, 8]))),
        ("mb", (ecx, edx), false, Ok(("668b840000668b8f0000669966f7f9668bc8", vec![3, 8]))),
        ("cm", (eax, edx), true, Ok(("66b8fbffffff668b0e0000669966f7f96650585a", vec![9]))),
        ("cm", (edx, eax), true, Ok(("66b8fbffffff668b0e0000669966f7f96687c26650585a", vec![9]))),
        ("cm", (ebx, eax), false, Ok(("66b8fbffffff668b0e0000669966f7f9668bd8668bc2", vec![9]))),
        ("held", (eax, edx), true, Err(held)),
        ("held", (edx, eax), false, Err(held)),
    ];
    for (key, seats, restore, want) in rows {
        let op = mir::Op { kind: Kind::Divmod, args: args(key), ..mir::Op::new(0, OpCode::Operation(Operation::Call), "B$DVI4", vec![], vec![]) };
        let got = divides(&op, seats, restore).map(|made| (hex(&made.code), made.fields));
        let want = want.map(|(code, fields)| (code.to_owned(), fields)).map_err(str::to_owned);
        assert_eq!(got, want, "{key} {seats:?} {restore}");
    }
    let op = mir::Op { kind: Kind::Mul, args: args("cc"), ..mir::Op::new(0, OpCode::Operation(Operation::Call), "B$DVI4", vec![], vec![]) };
    assert_eq!(divides(&op, (eax, edx), true).map(|made| made.code), Err("B$DVI4: not a divide over two operands".to_owned()));
}
