//! Port of `tests/test_layout.py`: a whole body emitted, and everything that
//! moves with it. `test_a_tangled_class_is_split_on_the_phi_edge` checks
//! `legacy/regalloc`, which is not ported; the Emitted-places test lives in
//! `select_tests`; `test_peeled_ivarm_*` monkeypatches `transform.applied`
//! and `test_a_moved_operation_keeps_its_fixup` calls `mir.bodies`, so both
//! wait for the BC raise.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::{Decoder, DecoderOptions, FlowControl, Mnemonic, OpKind, Register};

use super::*;
use crate::frontend::{blocks as split, declen};
use crate::model::ir::{self, nodes, Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::mir;
use crate::objectfile::module::{self, Group, Module, SourceMap};
use crate::objectfile::omf;
use crate::wholeseg;

/// Python's `SimpleNamespace(code=..., absorbed={}, fixup_at={}, ...)`.
pub(crate) fn bare(code: &[u8]) -> Module {
    Module {
        records: Vec::new(),
        seg: 0,
        name: String::new(),
        code: code.to_vec(),
        start: 0,
        end: code.len() as i64,
        operands: IndexMap::default(),
        calls: IndexMap::default(),
        targets: BTreeSet::new(),
        publics: BTreeSet::new(),
        lines: BTreeSet::new(),
        chunks: Vec::new(),
        sites: BTreeSet::new(),
        fixup_at: IndexMap::default(),
        dgroup: Group::new([], []),
        program_data: None,
        refs: IndexMap::default(),
        float_protocols: IndexMap::default(),
        absorbed: IndexMap::default(),
        coverage: IndexMap::default(),
    }
}

fn sem(op: Operation, name: &str) -> Semantics {
    Semantics { name: Some(name.to_owned()), ..Semantics::new(op) }
}

fn targeted(op: Operation, name: &str, target: i64) -> Semantics {
    Semantics { target: Some(target), ..sem(op, name) }
}

fn mov_ax_1() -> Semantics {
    Semantics {
        dests: vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
        sources: vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
        ..sem(Operation::Move, "mov")
    }
}

fn insn(at: i64, what: Semantics) -> Arc<Insn> {
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
    LirBlock { succ, ..LirBlock::new(at, insns) }
}

fn body(name: &str, entry: i64, blocks: Vec<LirBlock>) -> LirBody {
    LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default())
}

fn decoded(code: &[u8]) -> Vec<iced_x86::Instruction> {
    Decoder::with_ip(16, code, 0, DecoderOptions::NONE).into_iter().collect()
}

fn emitted(path: &str) -> wholeseg::Emitted {
    let data = std::fs::read(path).unwrap();
    wholeseg::emitted(
        &data,
        true,
        true,
        None,
        None,
        crate::backend::cpu::ProfileOrName::Name("386"),
        false,
        false,
        None,
        &crate::model::passes::O2(),
    )
    .unwrap()
}

#[test]
fn test_reordered_block_materializes_its_cfg_fallthrough() {
    // Peeled IVARM's latch fell into its own header instead of the next iteration.
    for conditional in [false, true] {
        let what = if conditional { targeted(Operation::Branch, "jne", 30) } else { sem(Operation::Nothing, "nop") };
        let op = Arc::new(Insn::new(10, Some((10, 11)), Some(what), vec![], vec![]));
        let made = body(
            "fallthrough",
            10,
            vec![
                block(10, vec![op], if conditional { vec![30, 40] } else { vec![40] }),
                block(20, vec![], vec![]),
                block(30, vec![], vec![]),
                block(40, vec![], vec![]),
            ],
        );
        let changed = _fallthroughs(made);
        let jump = changed.blocks.iter().find(|block| block.at == 10).unwrap().insns.last().unwrap().clone();
        assert_eq!(jump.what, Some(targeted(Operation::Jump, "jmp", 40)));
        assert_eq!(jump.covers, Some((10, 10)));
        assert!(jump.node.is_none() && jump.symbol == Some(false));
        assert_eq!(_fallthroughs(changed.clone()), changed);
    }
}

#[test]
fn test_empty_reordered_block_gets_its_own_jump_anchor() {
    let made = body(
        "empty",
        10,
        vec![block(10, vec![], vec![30]), block(20, vec![], vec![]), block(30, vec![], vec![])],
    );
    let changed = _fallthroughs(made);
    let first = changed.blocks.iter().find(|block| block.at == 10).unwrap().insns[0].clone();
    assert!(Arc::ptr_eq(&_anchors(&changed)[&10], &first));
    assert_eq!(first.what.as_ref().unwrap().target, Some(30));
}

/// Frontend-parity LOOP hung after its exit branch became `jmp self`: an
/// inserted instruction shared the branch's source address, and resolving the
/// label through the address map sent the exit edge back to the first
/// occurrence.
#[test]
fn test_branch_target_is_the_block_occurrence_not_a_reused_source_address() {
    let branch = insn(10, targeted(Operation::Jump, "jmp", 30));
    let middle = insn(20, mov_ax_1());
    // Deliberately shares `at` with the branch.
    let exit = insn(10, sem(Operation::Return, "ret"));
    let made = body(
        "same-source-address",
        10,
        vec![block(10, vec![branch], vec![30]), block(20, vec![middle], vec![]), block(30, vec![exit], vec![])],
    );
    let found = bare(&[0; 40]);

    let emitted = lay_out(&made, 0, &found, &BTreeSet::new(), Some(&SourceMap::default())).unwrap();

    let decoded = decoded(&emitted.code);
    assert_eq!(decoded[0].near_branch_target(), decoded.last().unwrap().ip());
}

/// Frontend-parity LOOP changed its exit edge into a branch to its body: the
/// inverted branch must target the former fall-through edge.
#[test]
fn test_inverted_fallthrough_branch_targets_the_other_edge() {
    let branch = insn(10, targeted(Operation::Branch, "jg", 20));
    let placed_next = insn(20, mov_ax_1());
    let exit = insn(40, sem(Operation::Return, "ret"));
    let made = body(
        "inverted-exit",
        10,
        vec![
            block(10, vec![branch], vec![20, 40]),
            block(20, vec![placed_next], vec![10]),
            block(40, vec![exit], vec![]),
        ],
    );

    let changed = _fallthroughs(made);
    let last = changed.blocks[0].insns.last().unwrap().what.clone();

    assert_eq!(last, Some(targeted(Operation::Branch, "jle", 40)));
}

/// suite/arrays' second loop never exited: `jle latch; jmp body`. The empty
/// latch was given its jump back to the body, and the exit edge fell into it.
#[test]
fn test_a_jump_given_to_an_empty_block_does_not_steal_its_predecessors_fallthrough() {
    let work = insn(5, mov_ax_1());
    let test = insn(10, targeted(Operation::Branch, "jle", 20));
    let exit = insn(30, sem(Operation::Return, "ret"));
    let made = body(
        "empty-latch",
        5,
        vec![
            block(5, vec![work], vec![10]),
            block(10, vec![test], vec![20, 30]),
            block(20, vec![], vec![5]),
            block(30, vec![exit], vec![]),
        ],
    );
    let found = bare(&[0; 40]);

    let emitted = lay_out(&_fallthroughs(made), 0, &found, &BTreeSet::new(), Some(&SourceMap::default())).unwrap();

    let decoded = decoded(&emitted.code);
    let ret = decoded.iter().find(|one| one.mnemonic() == Mnemonic::Ret).unwrap().ip();
    let branch = decoded.iter().find(|one| one.flow_control() == FlowControl::ConditionalBranch).unwrap();
    assert!(ret == branch.near_branch_target() || ret == branch.next_ip());
}

/// PRESSX retained an unconditional jump to its exit immediately after loop elimination.
#[test]
#[ignore = "needs BC raise"]
fn test_pressx_has_no_jump_to_the_following_instruction() {
    let result = emitted("fixtures/omf/pressx-p-g2.obj");
    assert_eq!(result.outcome, wholeseg::Emission::Lir, "{}", result.reason);
    let found = module::of(&omf::parse(&result.data).unwrap()).unwrap();
    let mapped = split::code_map(&found).unwrap();
    for block in split::partition(&found, &mapped) {
        for one in &block.insns {
            let insn = &one.insn;
            if insn.mnemonic() == Mnemonic::Jmp && matches!(insn.op0_kind(), OpKind::NearBranch16 | OpKind::NearBranch32) {
                assert_ne!(insn.near_branch_target(), insn.next_ip());
            }
        }
    }
}

/// Removing PRESSX's empty exit jump must not remove a loop or execute skipped data.
#[test]
fn test_fallthrough_relaxation_preserves_targets_and_intervening_data() {
    let jump = |at: i64, to: i64| {
        Item::Op(Arc::new(Insn::new(at, Some((at, at + 2)), Some(targeted(Operation::Jump, "jmp", to)), vec![], vec![])))
    };
    let ret = |at: i64| Item::Op(Arc::new(Insn::new(at, Some((at, at + 1)), Some(sem(Operation::Return, "ret")), vec![], vec![])));
    for shape in ["next", "chain", "self", "data"] {
        let (ops, code): (Vec<Item>, Vec<u8>) = match shape {
            "chain" => (vec![jump(0, 2), jump(2, 4), ret(4)], vec![0xEB, 0x00, 0xEB, 0x00, 0xC3]),
            "self" => (vec![jump(0, 0), ret(2)], vec![0xEB, 0x00, 0xC3]),
            "data" => (vec![jump(0, 3), Item::Table(Table::new(2, 3)), ret(3)], vec![0xEB, 0x01, 0x90, 0xC3]),
            _ => (vec![jump(0, 2), ret(2)], vec![0xEB, 0x00, 0xC3]),
        };
        let found = bare(&code);
        let result = asm::assemble(
            &ops,
            0,
            &found,
            &BTreeSet::new(),
            false,
            None,
            None,
            None,
            None,
            Some(&SourceMap::default()),
        )
        .unwrap();
        let expected: &[u8] = match shape {
            "next" | "chain" => &[0xC3],
            "self" => &[0xEB, 0xFE, 0xC3],
            _ => &[0xEB, 0x01, 0x90, 0xC3],
        };
        assert_eq!(result.code, expected, "{shape}");
    }
}

/// nbody's copied FLD still read SI after allocation moved its pointer.
#[test]
fn test_emulator_load_uses_the_allocated_address() {
    let raw = [0xCD, 0x35, 0x04];
    let through = |register| {
        Semantics {
            dests: vec![Loc::St(ir::St { index: 0 })],
            sources: vec![Loc::Mem(ir::Mem { through: register, ..ir::Mem::new(None, 4) })],
            ..sem(Operation::FloatLoad, "fld")
        }
    };
    let node = nodes::Node::Opaque(nodes::Opaque {
        insn: declen::decode(&raw, 0).unwrap(),
        effects: ir::NO_EFFECT.clone(),
        semantics: through(Register::SI),
    });
    let source = mir::Op {
        kind: mir::Kind::Fload,
        source_backed: true,
        id: Some(1),
        absorbed: vec![1],
        ..mir::Op::new(0, mir::OpCode::Operation(Operation::FloatLoad), "fld", vec![], vec![])
    };
    let op = Insn {
        op: Some(Arc::new(source)),
        node: Some(Arc::new(node)),
        ..Insn::new(0, Some((0, 3)), Some(through(Register::DI)), vec![], vec![])
    };
    let found = bare(&raw);
    let done = asm::assemble(
        &[Item::Op(Arc::new(op))],
        0,
        &found,
        &BTreeSet::new(),
        false,
        None,
        None,
        None,
        None,
        Some(&SourceMap::default()),
    )
    .unwrap();
    assert_eq!(done.code, [0xCD, 0x35, 0x05]);
}

/// A rotated loop's preheader kept `jmp short` to the instruction after it.
#[test]
fn test_a_jump_to_the_block_placed_next_emits_nothing() {
    let jump = Arc::new(Insn::new(0, Some((0, 2)), Some(targeted(Operation::Jump, "jmp", 4)), vec![], vec![]));
    let work = Arc::new(Insn::new(4, Some((4, 6)), Some(sem(Operation::Move, "mov")), vec![], vec![]));
    let made = body("fall", 0, vec![block(0, vec![jump], vec![4]), block(4, vec![work], vec![])]);
    let fallen = _fallen(made);
    let last = fallen.blocks[0].insns.last().unwrap();
    assert!(last.what.as_ref().unwrap().op == Operation::Nothing && last.covers == Some((0, 2)));
}

/// BC's dead `jmp short` after a WEND reached by GOTO must not survive a
/// dropped jump: wrapping the angle ran it into the middle of an `add`.
#[test]
#[ignore = "needs BC raise"]
fn test_no_branch_lands_inside_an_instruction_after_a_dropped_jump() {
    let result = emitted("fixtures/omf/wendgo-q-o.obj");
    assert_eq!(result.outcome, wholeseg::Emission::Lir, "{}", result.reason);
    let found = module::of(&omf::parse(&result.data).unwrap()).unwrap();
    let mapped = split::code_map(&found).unwrap();
    let mut owners: IndexMap<usize, Vec<usize>> = IndexMap::default();
    for block in split::partition(&found, &mapped) {
        for one in &block.insns {
            for at in one.at..one.end() {
                owners.entry(at).or_default().push(one.at);
            }
        }
    }
    let shared: Vec<usize> = owners
        .iter()
        .filter(|(_, who)| who.iter().collect::<BTreeSet<_>>().len() > 1)
        .map(|(at, _)| *at)
        .collect();
    assert!(shared.is_empty(), "{shared:?}");
}

/// deedlines' plasmablobs: a THEN block emptied in MIR still held the phi's
/// `mov cx,-1`, and dropping the `jmp` over it inverted the plasma ramp.
#[test]
#[ignore = "needs BC raise"]
fn test_a_jump_over_a_block_holding_a_phi_copy_is_kept() {
    let result = emitted("fixtures/omf/rcflip-q-o.obj");
    assert_eq!(result.outcome, wholeseg::Emission::Lir, "{}", result.reason);
    let found = module::of(&omf::parse(&result.data).unwrap()).unwrap();
    let mapped = split::code_map(&found).unwrap();
    let mut insns: Vec<_> = split::partition(&found, &mapped).into_iter().flat_map(|block| block.insns).collect();
    insns.sort_by_key(|one| one.at);
    let collapsed: Vec<String> = insns
        .windows(2)
        .filter(|pair| {
            pair[0].insn.flow_control() == FlowControl::ConditionalBranch
                && pair[0].insn.near_branch_target() == pair[1].at as u64
        })
        .map(|pair| format!("{:#x}: {}", pair[0].at, pair[0].insn))
        .collect();
    assert!(collapsed.is_empty(), "{collapsed:?}");
}

// tests/test_emission_order.py: repeated provenance is not the order in
// which emitted instructions execute. `test_ordered_body_selects_ordered_object_layout`
// monkeypatches `layout.rebuild` and is not portable.

#[test]
fn test_linear_placement_requires_one_complete_acyclic_chain() {
    let cases: [([&[i64]; 3], [i64; 3]); 5] = [
        ([&[20], &[], &[10]], [0, 20, 10]),
        ([&[20], &[], &[0]], [0, 10, 20]),
        ([&[10, 20], &[], &[10]], [0, 10, 20]),
        ([&[10], &[], &[]], [0, 10, 20]),
        ([&[30], &[], &[10]], [0, 10, 20]),
    ];
    for (successors, expected) in cases {
        let blocks = [0, 10, 20]
            .into_iter()
            .zip(successors)
            .map(|(at, succ)| {
                let nothing = Arc::new(Insn::new(at, Some((at, at)), Some(sem(Operation::Nothing, "")), vec![], vec![]));
                block(at, vec![nothing], succ.to_vec())
            })
            .collect();
        let made = body("linear", 0, blocks);
        assert_eq!(_ordered(&made, true).iter().map(|op| op.at).collect::<Vec<_>>(), expected);
        assert_eq!(_ordered(&made, false).iter().map(|op| op.at).collect::<Vec<_>>(), [0, 10, 20]);
    }
}

/// FPCSE's removed loop still took three unconditional jumps through its old block layout.
#[test]
#[ignore = "needs BC raise"]
fn test_removed_floating_loop_is_emitted_in_execution_order() {
    for tag in ["p-g2", "q-o", "v-g3"] {
        let result = emitted(&format!("fixtures/omf/fpcse-{tag}.obj"));
        assert_eq!(result.outcome, wholeseg::Emission::Lir, "{}", result.reason);
        let found = module::of(&omf::parse(&result.data).unwrap()).unwrap();
        let mapped = split::code_map(&found).unwrap();
        assert!(
            !split::partition(&found, &mapped)
                .iter()
                .any(|block| block.insns.iter().any(|one| one.insn.mnemonic() == Mnemonic::Jmp)),
            "{tag}"
        );
    }
}

/// IVPROC VBDOS put zero padding between a specialized store and its jump.
#[test]
fn test_carried_padding_follows_its_original_byte_owner() {
    let nothing = || sem(Operation::Nothing, "");
    let owner = Arc::new(Insn::new(100, Some((100, 105)), Some(nothing()), vec![], vec![]));
    let clone = Arc::new(Insn::new(30, Some((30, 30)), Some(nothing()), vec![], vec![]));
    let earlier = Arc::new(Insn::new(10, Some((10, 15)), Some(nothing()), vec![], vec![]));
    let padding = Table::new(105, 107);
    assert_eq!(
        _interleaved(&[owner.clone(), clone.clone(), earlier.clone()], &[padding]),
        [Item::Op(owner), Item::Table(padding), Item::Op(clone), Item::Op(earlier)]
    );
}

fn moving(at: i64, number: i64, covers: (i64, i64)) -> Arc<Insn> {
    let what = Semantics {
        dests: vec![Loc::Reg(Reg { register: Register::AX, width: 2 })],
        sources: vec![Loc::Imm(Imm { value: number, width: 2, address: None })],
        ..sem(Operation::Move, "mov")
    };
    Arc::new(Insn::new(at, Some(covers), Some(what), vec![], vec![]))
}

fn rebuilt_ordered(found: &Module, name: &str, made: LirBody) -> Laid {
    rebuild(found, vec![(name.to_owned(), made)], &[], &BTreeSet::new(), None, false, None, true, &BTreeSet::new(), None)
        .unwrap()
}

/// FPDEEP's three iterations were interleaved by source address and timed out.
#[test]
fn test_explicit_emission_order_does_not_group_clones_by_source_address() {
    let first = moving(0, 1, (0, 3));
    let second = moving(3, 2, (3, 6));
    let clone = moving(0, 3, (0, 0));
    let made = body("order", 0, vec![block(0, vec![first, second, clone], vec![])]);
    let found = bare(&[0xB8, 0x01, 0x00, 0xB8, 0x02, 0x00]);
    let emitted = rebuilt_ordered(&found, "order", made);
    assert_eq!(emitted.code, [0xB8, 0x01, 0x00, 0xB8, 0x02, 0x00, 0xB8, 0x03, 0x00]);
    assert_eq!(emitted.moved[&0], 0);
    assert_eq!(emitted.moved[&3], 3);
}

/// FPDEEP's second and third printed iterations emitted unrelocated call 0:0.
#[test]
fn test_cloned_far_call_keeps_its_relocation_without_claiming_input_bytes() {
    let what = sem(Operation::Call, "call");
    let source = Arc::new(mir::Op {
        id: Some(7),
        symbol: Some(true),
        absorbed: vec![7],
        ..mir::Op::new(0, mir::OpCode::Operation(Operation::Call), "call", vec![], vec![])
    });
    let original = Arc::new(Insn {
        op: Some(source.clone()),
        symbol: Some(true),
        ..Insn::new(0, Some((0, 5)), Some(what), vec![], vec![])
    });
    let clone = Arc::new(Insn { covers: Some((0, 0)), ..(*original).clone() });
    let mut found = bare(&[0x9A, 0x00, 0x00, 0x00, 0x00]);
    found.coverage.insert(7, vec![(0, 5)]);
    found.calls.insert(0, "B$PSSD".to_owned());
    let fields = BTreeSet::from([1]);
    assert_eq!(asm::_ranges_of(&Item::Op(clone.clone()), &found, None), [(0, 0)]);
    assert_eq!(asm::_length_of(&clone, &found, None), Some(0));
    let emitted = asm::assemble(
        &[Item::Op(original), Item::Op(clone.clone())],
        0,
        &found,
        &fields,
        false,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(emitted.relocations, [(1, 1), (6, 1)]);
    let anonymous = Insn { op: Some(Arc::new(mir::Op { id: None, ..(*source).clone() })), ..(*clone).clone() };
    assert_eq!(asm::_field_in(&found, &anonymous, &fields, None), None);
    let unsymbolic = Insn { symbol: Some(false), ..(*clone).clone() };
    assert_eq!(asm::_field_in(&found, &unsymbolic, &fields, None), None);
}

/// A folded site's push run and call must survive after MIR coverage is gone.
#[test]
fn test_layout_uses_disjoint_ranges_resolved_onto_lir() {
    let source = mir::Op {
        id: Some(8),
        absorbed: vec![7, 8],
        ..mir::Op::new(20, mir::OpCode::Operation(Operation::Nothing), "", vec![], vec![])
    };
    let ranges = vec![(10, 12), (20, 23)];
    let instruction = Arc::new(Insn {
        spread: ranges.clone(),
        op: Some(Arc::new(source)),
        ..Insn::new(20, Some((20, 23)), Some(sem(Operation::Nothing, "")), vec![], vec![])
    });
    let found = bare(&[]);

    assert_eq!(asm::_ranges_of(&Item::Op(instruction.clone()), &found, None), ranges);
    assert_eq!(asm::_length_of(&instruction, &found, None), Some(5));
}

/// FPDEEP's entry jump landed in a cloned header before its first iteration.
#[test]
fn test_block_entry_is_not_the_first_clone_of_its_source_address() {
    let initial = moving(0, 1, (0, 3));
    let cloned_header = moving(3, 99, (3, 3));
    let header = moving(3, 2, (3, 6));
    let made = body("labels", 0, vec![block(0, vec![initial, cloned_header], vec![3]), block(3, vec![header], vec![])]);
    let found = bare(&[0xB8, 0x01, 0x00, 0xB8, 0x02, 0x00]);
    let emitted = rebuilt_ordered(&found, "labels", made);
    assert_eq!(emitted.code, [0xB8, 0x01, 0x00, 0xB8, 0x63, 0x00, 0xB8, 0x02, 0x00]);
    assert_eq!(emitted.moved[&3], 6);
}

// tests/test_loop_exit_layout.py

/// SCREEN's BG_BAND skipped a split exit and incremented x as the next row.
#[test]
fn test_split_exit_executes_its_reload_before_the_increment() {
    let found = module::of(&omf::parse(&std::fs::read("fixtures/omf/harr-v-g3.obj").unwrap()).unwrap()).unwrap();
    let bridge = 0x1_0000_0001;
    let op = |at: i64, meaning: Semantics| {
        Arc::new(Insn { symbol: Some(false), ..Insn::new(at, Some((at, at)), Some(meaning), vec![], vec![]) })
    };
    let ax = || Loc::Reg(Reg { register: Register::AX, width: 2 });
    let made = body(
        "split exit",
        0x30,
        vec![
            block(0x30, vec![op(0x30, targeted(Operation::Branch, "jle", 0x30))], vec![0x30, bridge]),
            block(
                0x40,
                vec![
                    op(0x40, Semantics { dests: vec![ax()], sources: vec![ax()], ..sem(Operation::Unary, "inc") }),
                    op(0x40, sem(Operation::Return, "ret")),
                ],
                vec![],
            ),
            block(
                bridge,
                vec![
                    op(
                        bridge,
                        Semantics {
                            dests: vec![ax()],
                            sources: vec![Loc::Imm(Imm { value: 7, width: 2, address: None })],
                            ..sem(Operation::Move, "mov")
                        },
                    ),
                    op(bridge, targeted(Operation::Jump, "jmp", 0x40)),
                ],
                vec![0x40],
            ),
        ],
    );
    let laid = rebuild(
        &found,
        vec![("split exit".to_owned(), made)],
        &[],
        &BTreeSet::new(),
        None,
        false,
        None,
        false,
        &BTreeSet::new(),
        None,
    )
    .unwrap();
    let instructions: Vec<_> = Decoder::with_ip(16, &laid.code, 0x30, DecoderOptions::NONE).into_iter().collect();
    assert_eq!(instructions[0].mnemonic(), Mnemonic::Jle);
    assert_eq!(instructions[1].mnemonic(), Mnemonic::Jmp);
    assert_eq!(instructions[1].near_branch_target() as i64, laid.moved[&bridge]);
}

// tests/test_symbolic_relocation.py. `test_load_hoisted_to_call_does_not_acquire_call_fixup`
// calls `mir.bodies` and waits for the BC raise.

fn selected(op: mir::Op, source: Option<&SourceMap>) -> Insn {
    let id = op.id.unwrap_or_default();
    let node = source.and_then(|source| source.nodes.get(&id).cloned());
    let ranges = source.and_then(|source| source.occurrences.get(&id).cloned()).unwrap_or_default();
    let what = crate::backend::lower::current(&op, crate::backend::lower::Place::Default, node.as_deref()).unwrap();
    let mut made = Insn::new(
        op.at,
        Some(ranges.first().copied().unwrap_or((op.at, op.at))),
        what,
        op.defines.iter().map(|one| one.id).collect(),
        op.uses.iter().map(|one| one.id).collect(),
    );
    made.symbol = op.symbol;
    made.node = node;
    made.op = Some(Arc::new(op));
    made
}

/// VBDOS nbody refused 0x1b7: promotion left a fixup on a register-to-register move.
#[test]
fn test_promoted_symbolic_load_drops_its_old_fixup() {
    let (source, result) = (mir::Value::new(1, 0), mir::Value::new(2, 0));
    let op = mir::Op {
        kind: mir::Kind::Copy,
        args: vec![mir::Arg::Held(mir::Held { value: source, width: 4 })],
        results: vec![mir::Arg::Held(mir::Held { value: result, width: 4 })],
        id: Some(1),
        symbol: Some(true),
        ..mir::Op::new(0x1b7, mir::OpCode::Operation(Operation::Move), "mov", vec![result], vec![source])
    };
    let mut found = bare(&[]);
    found.refs.insert(1, vec![0x1b9]);
    let source_map = SourceMap { refs: found.refs.clone(), ..SourceMap::default() };
    assert!(asm::_fields_in(&found, &selected(op, Some(&source_map)), &BTreeSet::new(), Some(&source_map)).is_empty());
}

/// VBDOS nbody crashed emission when a synthetic instruction's address exceeded BC's bytes.
#[test]
fn test_inserted_instruction_never_reads_original_interrupt_bytes() {
    let op = insn(100, mov_ax_1());
    let found = bare(&[0x90]);
    let done = asm::assemble(
        &[Item::Op(op)],
        0,
        &found,
        &BTreeSet::new(),
        false,
        None,
        None,
        None,
        None,
        Some(&SourceMap::default()),
    )
    .unwrap();
    assert_eq!(done.code, [0xB8, 0x01, 0x00]);
}

// tests/test_lir_emission_order.py

/// Qrender h_frame reported build time zero: layout moved XOR across CMP.
#[test]
fn test_zeroing_stays_before_the_comparison_in_emitted_bytes() {
    let found = module::of(&omf::parse(&std::fs::read("fixtures/omf/harr-v-g3.obj").unwrap()).unwrap()).unwrap();
    let empty = mir::MirBody::new(0x30, vec![mir::MirBlock::new(0x30, vec![], vec![], vec![])]);
    let lowered = crate::backend::lower::lowered(
        "flags",
        &empty,
        Some(&IndexMap::default()),
        BTreeSet::new(),
        Some(&IndexMap::default()),
        "386",
        Default::default(),
    )
    .unwrap();
    let dest = Loc::Reg(Reg { register: Register::AX, width: 2 });
    let zero = Arc::new(Insn::new(
        0x33,
        Some((0x33, 0x33)),
        Some(Semantics {
            dests: vec![dest],
            sources: vec![Loc::Imm(Imm { value: 0, width: 2, address: None })],
            ..sem(Operation::Move, "mov")
        }),
        vec![],
        vec![],
    ));
    let compare = Arc::new(Insn::new(
        0x30,
        Some((0x30, 0x30)),
        Some(Semantics {
            sources: vec![
                Loc::Reg(Reg { register: Register::BX, width: 2 }),
                Loc::Imm(Imm { value: 0, width: 2, address: None }),
            ],
            ..sem(Operation::Compare, "cmp")
        }),
        vec![],
        vec![],
    ));
    let replaced = LirBody { blocks: vec![LirBlock::new(0x30, vec![zero, compare])], ..lowered };
    let zeroed = crate::backend::peephole::zeroes(&replaced);
    let entries = if zeroed.ordered { BTreeSet::from([zeroed.entry]) } else { BTreeSet::new() };
    let laid = rebuild(
        &found,
        vec![(zeroed.name.clone(), zeroed)],
        &[],
        &BTreeSet::new(),
        None,
        false,
        None,
        false,
        &entries,
        None,
    )
    .unwrap();
    let mnemonics: Vec<Mnemonic> = decoded(&laid.code).iter().map(|one| one.mnemonic()).collect();
    assert_eq!(mnemonics, [Mnemonic::Xor, Mnemonic::Test]);
}
