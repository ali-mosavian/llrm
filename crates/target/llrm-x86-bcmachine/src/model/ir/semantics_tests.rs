//! Port of the single-instruction half of `tests/test_ir.py`.

use std::collections::BTreeSet;

use iced_x86::Register;

use super::*;
use crate::frontends::bc::declen::decode;
use crate::model::ir::nodes::{Node, Opaque, pinned};
use crate::model::ir::{Address, modelled};
use crate::objectfile::module::{Addr, literal_only};

/// helpers' `hx`.
fn hx(s: &str) -> Vec<u8> {
    let digits: String = s.split_whitespace().collect();
    (0..digits.len()).step_by(2).map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap()).collect()
}

const STATIC: Addr = Addr::new(Space::Literal, 0x1234);
const LOCAL: Addr = Addr::new(Space::Frame, -4);
const FRAME: Addr = Addr::new(Space::Frame, -0x38);
const AX: Loc = reg(Register::AX, 2);
const DX: Loc = reg(Register::DX, 2);
const BX: Loc = reg(Register::BX, 2);
const EAX: Loc = reg(Register::EAX, 4);
const ECX: Loc = reg(Register::ECX, 4);
const EDX: Loc = reg(Register::EDX, 4);
const ES: Loc = reg(Register::ES, 2);
const CS: Loc = reg(Register::CS, 2);
const ST0: Loc = Loc::St(St { index: 0 });
const ST1: Loc = Loc::St(St { index: 1 });

fn mem(addr: Option<Addr>, width: u32) -> Loc {
    Loc::Mem(Mem::new(addr, width))
}

fn imm(value: i64, width: u32) -> Loc {
    Loc::Imm(Imm { value, width, address: None })
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    shaped(op, name, dests, sources)
}

fn _insn(code: &str) -> Insn {
    decode(&hx(code), 0).unwrap()
}

fn _effects(code: &str) -> Effects {
    instruction_effects(&_insn(code), &literal_only)
}

fn _semantics(code: &str) -> Semantics {
    instruction_semantics(&_insn(code), &literal_only)
}

fn _defs(code: &str) -> BTreeSet<Register> {
    _effects(code).defs.expect("a real instruction, not a call")
}

fn _uses(code: &str) -> BTreeSet<Register> {
    _effects(code).uses.expect("a real instruction, not a call")
}

#[test]
fn test_a_sub_register_write_is_normalised_in_a_real_instructions_own_effects() {
    let effects = _effects("B0 05");
    assert_eq!(effects.defs, Some(BTreeSet::from([Register::EAX])));
    assert_eq!(effects.uses, Some(BTreeSet::from([Register::EAX])));
}

#[test]
fn test_a_call_or_interrupt_gets_the_conservative_answer_not_iceds_own() {
    let effects = _effects("9A 00 00 00 00");
    assert_eq!(effects.defs, None);
    assert_eq!(effects.uses, None);
    assert_eq!(effects.flags_written, ALL);
    assert_eq!(effects.flags_read, ALL);
    assert_eq!(effects.loads, vec![Mem::new(None, 0)]);
    assert_eq!(effects.stores, vec![Mem::new(None, 0)]);
}

#[test]
fn test_a_barriers_memory_reach_is_unknown_in_both_directions() {
    let effects = _effects("EC");
    assert!(effects.touches_memory());
    assert_eq!(effects.loads, *ANY_MEMORY);
    assert_eq!(effects.stores, *ANY_MEMORY);
}

#[test]
fn test_one_instructions_own_operation_shape() {
    let lea = Loc::Address(Address::new(Some(Addr::new(Space::Frame, -0x1A))));
    let cases = [
        ("8B 06 34 12", semantics(Operation::Move, "mov", vec![AX], vec![mem(Some(STATIC), 2)])),
        ("89 46 FC", semantics(Operation::Move, "mov", vec![mem(Some(LOCAL), 2)], vec![AX])),
        ("03 C2", semantics(Operation::Binary, "add", vec![AX], vec![AX, DX])),
        ("13 D3", semantics(Operation::Binary, "adc", vec![DX], vec![DX, BX])),
        ("F7 D0", semantics(Operation::Unary, "not", vec![AX], vec![AX])),
        ("99", semantics(Operation::Extend, "cwd", vec![DX], vec![AX])),
        ("FF 36 34 12", semantics(Operation::Push, "push", vec![], vec![mem(Some(STATIC), 2)])),
        ("58", semantics(Operation::Pop, "pop", vec![AX], vec![])),
        ("8D 7E E6", semantics(Operation::Address, "lea", vec![reg(Register::DI, 2)], vec![lea])),
        ("83 3E 34 12 05", semantics(Operation::Compare, "cmp", vec![], vec![mem(Some(STATIC), 2), imm(5, 2)])),
        ("75 10", Semantics { target: Some(0x12), ..semantics(Operation::Branch, "jne", vec![], vec![]) }),
        ("E9 00 01", Semantics { target: Some(0x103), ..semantics(Operation::Jump, "jmp", vec![], vec![]) }),
        ("C3", semantics(Operation::Return, "ret", vec![], vec![])),
        ("9A 00 00 00 00", semantics(Operation::Call, "call", vec![], vec![])),
    ];
    for (code, expected) in cases {
        assert_eq!(_semantics(code), expected, "{code}");
    }
}

#[test]
fn test_an_immediate_is_reported_as_the_value_it_means() {
    for (code, expected) in
        [("83 C0 FF", imm(-1, 2)), ("05 FF FF", imm(-1, 2)), ("6A FE", imm(-2, 2)), ("66 6A FE", imm(-2, 4))]
    {
        assert_eq!(_semantics(code).sources.last(), Some(&expected), "{code}");
    }
}

#[test]
fn test_a_dword_immediate_store_carries_both_the_address_and_the_value() {
    assert_eq!(
        _semantics("66 C7 06 34 12 78 56 34 12"),
        semantics(Operation::Move, "mov", vec![mem(Some(STATIC), 4)], vec![imm(0x12345678, 4)])
    );
}

#[test]
fn test_a_push_of_a_static_writes_a_stack_cell_that_is_not_that_static() {
    let effects = _effects("FF 36 34 12");
    assert_eq!(effects.loads, vec![Mem::new(Some(STATIC), 2)]);
    assert_eq!(effects.stores, vec![Mem::new(None, 2)]);
}

#[test]
fn test_a_load_writes_no_memory_and_a_store_reads_none() {
    assert_eq!(_effects("8B 06 34 12").stores, vec![]);
    assert_eq!(_effects("89 46 FC").loads, vec![]);
    assert_eq!(_effects("89 46 FC").stores, vec![Mem::new(Some(LOCAL), 2)]);
}

#[test]
fn test_lea_touches_no_memory_at_all() {
    let effects = _effects("8D 7E E6");
    assert_eq!(effects.loads, vec![]);
    assert_eq!(effects.stores, vec![]);
    assert!(!effects.touches_memory());
    assert_eq!(effects.flags_written, Flag::NONE);
}

#[test]
fn test_cwd_writes_dx_alone_and_reads_ax() {
    assert_eq!(_defs("99"), BTreeSet::from([Register::EDX]));
    assert!(!_defs("99").contains(&Register::EAX));
    assert_eq!(_uses("99"), BTreeSet::from([Register::EAX, Register::EDX]));
}

#[test]
fn test_the_absorbed_divide_names_both_of_its_destinations() {
    assert_eq!(_semantics("66 F7 F9"), semantics(Operation::Divide, "idiv", vec![EAX, EDX], vec![EDX, EAX, ECX]));
    assert_eq!(_defs("66 F7 F9"), BTreeSet::from([Register::EAX, Register::EDX]));
    assert!(!_defs("66 F7 F9").contains(&Register::ECX));
}

#[test]
fn test_the_absorbed_multiply_leaves_edx_alone() {
    assert_eq!(_semantics("66 0F AF C1"), semantics(Operation::Multiply, "imul", vec![EAX], vec![EAX, ECX]));
    assert_eq!(_defs("66 0F AF C1"), BTreeSet::from([Register::EAX]));
}

#[test]
fn test_a_three_operand_multiply_does_not_read_its_own_destination() {
    assert_eq!(_semantics("66 6B C1 04"), semantics(Operation::Multiply, "imul", vec![EAX], vec![ECX, imm(4, 4)]));
    assert!(!_uses("66 6B C1 04").contains(&Register::EAX));
}

#[test]
fn test_a_branch_reads_the_flags_it_tests_and_writes_none() {
    let effects = _effects("75 10");
    assert_eq!(effects.flags_read, Flag::ZF);
    assert_eq!(effects.flags_written, Flag::NONE);
    assert_eq!(effects.defs, Some(BTreeSet::new()));
}

#[test]
fn test_add_reads_no_flag_and_adc_reads_the_carry() {
    assert_eq!(_effects("03 C2").flags_read, Flag::NONE);
    assert_eq!(_effects("13 D3").flags_read, Flag::CF);
}

#[test]
fn test_not_writes_no_flags() {
    assert_eq!(_effects("F7 D0").flags_written, Flag::NONE);
}

#[test]
fn test_the_deliberately_refused_shapes_stay_barriers() {
    for (code, why) in [
        ("D9 C1", "fld st(1), a stack duplicate -- no memory operand for _float_load to accept"),
        ("CD 21", "an ordinary software interrupt"),
        ("E4 40", "in -- what it does happens in a device"),
        ("EE", "out, likewise"),
        ("AB", "stosw with no rep prefix: a different shape, absent from this corpus"),
        ("F6 E9", "byte-wide imul, whose whole product lands in ax rather than a pair"),
        ("FF 2E 34 12", "an indirect far jmp, which reads its target out of memory"),
    ] {
        assert_eq!(_semantics(code), *UNMODELLED, "{why}");
    }
}

fn far_bx() -> Addr {
    Addr { base: Register::BX, segment: Register::ES, ..Addr::new(Space::Far, 0) }
}

#[test]
fn test_a_far_pointer_load_is_modelled() {
    assert_eq!(_semantics("26 8B 07"), semantics(Operation::Move, "mov", vec![AX], vec![mem(Some(far_bx()), 2)]));
}

#[test]
fn test_a_far_pointer_store_is_modelled() {
    assert_eq!(_semantics("26 89 07"), semantics(Operation::Move, "mov", vec![mem(Some(far_bx()), 2)], vec![AX]));
}

#[test]
fn test_a_descriptor_load_into_a_segment_register_is_modelled() {
    let found = Addr { base: Register::SI, ..Addr::new(Space::Literal, 2) };
    assert_eq!(_semantics("8E 44 02"), semantics(Operation::Move, "mov", vec![ES], vec![mem(Some(found), 2)]));
}

#[test]
fn test_a_redundant_ds_prefix_is_not_an_override_to_record() {
    let literal = Addr::new(Space::Literal, 0);
    assert_eq!(_semantics("3E 8E 06 00 00"), semantics(Operation::Move, "mov", vec![ES], vec![mem(Some(literal), 2)]));
}

#[test]
fn test_reading_a_segment_register_is_modelled_too() {
    assert_eq!(_semantics("8C C8"), semantics(Operation::Move, "mov", vec![AX], vec![CS]));
}

#[test]
fn test_a_segment_override_is_modelled_with_its_segment() {
    let found = _semantics("26 8B 44 02");
    assert!(found.op == Operation::Move && found.dests == vec![AX]);
    let [Loc::Mem(source)] = found.sources.as_slice() else {
        panic!("one memory source");
    };
    assert!(source.addr.is_some_and(|addr| addr.segment == Register::ES));
    assert!(modelled(&found));
}

#[test]
fn test_xchg_is_a_genuine_two_way_swap() {
    assert_eq!(_semantics("93"), semantics(Operation::Exchange, "xchg", vec![BX, AX], vec![AX, BX]));
    assert_eq!(_defs("93"), BTreeSet::from([Register::EBX, Register::EAX]));
    assert_eq!(_uses("93"), BTreeSet::from([Register::EBX, Register::EAX]));
}

#[test]
fn test_a_barrier_still_carries_a_complete_effect() {
    assert_eq!(_defs("E4 40"), BTreeSet::from([Register::EAX]));
    assert_eq!(_effects("E4 40").flags_written, Flag::NONE);
}

#[test]
fn test_the_x87_vocabulary_is_modelled_in_its_one_measured_shape() {
    let frame = |width| mem(Some(FRAME), width);
    let cases = [
        ("D9 46 C8", semantics(Operation::FloatLoad, "fld", vec![ST0], vec![frame(4)])),
        ("DD 46 C8", semantics(Operation::FloatLoad, "fld", vec![ST0], vec![frame(8)])),
        ("DB 46 C8", semantics(Operation::FloatLoad, "fild", vec![ST0], vec![frame(4)])),
        ("DF 46 C8", semantics(Operation::FloatLoad, "fild", vec![ST0], vec![frame(2)])),
        ("D9 5E C8", semantics(Operation::FloatStore, "fstp", vec![frame(4)], vec![ST0])),
        ("DD 5E C8", semantics(Operation::FloatStore, "fstp", vec![frame(8)], vec![ST0])),
        ("DF 5E C8", semantics(Operation::FloatStore, "fistp", vec![frame(2)], vec![ST0])),
        ("DB 5E C8", semantics(Operation::FloatStore, "fistp", vec![frame(4)], vec![ST0])),
        ("D8 46 C8", semantics(Operation::FloatArith, "fadd", vec![ST0], vec![ST0, frame(4)])),
        ("D8 66 C8", semantics(Operation::FloatArith, "fsub", vec![ST0], vec![ST0, frame(4)])),
        ("D8 4E C8", semantics(Operation::FloatArith, "fmul", vec![ST0], vec![ST0, frame(4)])),
        ("D8 76 C8", semantics(Operation::FloatArith, "fdiv", vec![ST0], vec![ST0, frame(4)])),
        ("DE 66 C8", semantics(Operation::FloatArith, "fisub", vec![ST0], vec![ST0, frame(2)])),
        ("DE 76 C8", semantics(Operation::FloatArith, "fidiv", vec![ST0], vec![ST0, frame(2)])),
        ("DE C1", semantics(Operation::FloatArithPop, "faddp", vec![ST1], vec![ST1, ST0])),
        ("DE E9", semantics(Operation::FloatArithPop, "fsubp", vec![ST1], vec![ST1, ST0])),
        ("DE C9", semantics(Operation::FloatArithPop, "fmulp", vec![ST1], vec![ST1, ST0])),
        ("DE F9", semantics(Operation::FloatArithPop, "fdivp", vec![ST1], vec![ST1, ST0])),
        ("D9 E0", semantics(Operation::FloatUnary, "fchs", vec![ST0], vec![ST0])),
        ("D9 E1", semantics(Operation::FloatUnary, "fabs", vec![ST0], vec![ST0])),
        ("D9 FA", semantics(Operation::FloatUnary, "fsqrt", vec![ST0], vec![ST0])),
        ("9B", semantics(Operation::Nothing, "wait", vec![], vec![])),
    ];
    for (code, expected) in cases {
        assert_eq!(_semantics(code), expected, "{code}");
    }
}

#[test]
fn test_an_emulated_x87_site_models_identically_to_its_literal_encoding() {
    let literal = _insn("D9 46 C8");
    let emulated = _insn("CD 35 46 C8");
    assert_eq!(emulated.length, 4);
    assert_eq!(instruction_semantics(&emulated, &literal_only), instruction_semantics(&literal, &literal_only));
    let real_effects = instruction_effects(&literal, &literal_only);
    let emulated_effects = instruction_effects(&emulated, &literal_only);
    assert_eq!(emulated_effects.loads, real_effects.loads);
    assert_eq!(real_effects.loads, vec![Mem::new(Some(FRAME), 4)]);
    assert_eq!(emulated_effects.stores, real_effects.stores);
    assert_eq!(real_effects.stores, vec![]);
}

#[test]
fn test_an_x87_loads_memory_reach_is_the_real_operand_not_any_memory() {
    let effects = _effects("D9 46 C8");
    assert_eq!(effects.loads, vec![Mem::new(Some(FRAME), 4)]);
    assert_eq!(effects.stores, vec![]);
    assert!(effects.fp_stack);
    assert_eq!(effects.flags_written, Flag::NONE);
    assert_eq!(effects.flags_read, Flag::NONE);
}

#[test]
fn test_an_x87_stores_memory_reach_is_the_real_operand_too() {
    let effects = _effects("DD 5E C8");
    assert_eq!(effects.loads, vec![]);
    assert_eq!(effects.stores, vec![Mem::new(Some(FRAME), 8)]);
    assert!(effects.fp_stack);
}

#[test]
fn test_x87_arithmetic_touches_the_fp_stack_without_naming_it_as_memory() {
    let effects = _effects("D8 46 C8");
    assert!(effects.fp_stack);
    assert_eq!(effects.loads, vec![Mem::new(Some(FRAME), 4)]);
    assert_eq!(effects.stores, vec![]);
}

#[test]
fn test_wait_touches_no_resource_this_module_tracks() {
    let effects = _effects("9B");
    assert!(!effects.fp_stack);
    assert_eq!(effects.defs, Some(BTreeSet::new()));
    assert_eq!(effects.uses, Some(BTreeSet::new()));
    assert_eq!(effects.loads, vec![]);
    assert_eq!(effects.stores, vec![]);
    assert_eq!(effects.flags_written, Flag::NONE);
}

#[test]
fn test_a_call_may_reach_the_fp_stack_too() {
    assert!(_effects("9A 00 00 00 00").fp_stack);
}

#[test]
fn test_a_segment_register_is_pushed_and_popped_like_any_other() {
    for (code, expected) in [
        ("0E", semantics(Operation::Push, "push", vec![], vec![CS])),
        ("16", semantics(Operation::Push, "push", vec![], vec![reg(Register::SS, 2)])),
        ("07", semantics(Operation::Pop, "pop", vec![ES], vec![])),
    ] {
        assert_eq!(_semantics(code), expected, "{code}");
    }
}

#[test]
fn test_leave_names_both_registers_it_writes_and_the_one_it_reads() {
    assert_eq!(
        _semantics("C9"),
        semantics(
            Operation::Leave,
            "leave",
            vec![reg(Register::SP, 2), reg(Register::BP, 2)],
            vec![reg(Register::BP, 2)]
        )
    );
    assert_eq!(_defs("C9"), BTreeSet::from([Register::EBP, Register::ESP]));
    assert_eq!(_effects("C9").flags_written, Flag::NONE);
}

#[test]
fn test_a_rep_fill_writes_a_cell_it_cannot_name_and_reads_its_count() {
    assert_eq!(
        _semantics("F3 AB"),
        semantics(
            Operation::Fill,
            "stosw",
            vec![mem(None, 0)],
            vec![AX, reg(Register::CX, 2), reg(Register::DI, 2), ES]
        )
    );
    assert_eq!(_effects("F3 AB").stores, vec![Mem::new(None, 0)]);
    assert_eq!(_effects("F3 AB").loads, vec![]);
}

#[test]
fn test_the_widening_multiply_names_both_halves_of_its_product() {
    assert_eq!(
        _semantics("F7 E9"),
        semantics(Operation::Multiply, "imul", vec![AX, DX], vec![AX, reg(Register::CX, 2)])
    );
    assert_eq!(_semantics("66 F7 E9"), semantics(Operation::Multiply, "imul", vec![EAX, EDX], vec![EAX, ECX]));
    assert_eq!(_defs("F7 E9"), BTreeSet::from([Register::EAX, Register::EDX]));
}

#[test]
fn test_a_far_jump_is_control_leaving_the_body_with_no_target_invented() {
    let found = _semantics("EA 00 00 00 00");
    assert_eq!(found, semantics(Operation::Escape, "jmp", vec![], vec![]));
    assert_eq!(found.target, None);
    assert!(modelled(&found));
}

#[test]
fn test_the_frame_and_shift_forms_are_modelled() {
    for (code, op, name) in [
        ("CB", Operation::Return, "retf"),
        ("CA 04 00", Operation::Return, "retf"),
        ("C1 E6 02", Operation::Binary, "shl"),
        ("D1 EE", Operation::Binary, "shr"),
        ("C1 FE 02", Operation::Binary, "sar"),
        ("90", Operation::Nothing, "nop"),
    ] {
        let found = _semantics(code);
        assert_eq!(found.op, op, "{code}");
        assert_eq!(found.name.as_deref(), Some(name), "{code}");
    }
}

#[test]
fn test_a_shift_reads_its_own_destination() {
    let found = _semantics("C1 E6 02");
    assert_eq!(found.dests[0], found.sources[0]);
    assert!(matches!(found.sources[1], Loc::Imm(_)));
}

#[test]
fn test_a_far_return_carries_the_bytes_it_pops() {
    assert_eq!(_semantics("CA 04 00").sources, vec![imm(4, 2)]);
}

#[test]
fn test_a_barrier_whose_reach_is_wholly_unknown_pins_every_register() {
    let insn = decode(&hx("CD 21"), 0).unwrap();
    let node = Node::Opaque(Opaque {
        effects: instruction_effects(&insn, &literal_only),
        semantics: UNMODELLED.clone(),
        insn,
    });
    assert_eq!(node.effects().defs, None);
    assert_eq!(pinned(&node), None);
}
