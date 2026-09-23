//! Ports of the peephole tests: tests/test_peephole.py and the peephole-only
//! tests of test_machine_copyprop, test_memory_folding, test_addressforms,
//! test_dead_address_arithmetic, test_parcopy, test_postallocation and
//! test_prologue.  Checks Python made through `masm` compare the `repr` of
//! the semantics Python printed instead: `masm` is not ported.
use std::cell::RefCell;
use std::rc::Rc;

use crate::support::hash::HashMap;
use std::sync::Arc;

use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
use crate::support::hash::IndexMap;

use super::*;
use crate::backend::frame::Frame;
use crate::backend::{copyprop, parcopy, prologue, spillforward, verify};
use crate::model::ir::Addr;
use crate::model::mir::{Kind, OpCode};
use crate::support::pyrepr::Repr;

fn r(register: Register, width: u32) -> Reg {
    Reg { register, width }
}

fn rl(register: Register, width: u32) -> Loc {
    Loc::Reg(r(register, width))
}

fn im(value: i64, width: u32) -> Loc {
    Loc::Imm(imm(value, width))
}

fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    semantics(op, name, dests, sources)
}

fn semt(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Semantics {
    Semantics { target, ..semantics(op, name, dests, sources) }
}

fn insn(at: i64, covers: Option<(i64, i64)>, what: Option<Semantics>, defines: Vec<u32>, uses: Vec<u32>) -> Insn {
    Insn::new(at, covers, what, defines, uses)
}

fn block(at: i64, insns: Vec<Arc<Insn>>, succ: Vec<i64>) -> LirBlock {
    LirBlock { succ, ..LirBlock::new(at, insns) }
}

fn body(name: &str, entry: i64, blocks: Vec<LirBlock>) -> LirBody {
    LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default())
}

fn mem(addr: Option<Addr>, width: u32, through: Register, offset: i64, disp_width: u32) -> Mem {
    Mem { through, offset, disp_width, ..Mem::new(addr, width) }
}

fn frame(disp: i64) -> Option<Addr> {
    Some(Addr::new(Space::Frame, disp))
}

fn names(insns: &[Arc<Insn>]) -> Vec<String> {
    insns.iter().map(|one| one.what.as_ref().unwrap().name.clone().unwrap()).collect()
}

fn code(what: &Semantics) -> Vec<u8> {
    select::emit(what, 0, None, false, false, None).unwrap().code
}

fn peephole() -> Peephole {
    Peephole::new(None, "386").unwrap()
}

fn transform(input: LirBody) -> LirBody {
    peephole().transform(input).unwrap()
}

fn whats(insns: &[Arc<Insn>]) -> Vec<Semantics> {
    insns
        .iter()
        .filter(|one| one.what.as_ref().is_some_and(|what| what.op != Operation::Nothing))
        .map(|one| one.what.clone().unwrap())
        .collect()
}

fn op(at: i64, operation: Operation, name: &str, kind: Kind) -> mir::Op {
    mir::Op { kind, ..mir::Op::new(at, OpCode::Operation(operation), name, vec![], vec![]) }
}

// ---------------------------------------------------------------- test_peephole

#[test]
fn test_screen_argument_reuses_its_required_register_constant() {
    // SCREEN's duplicate PUSH 1/MOV AX,1 contributed to E1M1 exhausting its far heap.
    for (width, register, value) in [(2, Register::AX, 1), (4, Register::ECX, 0x1234_5678)] {
        let operand = im(value, width);
        let dest = rl(register, width);
        let push = insn(0, Some((0, 1)), Some(sem(Operation::Push, "push", vec![], vec![operand.clone()])), vec![], vec![]);
        let moved =
            insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![operand])), vec![7], vec![]);
        let result = transform(body("screen", 0, vec![block(0, vec![Arc::new(push.clone()), Arc::new(moved.clone())], vec![])]));
        let expected = [
            moved.what.clone().unwrap(),
            Semantics { sources: vec![dest], ..push.what.clone().unwrap() },
        ];
        let insns = result.insns();
        assert_eq!(
            insns.iter().flat_map(|one| code(one.what.as_ref().unwrap())).collect::<Vec<u8>>(),
            expected.iter().flat_map(code).collect::<Vec<u8>>()
        );
        assert_eq!(insns[0].defines, [7]);
        assert_eq!(insns[1].uses, [7]);
    }
}

fn concat_parts(last: Insn) -> Vec<Arc<Insn>> {
    let (high, low, result) = (rl(Register::DX, 2), rl(Register::AX, 2), rl(Register::EAX, 4));
    let marker = op(0, Operation::Move, "concat", Kind::Concat);
    vec![
        Arc::new(Insn {
            op: Some(Arc::new(marker)),
            ..insn(0, Some((0, 0)), Some(sem(Operation::Push, "push", vec![], vec![high])), vec![], vec![1])
        }),
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Push, "push", vec![], vec![low])), vec![], vec![2])),
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Pop, "pop", vec![result], vec![])), vec![3], vec![])),
        Arc::new(last),
    ]
}

#[test]
fn test_word_pair_concat_uses_the_386_funnel_sequence() {
    // qgl_surf_from_member used push DX/push AX/pop EAX; BCC needs only SHL/SHRD.
    let compare = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(Register::EAX, 4), im(0, 4)])),
        vec![],
        vec![3],
    );
    let transformed = transform(body("qgl_surf_from_member", 0, vec![block(0, concat_parts(compare), vec![])]));
    let emitted: Vec<String> = whats(&transformed.insns()).into_iter().map(|what| what.name.unwrap()).collect();
    assert_eq!(emitted[..2], ["shl", "shrd"]);
    assert!(
        !transformed
            .insns()
            .iter()
            .any(|one| matches!(one.what.as_ref().unwrap().op, Operation::Push | Operation::Pop))
    );
}

#[test]
fn test_word_pair_concat_keeps_the_stack_sequence_when_flags_are_live() {
    // SHL/SHRD modify flags; a branch reading the incoming flags must keep the stack join.
    let branch = insn(1, Some((1, 1)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(10))), vec![], vec![]);
    let transformed = transform(body(
        "flagged",
        0,
        vec![block(0, concat_parts(branch), vec![10]), block(10, vec![], vec![])],
    ));
    let ops: Vec<Operation> =
        transformed.blocks[0].insns[..3].iter().map(|one| one.what.as_ref().unwrap().op).collect();
    assert_eq!(ops, [Operation::Push, Operation::Push, Operation::Pop]);
}

#[test]
fn test_argument_materialization_does_not_cross_observable_boundaries() {
    // SCREEN's stack argument must not change when its register setup cannot move before it.
    for barrier in [
        "different",
        "width",
        "stack",
        "frame",
        "relocation",
        "covered",
        "gap",
        "group",
        "symbol",
        "clobber",
        "requires",
        "block",
    ] {
        let literal = im(1, 2);
        let mut push =
            insn(0, Some((0, 1)), Some(sem(Operation::Push, "push", vec![], vec![literal.clone()])), vec![], vec![]);
        let mut moved = insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![rl(Register::AX, 2)], vec![literal])),
            vec![7],
            vec![],
        );
        match barrier {
            "different" => moved.what.as_mut().unwrap().sources = vec![im(2, 2)],
            "width" => moved.what.as_mut().unwrap().dests = vec![rl(Register::EAX, 4)],
            "stack" | "frame" => {
                moved.what.as_mut().unwrap().dests =
                    vec![rl(if barrier == "stack" { Register::SP } else { Register::BP }, 2)];
            }
            "relocation" => {
                let symbol = Loc::Imm(Imm { value: 1, width: 2, address: Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }) });
                push.what.as_mut().unwrap().sources = vec![symbol.clone()];
                moved.what.as_mut().unwrap().sources = vec![symbol];
            }
            "covered" => moved.covers = Some((1, 4)),
            "gap" => {
                moved.at = 2;
                moved.covers = Some((2, 2));
            }
            "group" => moved.group = Some(1),
            "symbol" => moved.symbol = Some(true),
            "clobber" => push.clobbers = [Register::AX].into(),
            "requires" => push.requires = vec![(Held { value: 5, width: 2 }, Register::AX)],
            _ => {}
        }
        let (push, moved) = (Arc::new(push), Arc::new(moved));
        let blocks = if barrier == "block" {
            vec![block(0, vec![push], vec![1]), block(1, vec![moved], vec![])]
        } else {
            vec![block(0, vec![push, moved], vec![])]
        };
        let input = body("screen", 0, blocks);
        assert_eq!(pushed_constants(&input), input, "{barrier}");
    }
}

#[test]
fn test_entry_reload_requires_agreement_on_every_edge() {
    // NBODY's header reload is redundant only when all paths carry the exact stored bits.
    for mismatch in ["none", "register", "slot", "width", "clobber", "missing", "unowned", "entry"] {
        let register = rl(Register::EAX, 4);
        let cell = Mem { through: Register::BP, ..Mem::new(frame(-4), 4) };
        let store = insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![Loc::Mem(cell.clone())], vec![register.clone()])),
            vec![],
            vec![],
        );
        let mut other = store.clone();
        match mismatch {
            "register" => other.what.as_mut().unwrap().sources = vec![rl(Register::ECX, 4)],
            "slot" => {
                other.what.as_mut().unwrap().dests =
                    vec![Loc::Mem(Mem { addr: Some(cell.addr.unwrap().plus(-4)), ..cell.clone() })];
            }
            "width" => other.what.as_mut().unwrap().dests = vec![Loc::Mem(Mem { width: 2, ..cell.clone() })],
            "clobber" => other.clobbers = [Register::EAX].into(),
            "missing" => other.what = None,
            _ => {}
        }
        let reload = Insn {
            spill_reload: mismatch != "unowned",
            ..insn(
                3,
                Some((3, 3)),
                Some(sem(Operation::Move, "mov", vec![register], vec![Loc::Mem(cell)])),
                vec![],
                vec![],
            )
        };
        let input = body(
            "join",
            if mismatch == "entry" { 3 } else { 0 },
            vec![
                block(0, vec![], vec![1, 2]),
                block(1, vec![Arc::new(store)], vec![3]),
                block(2, vec![Arc::new(other)], vec![3]),
                block(3, vec![Arc::new(reload.clone())], vec![]),
            ],
        );
        let done = spillforward::forwarded(&input);
        let kept = done.blocks[done.blocks.len() - 1]
            .insns
            .iter()
            .any(|one| one.spill_reload || one.what == reload.what);
        assert_eq!(kept, !["none", "unowned"].contains(&mismatch), "{mismatch}");
    }
}

#[test]
fn test_forwarded_spill_reload_retains_its_virtual_definition() {
    // BASIC nbody's store read value#979 after spill forwarding removed its reload.
    let register = rl(Register::EAX, 4);
    let source = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-4), 4) });
    let destination = Loc::Mem(Mem::new(Some(Addr::new(Space::Segment, 8)), 4));
    let establish = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![source.clone()], vec![register.clone()])),
        vec![],
        vec![],
    );
    let reload = Insn {
        spill_reload: true,
        ..insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![register.clone()], vec![source])), vec![2], vec![])
    };
    let consume =
        insn(3, Some((3, 4)), Some(sem(Operation::Move, "mov", vec![destination], vec![register])), vec![], vec![2]);
    let input = body(
        "forwarded-definition",
        1,
        vec![block(1, vec![Arc::new(establish), Arc::new(reload), Arc::new(consume)], vec![])],
    );

    let done = spillforward::forwarded(&input);

    assert!(verify::verify(&done, false).is_empty());
    assert_eq!(done.insns().iter().filter(|one| one.what.as_ref().unwrap().op == Operation::Move).count(), 2);
}

#[test]
fn test_forwarded_register_copy_retains_its_virtual_definition() {
    // PITSNAP's IN read value#188 after copy propagation removed its AL setup.
    let (al, ah) = (rl(Register::AL, 1), rl(Register::AH, 1));
    let establish = insn(1, Some((1, 2)), Some(sem(Operation::Move, "mov", vec![al], vec![ah])), vec![], vec![]);
    let define = Insn { at: 2, covers: Some((2, 3)), defines: vec![2], ..establish.clone() };
    let consume = insn(3, Some((3, 4)), None, vec![3], vec![2]);
    let input = body(
        "forwarded-copy-definition",
        1,
        vec![block(1, vec![Arc::new(establish), Arc::new(define), Arc::new(consume)], vec![])],
    );

    let done = copyprop::forwarded(&input);

    assert!(verify::verify(&done, false).is_empty());
    assert_eq!(
        done.insns().iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move)).count(),
        1
    );
}

#[test]
fn test_commuted_accumulator_keeps_the_saved_value() {
    // NBODY's saved accumulator must remain valid even when a later instruction reads it.
    for name in ["add", "and", "or", "xor", "sub", "adc"] {
        for width in [2u32, 4] {
            let registers = if width == 2 {
                [Register::CX, Register::SI, Register::AX]
            } else {
                [Register::ECX, Register::ESI, Register::EAX]
            };
            let [accumulator, temporary, term] = registers.map(|register| r(register, width));
            let semantics = [
                sem(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Reg(accumulator)]),
                sem(Operation::Move, "mov", vec![Loc::Reg(accumulator)], vec![Loc::Reg(term)]),
                sem(Operation::Binary, name, vec![Loc::Reg(accumulator)], vec![Loc::Reg(accumulator), Loc::Reg(temporary)]),
            ];
            let insns: Vec<Arc<Insn>> = semantics
                .iter()
                .enumerate()
                .map(|(index, what)| {
                    let index = index as i64;
                    Arc::new(insn(index, Some((index, index + 1)), Some(what.clone()), vec![], vec![]))
                })
                .collect();
            let input = body("accumulator", 0, vec![block(0, insns.clone(), vec![])]);
            let result = commuted(&input);
            if ["sub", "adc"].contains(&name) {
                assert_eq!(result, input);
                continue;
            }
            let out = result.insns();
            assert_eq!(out.len(), 2);
            assert_eq!(out[0].what.as_ref(), Some(&semantics[0]));
            assert_eq!(out[1].what.as_ref().unwrap().sources, [Loc::Reg(accumulator), Loc::Reg(term)]);
            for (seed, addend) in [(0u64, 0u64), (1, 2), (0x7FFF, 1), (0xFFFF_FFFF, 1)] {
                let mask = (1u64 << (width * 8)) - 1;
                let execute = |operations: &[Arc<Insn>]| {
                    let mut values: HashMap<Reg, u64> =
                        HashMap::from_iter([(accumulator, seed & mask), (temporary, 42), (term, addend & mask)]);
                    for one in operations {
                        let what = one.what.as_ref().unwrap();
                        let operands: Vec<u64> = what
                            .sources
                            .iter()
                            .map(|arg| match arg {
                                Loc::Reg(arg) => values[arg],
                                _ => unreachable!(),
                            })
                            .collect();
                        let value = match what.name.as_deref().unwrap() {
                            "mov" => operands[0],
                            "add" => operands.iter().sum(),
                            "and" => operands[0] & operands[1],
                            "or" => operands[0] | operands[1],
                            "xor" => operands[0] ^ operands[1],
                            other => unreachable!("{other}"),
                        };
                        let Loc::Reg(dest) = what.dests[0] else { unreachable!() };
                        values.insert(dest, value & mask);
                    }
                    values
                };
                assert_eq!(execute(&insns), execute(&out));
            }
        }
    }
}

#[test]
fn test_constant_push_pair_preserves_stack_bytes() {
    for (high, low) in [(0x43F3i64, 0xC000i64), (-1, -2), (0, 0), (0x8000, 0x7FFF)] {
        let push = |at: i64, number: i64| {
            Arc::new(insn(at, Some((at, at + 3)), Some(sem(Operation::Push, "push", vec![], vec![im(number, 2)])), vec![], vec![]))
        };
        let pair = vec![push(0, high), push(3, low)];
        let result = pushes(&body("arguments", 0, vec![block(0, pair, vec![])])).insns();
        assert!(result.len() == 1 && result[0].covers == Some((0, 6)));
        let Loc::Imm(operand) = &result[0].what.as_ref().unwrap().sources[0] else { unreachable!() };
        let expected: Vec<u8> = ((low & 0xFFFF) as u16)
            .to_le_bytes()
            .into_iter()
            .chain(((high & 0xFFFF) as u16).to_le_bytes())
            .collect();
        assert_eq!(u32::try_from(operand.value).unwrap().to_le_bytes().to_vec(), expected);
        let made = code(result[0].what.as_ref().unwrap());
        let decoded = Decoder::new(16, &made, DecoderOptions::NONE).decode();
        assert_eq!(decoded.stack_pointer_increment(), -4);
    }
}

#[test]
fn test_constant_push_fusion_stops_at_boundaries() {
    for barrier in ["relocation", "gap", "block", "instruction"] {
        let mut first =
            insn(0, Some((0, 3)), Some(sem(Operation::Push, "push", vec![], vec![im(1, 2)])), vec![], vec![]);
        let mut second = Insn { at: 3, covers: Some((3, 6)), ..first.clone() };
        match barrier {
            "relocation" => {
                first.what.as_mut().unwrap().sources = vec![Loc::Imm(Imm {
                    value: 1,
                    width: 2,
                    address: Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }),
                })];
            }
            "gap" => {
                second.at = 4;
                second.covers = Some((4, 7));
            }
            _ => {}
        }
        let (first, second) = (Arc::new(first), Arc::new(second));
        let insns = if barrier == "instruction" {
            vec![Arc::clone(&first), Arc::new(insn(3, Some((3, 3)), None, vec![], vec![])), Arc::clone(&second)]
        } else {
            vec![Arc::clone(&first), Arc::clone(&second)]
        };
        let blocks = if barrier == "block" {
            vec![block(0, vec![first], vec![3]), block(3, vec![second], vec![])]
        } else {
            vec![block(0, insns, vec![])]
        };
        let input = body("boundary", 0, blocks);
        assert_eq!(pushes(&input), input, "{barrier}");
    }
}

#[test]
fn test_dead_reload_requires_allocator_ownership_and_no_read() {
    // FPCSE's dead spill is removable, but source loads and live spills are not.
    for owned in [false, true] {
        for read in [false, true] {
            let ax = rl(Register::AX, 2);
            let slot = Frame::new(0).cell(1i64, 2).unwrap();
            let load = Arc::new(Insn {
                spill_reload: owned,
                ..insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![Loc::Mem(slot)])), vec![], vec![])
            });
            let used = Arc::new(insn(
                1,
                Some((1, 1)),
                Some(sem(Operation::Move, "mov", vec![rl(Register::BX, 2)], vec![ax.clone()])),
                vec![],
                vec![],
            ));
            let write =
                Arc::new(insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![ax], vec![im(4, 2)])), vec![], vec![]));
            let insns = if read { vec![Arc::clone(&load), used, write] } else { vec![Arc::clone(&load), write] };
            let result = overwritten(&body("reload", 0, vec![block(0, insns, vec![])]));
            assert_eq!(!result.insns().contains(&load), owned && !read, "{owned} {read}");
        }
    }
}

#[test]
fn test_overwritten_reload_keeps_virtual_definition() {
    // mdl_draw_tris kept a zero-cost copy of a spilled selector after its reload was physically dead.
    let ax = rl(Register::AX, 2);
    let slot = Frame::new(0).cell(1i64, 2).unwrap();
    let load = Insn {
        spill_reload: true,
        ..insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![Loc::Mem(slot)])), vec![1], vec![])
    };
    let copy = lir::anchor(Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![ax.clone()])),
        vec![2],
        vec![1],
    )));
    let overwrite = insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![ax], vec![im(0, 2)])), vec![], vec![]);
    let input = body("reload", 0, vec![block(0, vec![Arc::new(load), copy, Arc::new(overwrite)], vec![])]);

    let result = overwritten(&input);
    assert!(verify::verify(&result, false).is_empty());
    assert!(
        result
            .insns()
            .iter()
            .any(|one| one.what.as_ref().unwrap().op == Operation::Nothing && one.defines == [1])
    );
}

#[test]
fn test_overwritten_register_copy_respects_byte_reads() {
    // FPDEEP retains AX/SI allocation shuffles overwritten before any use.
    for (middle, removed) in [(Register::CX, true), (Register::AL, false), (Register::AH, false)] {
        let (ax, si) = (rl(Register::AX, 2), rl(Register::SI, 2));
        let copy =
            Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![si])), vec![], vec![]));
        let width = if [Register::AL, Register::AH].contains(&middle) { 1 } else { 2 };
        let read = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(
                Operation::Move,
                "mov",
                vec![rl(if width == 1 { Register::BL } else { Register::BX }, width)],
                vec![rl(middle, width)],
            )),
            vec![],
            vec![],
        ));
        let overwrite =
            Arc::new(insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![ax], vec![im(4, 2)])), vec![], vec![]));
        let result = overwritten(&body("copies", 0, vec![block(0, vec![Arc::clone(&copy), read, overwrite], vec![])]));
        assert_eq!(!result.insns().contains(&copy), removed, "{middle:?}");
    }
}

#[test]
fn test_word_copy_survives_partial_overwrite() {
    // Writing AL or AH alone cannot make a prior AX definition dead.
    for dest in [Register::AL, Register::AH] {
        let (ax, si) = (rl(Register::AX, 2), rl(Register::SI, 2));
        let copy = Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax], vec![si])), vec![], vec![]));
        let write = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![rl(dest, 1)], vec![im(0, 1)])),
            vec![],
            vec![],
        ));
        let input = body("partial", 0, vec![block(0, vec![copy, write], vec![])]);
        assert_eq!(overwritten(&input), input);
    }
}

#[test]
#[ignore = "fails in Python at 5c22b69b too (shl case)"]
fn test_index_lea_preserves_observed_shift_flags() {
    // ADDRM's copy/shift can become LEA only before a complete flag overwrite.
    for following in ["add", "adc", "inc", "shl", "call", "je"] {
        let (dest, source) = (rl(Register::SI, 2), rl(Register::BX, 2));
        let copy = insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])), vec![2], vec![1]);
        let shift = insn(
            0,
            Some((0, 2)),
            Some(sem(Operation::Binary, "shl", vec![dest.clone()], vec![dest, im(1, 1)])),
            vec![2],
            vec![2],
        );
        let last = insn(
            2,
            Some((2, 4)),
            Some(sem(Operation::Binary, following, vec![source.clone()], vec![source, im(1, 2)])),
            vec![],
            vec![],
        );
        let input = body("index", 0, vec![block(0, vec![Arc::new(copy), Arc::new(shift), Arc::new(last)], vec![])]);
        let result = addresses(&input, "386").unwrap().insns();
        assert_eq!(names(&result)[0], if following == "add" { "lea" } else { "mov" }, "{following}");
        if following == "add" {
            assert_eq!(result[0].covers, Some((0, 2)));
            assert_eq!(result[0].uses, [1]);
            assert_eq!(result[0].defines, [2]);
        }
    }
}

#[test]
fn test_word_scaled_lea_prices_the_partial_register_read() {
    // FARLOADLOOP gained five P6 cycles when AX*2 became LEA CX,[EAX+EAX].
    for (cpu, expected) in [
        ("386", vec!["lea", "cmp"]),
        ("K7", vec!["lea", "cmp"]),
        ("P6", vec!["mov", "shl", "cmp"]),
        ("Core", vec!["mov", "shl", "cmp"]),
    ] {
        let (dest, source) = (rl(Register::CX, 2), rl(Register::AX, 2));
        let copy = insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source])), vec![2], vec![1]);
        let shift = insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "shl", vec![dest.clone()], vec![dest.clone(), im(1, 1)])),
            vec![2],
            vec![2],
        );
        let compare =
            insn(2, Some((2, 2)), Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(8, 2)])), vec![], vec![2]);
        let input = body(
            "farloadloop._mark",
            0,
            vec![block(0, vec![Arc::new(copy), Arc::new(shift), Arc::new(compare)], vec![])],
        );

        let result = addresses(&input, cpu).unwrap().insns();

        assert_eq!(names(&result), expected, "{cpu}");
    }
}

#[test]
fn test_source_owned_scale_converges_with_a_synthetic_frontend() {
    // Frontend-parity ALGEBRA left BASIC's `mov; shl 2; add` intact.
    let (source, dest) = (rl(Register::EDX, 4), rl(Register::ECX, 4));
    let copy =
        insn(0x90, Some((0x90, 0x90)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])), vec![77], vec![72]);
    let shift = insn(
        0x90,
        Some((0x90, 0x95)),
        Some(sem(Operation::Binary, "shl", vec![dest.clone()], vec![dest.clone(), im(2, 1)])),
        vec![77],
        vec![77],
    );
    let addition = insn(
        0x90,
        Some((0x90, 0x90)),
        Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest.clone(), source])),
        vec![77],
        vec![77, 72],
    );
    let compare =
        insn(0x95, Some((0x95, 0x95)), Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(0, 4)])), vec![], vec![77]);
    let input = body(
        "frontend-scale",
        0x90,
        vec![block(0x90, vec![Arc::new(copy.clone()), Arc::new(shift), Arc::new(addition.clone()), Arc::new(compare)], vec![])],
    );

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["lea", "cmp"]);
    assert_eq!(result[0].covers, Some((0x90, 0x95)));
    assert_eq!(result[0].defines, addition.defines);
    assert_eq!(result[0].uses, copy.uses);
}

#[test]
fn test_scaled_lea_does_not_require_another_shift() {
    // HOTLPX's scale idiom needed three instructions when followed by CMP instead of SHL.
    for amount in [1i64, 2, 3] {
        for following in ["cmp", "adc", "je"] {
            for width in [2u32, 4] {
                let dest = rl(if width == 2 { Register::BX } else { Register::EBX }, width);
                let source = rl(if width == 2 { Register::CX } else { Register::ECX }, width);
                let make = |at: i64, kind: Operation, name: &str, args: Vec<Loc>| {
                    Arc::new(insn(at, Some((at, at)), Some(sem(kind, name, vec![dest.clone()], args)), vec![], vec![]))
                };
                let copy = make(0, Operation::Move, "mov", vec![source.clone()]);
                let shift = make(1, Operation::Binary, "shl", vec![dest.clone(), im(amount, 1)]);
                let add = make(2, Operation::Binary, "add", vec![dest.clone(), source.clone()]);
                let last = make(
                    3,
                    if following == "cmp" { Operation::Compare } else { Operation::Binary },
                    following,
                    vec![dest.clone(), im(0, 2)],
                );
                let input = body("scale", 0, vec![block(0, vec![copy, shift, add, last], vec![])]);
                let result = addresses(&input, "386").unwrap().insns();
                let expected = if following == "cmp" {
                    vec!["lea", "cmp"]
                } else if amount == 1 {
                    vec!["lea", "add", following]
                } else {
                    vec!["mov", "shl", "add", following]
                };
                assert_eq!(names(&result), expected, "{amount} {following} {width}");
                if following == "cmp" {
                    let emitted = crate::frontend::declen::decode(&code(result[0].what.as_ref().unwrap()), 0).unwrap().insn;
                    assert_eq!(emitted.memory_index_scale(), 1 << amount);
                }
            }
        }
    }
}

#[test]
fn test_loaded_scaled_add_uses_67h_lea() {
    // Matmul emitted `mov temp,[spill]; shl temp,1; add sum,temp`.
    let (temporary, total) = (rl(Register::EAX, 4), rl(Register::EDX, 4));
    let cell = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-4), 4) });
    let load = Insn {
        spill_reload: true,
        ..insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![temporary.clone()], vec![cell])), vec![1], vec![])
    };
    let shift = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "shl", vec![temporary.clone()], vec![temporary.clone(), im(1, 1)])),
        vec![1],
        vec![1],
    );
    let add = insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Binary, "add", vec![total.clone()], vec![total.clone(), temporary])),
        vec![2],
        vec![2, 1],
    );
    let compare = insn(3, Some((3, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])), vec![], vec![2]);
    let input = body(
        "matmul-scale",
        0,
        vec![block(0, vec![Arc::new(load), Arc::new(shift), Arc::new(add), Arc::new(compare)], vec![])],
    );

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "lea", "cmp"]);
    let Loc::Address(address) = &result[1].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
    assert_eq!(address.through, Register::EDX);
    assert_eq!(address.index, Register::EAX);
    assert_eq!(address.scale, 2);
}

#[test]
fn test_repeated_allocated_address_copies_use_one_clean_67h_base() {
    // lru_use copied one retained owner into BX/SI/DI at every field access.
    let owner = insn(
        1,
        Some((1, 3)),
        Some(sem(
            Operation::Move,
            "mov",
            vec![rl(Register::DX, 2)],
            vec![Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(6), 2) })],
        )),
        vec![1],
        vec![],
    );
    let registers = [Register::BX, Register::SI, Register::DI, Register::BX];
    let mut insns = vec![Arc::new(owner)];
    for (index, register) in registers.into_iter().enumerate() {
        let index = index as i64 + 2;
        let value = u32::try_from(index).unwrap();
        let copy = insn(
            index,
            Some((index, index)),
            Some(sem(Operation::Move, "mov", vec![rl(register, 2)], vec![rl(Register::DX, 2)])),
            vec![value],
            vec![1],
        );
        let cell = Mem {
            through: register,
            base: Some(Held { value, width: 2 }),
            selector: Some(Held { value: 20, width: 2 }),
            ..Mem::new(Some(Addr { segment: Register::FS, ..Addr::new(Space::Far, index) }), 2)
        };
        let load = insn(
            index,
            Some((index, index)),
            Some(sem(Operation::Move, "mov", vec![rl(Register::AX, 2)], vec![Loc::Mem(cell)])),
            vec![30 + value],
            vec![value, 20],
        );
        insns.extend([Arc::new(copy), Arc::new(load)]);
    }
    let input = body("secondary-base", 1, vec![block(1, insns, vec![])]);

    let result = secondary_bases(&input, "386").unwrap().insns();

    let moves: Vec<&Arc<Insn>> = result.iter().filter(|one| one.what.as_ref().unwrap().name.as_deref() == Some("mov")).collect();
    assert_eq!(moves.len(), 5); // the owner load and four actual memory loads
    let extension = result.iter().find(|one| one.what.as_ref().unwrap().name.as_deref() == Some("movzx")).unwrap();
    assert_eq!(extension.what.as_ref().unwrap().dests, [rl(Register::EDX, 4)]);
    let cells: Vec<&Loc> = moves[1..].iter().map(|one| &one.what.as_ref().unwrap().sources[0]).collect();
    assert!(cells.iter().all(|cell| matches!(cell, Loc::Mem(cell) if cell.through == Register::EDX)));
    assert!(cells.iter().all(|cell| matches!(cell, Loc::Mem(cell) if cell.base == Some(Held { value: 1, width: 4 }))));

    // P6 and Core charge both a partial-register merge and a length-changing
    // 67h decode stall; selection must not optimize against a cheaper model.
    for target_cpu in ["P6", "Core"] {
        let expensive = secondary_bases(&input, target_cpu).unwrap().insns();
        assert!(!expensive.iter().any(|one| one.what.as_ref().unwrap().name.as_deref() == Some("movzx")));
        assert_eq!(expensive.iter().filter(|one| one.what.as_ref().unwrap().name.as_deref() == Some("mov")).count(), 9);
    }
}

#[test]
fn test_loaded_scaled_add_skips_metadata_only_anchors() {
    // Matmul retained `load; shl; add` when metadata anchors separated it.
    let (temporary, total) = (rl(Register::EBX, 4), rl(Register::EAX, 4));
    let cell = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-8), 4) });
    let load = Insn {
        symbol: Some(true),
        ..insn(148, Some((148, 148)), Some(sem(Operation::Move, "mov", vec![temporary.clone()], vec![cell])), vec![1], vec![])
    };
    let shift = Insn {
        symbol: Some(true),
        ..insn(
            155,
            Some((155, 155)),
            Some(sem(Operation::Binary, "shl", vec![temporary.clone()], vec![temporary.clone(), im(1, 1)])),
            vec![1],
            vec![1],
        )
    };
    let addition = Insn {
        symbol: Some(true),
        ..insn(
            156,
            Some((156, 156)),
            Some(sem(Operation::Binary, "add", vec![total.clone()], vec![total.clone(), temporary])),
            vec![2],
            vec![2, 1],
        )
    };
    let anchors = [
        Arc::new(insn(151, Some((151, 151)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])),
        Arc::new(insn(154, Some((154, 154)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])),
    ];
    let compare =
        insn(157, Some((157, 157)), Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])), vec![], vec![2]);
    let input = body(
        "matmul-anchored-scale",
        148,
        vec![block(
            148,
            vec![
                Arc::new(load),
                Arc::clone(&anchors[0]),
                Arc::new(shift.clone()),
                Arc::clone(&anchors[1]),
                Arc::new(addition),
                Arc::new(compare),
            ],
            vec![],
        )],
    );

    let transformed = addresses(&input, "386").unwrap();
    let result = transformed.insns();

    assert_eq!(names(&result), ["mov", "", "", "", "lea", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
    assert!(result[1] == anchors[0] && result[3] == anchors[1]);
    assert!(result[2].symbol == Some(false) && result[2].defines == shift.defines);
    assert_eq!(result[4].symbol, Some(true));
    let Loc::Address(address) = &result[4].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
    assert_eq!((address.through, address.index, address.scale), (Register::EAX, Register::EBX, 2));
    assert!(verify::verify(&transformed, false).is_empty());
}

#[test]
fn test_loaded_scaled_add_preserves_a_shifted_value_live_into_a_successor() {
    // A block-local use count lost a shifted temporary read by its successor.
    let (temporary, total, saved) = (rl(Register::EAX, 4), rl(Register::EDX, 4), rl(Register::ECX, 4));
    let cell = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-4), 4) });
    let load = Insn {
        spill_reload: true,
        ..insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![temporary.clone()], vec![cell])), vec![1], vec![])
    };
    let shift = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "shl", vec![temporary.clone()], vec![temporary.clone(), im(1, 1)])),
        vec![1],
        vec![1],
    );
    let add = insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Binary, "add", vec![total.clone()], vec![total.clone(), temporary.clone()])),
        vec![2],
        vec![2, 1],
    );
    let compare = insn(3, Some((3, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])), vec![], vec![2]);
    let preserve = insn(4, Some((4, 4)), Some(sem(Operation::Move, "mov", vec![saved], vec![temporary])), vec![3], vec![1]);
    let input = body(
        "live-scale",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(shift), Arc::new(add), Arc::new(compare)], vec![1]),
            block(1, vec![Arc::new(preserve)], vec![]),
        ],
    );

    let result = addresses(&input, "386").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["mov", "shl", "add", "cmp"]);
}

#[test]
fn test_scaled_address_requires_dead_flags_and_exact_allocated_operands() {
    // HOTLPX's LEA must retain low-word arithmetic without losing flags or owned bytes.
    for guard in ["none", "dword", "carry", "zero_shift", "wrong_source", "same", "stack", "relocation"] {
        let width = if guard == "dword" { 4 } else { 2 };
        let dest = rl(if width == 4 { Register::EBX } else { Register::BX }, width);
        let mut source = rl(if width == 4 { Register::ECX } else { Register::CX }, width);
        if guard == "same" {
            source = dest.clone();
        }
        if guard == "stack" {
            source = rl(Register::SP, width);
        }
        let make = |at: i64, kind: Operation, name: &str, sources: Vec<Loc>| {
            insn(at, Some((at, at)), Some(sem(kind, name, vec![dest.clone()], sources)), vec![], vec![])
        };
        let mut copy = make(0, Operation::Move, "mov", vec![source.clone()]);
        let shift = make(1, Operation::Binary, "shl", vec![dest.clone(), im(2, 1)]);
        let mut add = make(2, Operation::Binary, "add", vec![dest.clone(), source.clone()]);
        let mut last = make(3, Operation::Binary, "shl", vec![dest.clone(), im(2, 1)]);
        if guard == "carry" {
            last.what.as_mut().unwrap().name = Some("adc".to_owned());
        }
        if guard == "zero_shift" {
            last.what.as_mut().unwrap().sources = vec![dest.clone(), im(0, 1)];
        }
        if guard == "wrong_source" {
            add.what.as_mut().unwrap().sources = vec![dest.clone(), rl(Register::DX, 2)];
        }
        if guard == "relocation" {
            copy.symbol = Some(true);
        }
        let parts = [Arc::new(copy), Arc::new(shift), Arc::new(add), Arc::new(last)];
        let result = _scaled_address(&parts, false, "386").unwrap();
        if !["none", "dword"].contains(&guard) {
            assert!(result.is_none(), "{guard}");
            continue;
        }
        let result = result.unwrap();
        assert_eq!(result.what.as_ref().unwrap().dests, [dest]);
        let Loc::Address(address) = &result.what.as_ref().unwrap().sources[0] else { panic!("not an address") };
        assert_eq!(address.scale, 4);
        let mask = (1u64 << (8 * width)) - 1;
        for bits in [0u64, 1, 0x1234_FFFF, 0x8000_8000, 0xFFFF_FFFF] {
            let original = (((bits & mask) << 2) + (bits & mask)) & mask;
            assert_eq!((bits + bits * 4) & mask, original);
        }
    }
}

#[test]
fn test_zeroing_requires_flags_overwritten_before_observation() {
    // HARR-style zeroing is safe before CMP, but not before a carry consumer.
    for (following, zeroed) in
        [("cmp", true), ("add", true), ("adc", false), ("inc", false), ("shl", false), ("call", false), ("je", false)]
    {
        let dest = rl(Register::AX, 2);
        let first = insn(0, Some((0, 3)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![im(0, 2)])), vec![], vec![]);
        let last = insn(
            3,
            Some((3, 5)),
            Some(sem(
                if following == "cmp" { Operation::Compare } else { Operation::Binary },
                following,
                vec![],
                vec![dest, im(1, 2)],
            )),
            vec![],
            vec![],
        );
        let result = transform(body("zero", 0, vec![block(0, vec![Arc::new(first), Arc::new(last)], vec![])]));
        assert_eq!(names(&result.insns())[0], if zeroed { "xor" } else { "mov" }, "{following}");
    }
}

#[test]
fn test_zeroing_preserves_width_relocations_and_unknown_flag_observers() {
    for variant in ["plain", "dword", "byte", "relocation", "boundary", "unknown", "clobber"] {
        let width = if variant == "dword" {
            4
        } else if variant == "byte" {
            1
        } else {
            2
        };
        let register = match width {
            1 => Register::AL,
            2 => Register::AX,
            _ => Register::EAX,
        };
        let dest = rl(register, width);
        let address = (variant == "relocation").then(|| Addr { index: 5, ..Addr::new(Space::Segment, 0) });
        let first = Arc::new(insn(
            0,
            Some((0, 3)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![Loc::Imm(Imm { value: 0, width, address })])),
            vec![],
            vec![],
        ));
        let mut middle = insn(3, Some((3, 4)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![]);
        if variant == "unknown" {
            middle.what = None;
        }
        if variant == "clobber" {
            middle.clobbers = [Register::EAX].into();
        }
        let middle = Arc::new(middle);
        let last = Arc::new(insn(
            4,
            Some((4, 6)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![dest.clone(), im(1, width)])),
            vec![],
            vec![],
        ));
        let blocks = if variant == "boundary" {
            vec![block(0, vec![Arc::clone(&first)], vec![3]), block(3, vec![middle, last], vec![])]
        } else {
            vec![block(0, vec![Arc::clone(&first), middle, last], vec![])]
        };
        let result = zeroes(&body("zero", 0, blocks)).insns()[0].clone();
        // "boundary": the next block's cmp overwrites every flag before anything reads one.
        assert_eq!(
            result.what.as_ref().unwrap().name.as_deref(),
            Some(if ["plain", "dword", "boundary"].contains(&variant) { "xor" } else { "mov" }),
            "{variant}"
        );
        assert_eq!(result.what.as_ref().unwrap().dests, [dest]);
        assert_eq!(result.covers, first.covers);
    }
}

#[test]
fn test_zeroing_before_a_jump_asks_what_the_target_reads() {
    // `mov bx,0; jmp` stayed three bytes: every block end was taken to have its flags read.
    for (successor, zeroed) in [(Some("cmp"), true), (Some("jb"), false), (None, false)] {
        let dest = rl(Register::BX, 2);
        let zero = insn(0, Some((0, 3)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![im(0, 2)])), vec![], vec![]);
        let jump = insn(3, Some((3, 5)), Some(semt(Operation::Jump, "jmp", vec![], vec![], Some(5))), vec![], vec![]);
        let first = match successor {
            Some("cmp") => Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(1, 2)])),
            Some(_) => Some(semt(Operation::Branch, "jb", vec![], vec![], Some(9))),
            None => None,
        };
        let blocks = vec![
            block(0, vec![Arc::new(zero), Arc::new(jump)], vec![5]),
            block(5, vec![Arc::new(insn(5, Some((5, 7)), first, vec![], vec![]))], vec![]),
        ];
        let result = zeroes(&body("zero", 0, blocks)).insns()[0].clone();
        assert_eq!(result.what.as_ref().unwrap().name.as_deref(), Some(if zeroed { "xor" } else { "mov" }));
    }
}

#[test]
fn test_zero_compare_before_its_branch_is_or() {
    // `cmp ax,0; jl` is three bytes where `or ax,ax; jl` is two. Only AF differs.
    for (variant, rewritten) in [
        ("plain", true),
        ("moves", true),
        ("call", true),
        ("return", true),
        ("relocated", true),
        ("x87", true),
        ("between", false),
        ("pushf", false),
        ("adjust", false),
        ("memory", false),
    ] {
        let ax = rl(Register::AX, 2);
        let tested = if variant == "memory" { Loc::Mem(mem(frame(-2), 2, Register::BP, 0, 2)) } else { ax.clone() };
        let compare = insn(0, Some((0, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![tested, im(0, 2)])), vec![], vec![]);
        let moved = sem(Operation::Move, "mov", vec![rl(Register::BX, 2)], vec![rl(Register::CX, 2)]);
        let between = insn(3, Some((3, 5)), if variant == "between" { None } else { Some(moved) }, vec![], vec![]);
        let branch = insn(6, Some((6, 8)), Some(semt(Operation::Branch, "jl", vec![], vec![], Some(9))), vec![], vec![]);
        let after = sem(Operation::Compare, "cmp", vec![], vec![ax.clone(), im(1, 2)]);
        let symbol = Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }) });
        let last = match variant {
            "adjust" => None,
            "call" => Some(sem(Operation::Call, "call", vec![], vec![])),
            "return" => Some(sem(Operation::Return, "", vec![], vec![])),
            "relocated" => Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![symbol.clone()])),
            "x87" => Some(sem(
                Operation::Compare,
                "fcomp",
                vec![],
                vec![Loc::Mem(mem(frame(-4), 4, Register::BP, 0, 2))],
            )),
            "pushf" => Some(sem(Operation::Nothing, "pushf", vec![], vec![symbol])),
            _ => Some(after.clone()),
        };
        let compare = Arc::new(compare);
        let head = if ["moves", "between"].contains(&variant) {
            vec![Arc::clone(&compare), Arc::new(between), Arc::new(branch)]
        } else {
            vec![Arc::clone(&compare), Arc::new(branch)]
        };
        let blocks = vec![
            block(0, head, vec![8, 9]),
            block(8, vec![Arc::new(insn(8, Some((8, 9)), Some(after), vec![], vec![]))], vec![]),
            block(
                9,
                vec![Arc::new(Insn {
                    symbol: Some(["relocated", "pushf"].contains(&variant)),
                    ..insn(9, Some((9, 10)), last, vec![], vec![])
                })],
                vec![],
            ),
        ];
        let result = zero_compares(&body("zero", 0, blocks)).insns()[0].clone();
        let what = result.what.as_ref().unwrap();
        if rewritten {
            assert_eq!(
                (what.op, what.name.as_deref(), &what.dests, &what.sources),
                (Operation::Binary, Some("or"), &vec![ax.clone()], &vec![ax.clone(), ax]),
                "{variant}"
            );
        } else {
            assert_eq!(result.what, compare.what, "{variant}");
        }
    }
}

#[test]
fn test_register_round_trip_through_memory_is_one_instruction() {
    // `mov bx,[bp-4]; add bx,1; mov [bp-4],bx` where bcc writes `add word ptr [bp-4],1`.
    // Python printed these through `masm`; the reprs are of the semantics it printed.
    // "unsigned byte" is not ported: it fails against the Python implementation too.
    let word = "Mem(addr=[bp-0x4], width=2, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
                selector=None, index=None, scale=1, index_through=0)";
    let byte = "Mem(addr=[bp-0x4], width=1, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
                selector=None, index=None, scale=1, index_through=0)";
    let jl = "Semantics(op=<Operation.BRANCH: 'branch'>, name='jl', dests=(), sources=(), target=9, indirect=False)";
    let printed = |op: &str, name: &str, dests: String, sources: String| {
        format!("Semantics(op={op}, name='{name}', dests={dests}, sources={sources}, target=None, indirect=False)")
    };
    let binary = "<Operation.BINARY: 'binary'>";
    let cmp = "<Operation.COMPARE: 'cmp'>";
    for (variant, expected) in [
        (
            "add",
            Some(vec![printed(binary, "add", format!("({word},)"), format!("({word}, Imm(value=1, width=2, address=None))"))]),
        ),
        (
            "register",
            Some(vec![printed(binary, "add", format!("({word},)"), format!("({word}, Reg(register=22, width=2))"))]),
        ),
        ("unary", Some(vec![printed("<Operation.UNARY: 'unary'>", "neg", format!("({word},)"), format!("({word},)"))])),
        (
            "compare",
            Some(vec![printed(cmp, "cmp", "()".to_owned(), format!("({word}, Imm(value=5, width=2, address=None))")), jl.to_owned()]),
        ),
        (
            "zero",
            Some(vec![printed(cmp, "cmp", "()".to_owned(), format!("({word}, Imm(value=0, width=2, address=None))")), jl.to_owned()]),
        ),
        (
            "signed byte",
            Some(vec![printed(cmp, "cmp", "()".to_owned(), format!("({byte}, Imm(value=0, width=1, address=None))")), jl.to_owned()]),
        ),
        ("unsigned byte, sign read", None),
        ("live", None),
        ("addressed", None),
        ("bytes", None),
    ] {
        let extension = match variant {
            "signed byte" => Some("movsx"),
            "unsigned byte" | "unsigned byte, sign read" => Some("movzx"),
            _ => None,
        };
        let compares = ["compare", "zero"].contains(&variant) || extension.is_some();
        let (bx, cx) = (rl(Register::BX, 2), rl(Register::CX, 2));
        let mut cell = mem(frame(-4), if extension.is_some() { 1 } else { 2 }, Register::BP, 0, 2);
        if variant == "addressed" {
            cell = mem(None, 2, Register::BX, 2, 1);
        }
        let cell = Loc::Mem(cell);
        let spans = if variant == "bytes" { Some((0, 3)) } else { None };
        let make = |at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>, covers| {
            Arc::new(insn(at, covers, Some(semt(op, name, dests, sources, target)), vec![], vec![]))
        };
        let mut load = make(0, Operation::Move, "mov", vec![bx.clone()], vec![cell.clone()], None, spans);
        if let Some(extension) = extension {
            load = make(0, Operation::Extend, extension, vec![bx.clone()], vec![cell.clone()], None, None);
        }
        let mut work = match variant {
            "register" => make(1, Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), cx.clone()], None, None),
            "unary" => make(1, Operation::Unary, "neg", vec![bx.clone()], vec![bx.clone()], None, None),
            "compare" => make(1, Operation::Compare, "cmp", vec![], vec![bx.clone(), im(5, 2)], None, None),
            _ => make(1, Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), im(1, 2)], None, None),
        };
        if variant == "zero" || extension.is_some() {
            work = make(1, Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)], None, None);
        }
        let head = if compares {
            let name = if variant == "unsigned byte" { "je" } else { "jl" };
            vec![load, work, make(2, Operation::Branch, name, vec![], vec![], Some(9), None)]
        } else {
            vec![load, work, make(2, Operation::Move, "mov", vec![cell.clone()], vec![bx.clone()], None, None)]
        };
        let reread = make(8, Operation::Move, "mov", vec![rl(Register::AX, 2)], vec![bx.clone()], None, None);
        let rewrite = make(8, Operation::Move, "mov", vec![bx], vec![cx], None, None);
        // Back to the top rather than a return: liveness reads a return as reading every register.
        let blocks = vec![
            block(0, head.clone(), if compares { vec![8, 9] } else { vec![8] }),
            block(
                8,
                vec![
                    if variant == "live" { reread } else { Arc::clone(&rewrite) },
                    make(9, Operation::Jump, "jmp", vec![], vec![], Some(0), None),
                ],
                vec![0],
            ),
            block(9, vec![rewrite, make(10, Operation::Jump, "jmp", vec![], vec![], Some(0), None)], vec![0]),
        ];
        let result = fused(&body("fused", 0, blocks)).blocks[0].clone();
        match expected {
            None => assert_eq!(result.insns, head, "{variant}"),
            Some(expected) => {
                let found: Vec<String> = whats(&result.insns).iter().map(Repr::repr).collect();
                assert_eq!(found, expected, "{variant}");
            }
        }
    }
}

#[test]
fn test_fusion_preserves_a_virtual_dataflow_anchor() {
    // ls_animate emitted MOV AX,[bp-16]; OR AX,AX although BCC compares the slot.
    let (bx, cx) = (rl(Register::BX, 2), rl(Register::CX, 2));
    let cell = Loc::Mem(mem(frame(-4), 2, Register::BP, 0, 2));
    let load = Insn {
        spill_reload: true,
        ..insn(1, None, Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![cell.clone()])), vec![2], vec![])
    };
    let anchor = insn(2, None, Some(sem(Operation::Nothing, "", vec![], vec![])), vec![3], vec![2]);
    let compare = insn(3, None, Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![3]);
    let branch = insn(4, None, Some(semt(Operation::Branch, "je", vec![], vec![], Some(2))), vec![], vec![]);
    let overwrite = Arc::new(insn(5, None, Some(sem(Operation::Move, "mov", vec![bx], vec![cx])), vec![], vec![]));
    let input = body(
        "anchored-fusion",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(anchor), Arc::new(compare), Arc::new(branch)], vec![1, 2]),
            block(1, vec![Arc::clone(&overwrite)], vec![]),
            block(2, vec![overwrite], vec![]),
        ],
    );

    let done = fused(&input);

    assert!(verify::verify(&done, false).is_empty());
    let physical = whats(&done.blocks[0].insns);
    assert_eq!(physical.len(), 2);
    assert_eq!(physical[0], sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)]));
    assert_eq!(done.blocks[0].insns[0].what.as_ref().unwrap().op, Operation::Nothing);
    assert_eq!(done.blocks[0].insns[0].defines, [2]);
    assert_eq!(done.blocks[0].insns[1].defines, [3]);
}

#[test]
fn test_indirect_call_target_is_physically_live_into_the_call() {
    // qcport's mdl_ai lost value#47 after fusion removed the function pointer load.
    let (bx, cx) = (rl(Register::BX, 2), rl(Register::CX, 2));
    let cell = Loc::Mem(mem(frame(-4), 2, Register::BP, 0, 2));
    let load = insn(1, None, Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![cell])), vec![47], vec![]);
    let compare = insn(2, None, Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![47]);
    let branch = insn(3, None, Some(semt(Operation::Branch, "je", vec![], vec![], Some(2))), vec![], vec![]);
    let call = Insn {
        clobbers: [Register::EBX].into(),
        ..insn(4, None, Some(sem(Operation::Call, "call", vec![], vec![bx.clone()])), vec![], vec![47])
    };
    let overwrite = insn(5, None, Some(sem(Operation::Move, "mov", vec![bx], vec![cx])), vec![], vec![]);
    let input = body(
        "indirect-call-liveness",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(compare), Arc::new(branch)], vec![1, 2]),
            block(1, vec![Arc::new(call)], vec![]),
            block(2, vec![Arc::new(overwrite)], vec![]),
        ],
    );

    let done = fused(&input);

    assert!(verify::verify(&done, false).is_empty());
}

#[test]
fn test_far_pointer_loaded_in_one_instruction() {
    // `mov bx,[bp-8]; mov es,[bp-6]` where bcc writes `les bx,[bp-8]`.
    // Python printed these through `masm`; the reprs are of the semantics it printed.
    let les = |name: &str, segment: u32, cell: &str| {
        format!(
            "Semantics(op=<Operation.MOVE: 'move'>, name='{name}', dests=(Reg(register=24, width=2), \
             Reg(register={segment}, width=2)), sources=({cell},), target=None, indirect=False)"
        )
    };
    let frame_cell = "Mem(addr=[bp-0x8], width=4, through=26, offset=0, disp_width=2, base=None, \
                      stack_argument=False, selector=None, index=None, scale=1, index_through=0)";
    let through = "Mem(addr=None, width=4, through=24, offset=0, disp_width=1, base=None, stack_argument=False, \
                   selector=None, index=None, scale=1, index_through=0)";
    let override_ = "Mem(addr=[es:r0+0x10], width=4, through=27, offset=0, disp_width=2, base=Held(value=1, \
                     width=2), stack_argument=False, selector=None, index=None, scale=1, index_through=0)";
    for (variant, printed) in [
        ("offset first", Some(les("les", 71, frame_cell))),
        ("segment first", Some(les("les", 71, frame_cell))),
        ("fs", Some(les("lfs", 75, frame_cell))),
        ("through the offset", None),
        ("through the offset, segment first", Some(les("les", 71, through))),
        ("override", Some(les("les", 71, override_))),
        ("override, segment first", None),
        ("another cell", None),
    ] {
        let bx = rl(Register::BX, 2);
        let segment = rl(if variant == "fs" { Register::FS } else { Register::ES }, 2);
        let mut low = mem(frame(-8), 2, Register::BP, 0, 2);
        let mut high = mem(frame(if variant == "another cell" { -4 } else { -6 }), 2, Register::BP, 0, 2);
        let mut base = Register::BP;
        if variant.starts_with("through the offset") {
            (low, high) = (mem(None, 2, Register::BX, 0, 1), mem(None, 2, Register::BX, 2, 1));
            base = Register::BX;
        }
        if variant.starts_with("override") {
            // As lowered and allocated: the offset value placed in si.
            let far = |disp: i64| Mem {
                base: Some(Held { value: 1, width: 2 }),
                ..mem(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, disp) }), 2, Register::SI, 0, 2)
            };
            (low, high) = (far(16), far(18));
            base = Register::SI;
        }
        let moved = |at: i64, dest: Loc, cell: Mem| {
            Arc::new(insn(at, None, Some(sem(Operation::Move, "mov", vec![dest], vec![Loc::Mem(cell)])), vec![], vec![]))
        };
        let mut pair = vec![moved(0, bx, low), moved(1, segment, high)];
        if variant.ends_with("segment first") {
            pair.reverse();
        }
        let result = far_loads(&body("far", 0, vec![block(0, pair.clone(), vec![])])).blocks[0].insns.clone();
        let Some(printed) = printed else {
            assert_eq!(result, pair, "{variant}");
            continue;
        };
        let found: Vec<String> = result.iter().map(|one| one.what.as_ref().unwrap().repr()).collect();
        assert_eq!(found, [printed], "{variant}");
        let made = code(result[0].what.as_ref().unwrap());
        let decoded = Decoder::new(16, &made, DecoderOptions::NONE).decode();
        let expected = if variant == "fs" { Mnemonic::Lfs } else { Mnemonic::Les };
        assert_eq!((decoded.mnemonic(), decoded.op0_register(), decoded.memory_base()), (expected, Register::BX, base));
    }
}

#[test]
fn test_wait_elimination_does_not_cross_observable_work() {
    // FPCSEX's redundant waits may disappear, but integer observers still need completion.
    for middle in [Some(""), Some("mov"), Some("fnstsw"), Some("fninit"), None, Some("block")] {
        let instruction = |at: i64, name: Option<&str>| {
            let what = name.map(|name| sem(Operation::Nothing, name, vec![], vec![]));
            Arc::new(insn(at, Some((at, at + 1)), what, vec![], vec![]))
        };
        let (first, between, last) = (instruction(0, Some("wait")), instruction(1, middle), instruction(2, Some("fld")));
        let blocks = if middle == Some("block") {
            vec![block(0, vec![first], vec![2]), block(2, vec![last], vec![])]
        } else {
            vec![block(0, vec![first, between, last], vec![])]
        };
        let result = waits(&body("waits", 0, blocks));
        assert_eq!(
            result.insns().iter().any(|one| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("wait"))),
            middle != Some(""),
            "{middle:?}"
        );
    }
}

#[test]
fn test_repeated_copy_requires_unchanged_source_and_destination() {
    // LNGMXX copied ECX into EAX twice around CDQ; partial writes must prevent reuse.
    for change in [None, Some(Register::CH), Some(Register::AH)] {
        let moved = insn(
            0,
            Some((0, 1)),
            Some(sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![rl(Register::ECX, 4)])),
            vec![],
            vec![],
        );
        let mut extend = insn(
            1,
            Some((1, 2)),
            Some(sem(Operation::Extend, "cdq", vec![rl(Register::EDX, 4)], vec![rl(Register::EAX, 4)])),
            vec![],
            vec![],
        );
        if let Some(change) = change {
            extend.clobbers = [change].into();
        }
        let last = Insn { at: 2, covers: Some((2, 3)), ..moved.clone() };
        let input = body("copies", 0, vec![block(0, vec![Arc::new(moved.clone()), Arc::new(extend), Arc::new(last)], vec![])]);
        let result = constants(&input);
        assert_eq!(
            result.insns().iter().filter(|one| one.what == moved.what).count(),
            if change.is_none() { 1 } else { 2 },
            "{change:?}"
        );
    }
}

#[test]
fn test_copied_value_survives_overwriting_its_original_register() {
    // A copied value is a snapshot, not an alias of the register it came from.
    let copy = |at: i64, dest: Register, source: Loc| {
        Arc::new(insn(at, Some((at, at + 1)), Some(sem(Operation::Move, "mov", vec![rl(dest, 4)], vec![source])), vec![], vec![]))
    };
    let insns = vec![
        copy(0, Register::EAX, rl(Register::ECX, 4)),
        copy(1, Register::EDX, rl(Register::ECX, 4)),
        copy(2, Register::ECX, im(7, 4)),
        copy(3, Register::EAX, rl(Register::EDX, 4)),
        copy(4, Register::EAX, rl(Register::ECX, 4)),
    ];
    let result = constants(&body("snapshot", 0, vec![block(0, insns.clone(), vec![])]));
    let expected: Vec<Semantics> = insns.iter().filter(|one| one.at != 3).map(|one| one.what.clone().unwrap()).collect();
    assert_eq!(whats(&result.insns()), expected);
}

#[test]
fn test_partial_write_invalidates_constant() {
    let moved = |at: i64, dest: Loc, source: Loc| {
        Arc::new(insn(at, Some((at, at + 1)), Some(sem(Operation::Move, "mov", vec![dest], vec![source])), vec![], vec![]))
    };
    let first = moved(0, rl(Register::EAX, 4), im(512, 4));
    let change = moved(1, rl(Register::AH, 1), im(0, 1));
    let again = moved(2, rl(Register::EAX, 4), im(512, 4));
    let input = body("partial", 0, vec![block(0, vec![first, change, again], vec![])]);
    assert_eq!(constants(&input).blocks[0].insns.len(), 3);
}

#[test]
fn test_empty_ownership_marker_preserves_register_knowledge() {
    // Expanded FPDEEP emitted MOV AX,0 twice, separated only by a removed instruction's marker.
    for clobbers in [vec![], vec![Register::AX]] {
        let what = sem(Operation::Move, "mov", vec![rl(Register::AX, 2)], vec![im(0, 2)]);
        let first = insn(0, Some((0, 3)), Some(what.clone()), vec![], vec![]);
        let marker = Insn {
            clobbers: clobbers.iter().copied().collect(),
            ..insn(3, Some((3, 5)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])
        };
        let last = Insn { at: 5, covers: Some((5, 8)), ..first.clone() };
        let input = body("marker", 0, vec![block(0, vec![Arc::new(first), Arc::new(marker), Arc::new(last)], vec![])]);
        let result = constants(&input);
        assert_eq!(
            result.insns().iter().filter(|one| one.what.as_ref() == Some(&what)).count(),
            if clobbers.is_empty() { 1 } else { 2 }
        );
    }
}

#[test]
fn test_virtual_identity_marker_does_not_reload_nbody_dividend_constant() {
    // C nbody emitted MOV EAX,512 twice around CDQ before one IDIV.
    let moved = sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![im(512, 4)]);
    let first = insn(0, Some((0, 0)), Some(moved.clone()), vec![1], vec![]);
    let extend = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Extend, "cdq", vec![rl(Register::EDX, 4)], vec![rl(Register::EAX, 4)])),
        vec![2],
        vec![1],
    );
    let marker = insn(2, Some((2, 2)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![3], vec![1]);
    let again = insn(3, Some((3, 3)), Some(moved.clone()), vec![4], vec![]);
    let input = body(
        "nbody-dividend",
        0,
        vec![block(0, vec![Arc::new(first), Arc::new(extend), Arc::new(marker), Arc::new(again)], vec![])],
    );

    let result = constants(&input);

    assert_eq!(result.insns().iter().filter(|one| one.what.as_ref() == Some(&moved)).count(), 1);
    assert!(verify::verify(&result, false).is_empty());
}

#[test]
fn test_qlight_folds_batched_parameter_loads_into_their_extensions() {
    // The QB frontend loaded both parameters before widening either one.
    let (ax, cx, eax, ebx) = (rl(Register::AX, 2), rl(Register::CX, 2), rl(Register::EAX, 4), rl(Register::EBX, 4));
    let left = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(6), 2) });
    let right = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(8), 2) });
    let insns = vec![
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![left.clone()])), vec![1], vec![])),
        Arc::new(insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![cx.clone()], vec![right.clone()])), vec![2], vec![])),
        Arc::new(insn(2, Some((2, 2)), Some(sem(Operation::Extend, "movsx", vec![ebx.clone()], vec![ax])), vec![3], vec![1])),
        Arc::new(insn(3, Some((3, 3)), Some(sem(Operation::Extend, "movsx", vec![eax.clone()], vec![cx])), vec![4], vec![2])),
    ];
    let input = body("qlight-parameters", 0, vec![block(0, insns, vec![])]);

    let real = whats(&extensions(&input).insns());

    assert_eq!(real, [
        sem(Operation::Extend, "movsx", vec![ebx], vec![left]),
        sem(Operation::Extend, "movsx", vec![eax], vec![right]),
    ]);
}

#[test]
fn test_transitive_extension_preserves_nonlocal_machine_state() {
    for guard in ["signedness", "register", "shared", "clobber"] {
        let cell = Loc::Mem(Mem { through: Register::BX, ..Mem::new(None, 1) });
        let mut narrow =
            insn(0, Some((0, 3)), Some(sem(Operation::Extend, "movzx", vec![rl(Register::DX, 2)], vec![cell])), vec![1], vec![]);
        let mut wide = insn(
            3,
            Some((3, 6)),
            Some(sem(Operation::Extend, "movzx", vec![rl(Register::EDX, 4)], vec![rl(Register::DX, 2)])),
            vec![2],
            vec![1],
        );
        let mut tail = vec![];
        match guard {
            "signedness" => wide.what.as_mut().unwrap().name = Some("movsx".to_owned()),
            "register" => wide.what.as_mut().unwrap().dests = vec![rl(Register::EAX, 4)],
            "shared" => {
                tail.push(Arc::new(insn(
                    6,
                    Some((6, 6)),
                    Some(sem(Operation::Push, "push", vec![], vec![rl(Register::DX, 2)])),
                    vec![],
                    vec![1],
                )));
            }
            _ => narrow.clobbers = [Register::AX].into(),
        }
        let input = body("guarded", 0, vec![block(0, [vec![Arc::new(narrow), Arc::new(wide)], tail].concat(), vec![])]);
        assert_eq!(extensions(&input), input, "{guard}");
    }
}

/// Unrolling marked every clone symbolic; matmul then kept seven of its eight
/// `mov bx,[m]; movsx ebx,bx` pairs unfolded in the hot loop.
#[test]
fn test_register_only_extension_clone_does_not_claim_a_relocation() {
    let cell = Loc::Mem(Mem { through: Register::BX, ..Mem::new(None, 1) });
    let mut narrow =
        insn(0, Some((0, 3)), Some(sem(Operation::Extend, "movzx", vec![rl(Register::DX, 2)], vec![cell.clone()])), vec![1], vec![]);
    narrow.symbol = Some(true);
    let mut wide = insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Extend, "movzx", vec![rl(Register::EDX, 4)], vec![rl(Register::DX, 2)])),
        vec![2],
        vec![1],
    );
    wide.symbol = Some(true);
    let input = body("cloned-extension", 0, vec![block(0, vec![Arc::new(narrow), Arc::new(wide)], vec![])]);

    let result = extensions(&input).insns();

    assert_eq!(result[0].symbol, Some(true));
    assert_eq!(result[1].symbol, Some(false));
    assert_eq!(result[0].what.as_ref().unwrap().sources, vec![cell]);
}

#[test]
fn test_constant_knowledge_is_local_and_invalidated() {
    for interruption in
        ["none", "extend", "extend_write", "extend_clobber", "call", "clobber", "unknown", "relocation", "block"]
    {
        let mut source = im(512, 4);
        if interruption == "relocation" {
            source = Loc::Imm(Imm { value: 512, width: 4, address: Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }) });
        }
        let what = sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![source]);
        let first = insn(0, Some((0, 1)), Some(what.clone()), vec![], vec![]);
        let last = Insn { at: 2, covers: Some((2, 3)), ..first.clone() };
        let mut middle =
            insn(1, Some((1, 2)), Some(sem(Operation::Move, "mov", vec![rl(Register::BX, 2)], vec![im(7, 2)])), vec![], vec![]);
        if interruption == "call" {
            middle.what = Some(sem(Operation::Call, "call", vec![], vec![]));
        }
        if ["extend", "extend_clobber"].contains(&interruption) {
            middle.what = Some(sem(Operation::Extend, "cdq", vec![rl(Register::EDX, 4)], vec![rl(Register::EAX, 4)]));
            if interruption == "extend_clobber" {
                middle.clobbers = [Register::AH].into();
            }
        }
        if interruption == "extend_write" {
            middle.what = Some(sem(Operation::Extend, "movsx", vec![rl(Register::EAX, 4)], vec![rl(Register::AX, 2)]));
        }
        if interruption == "clobber" {
            middle.clobbers = [Register::EAX].into();
        }
        if interruption == "unknown" {
            middle.what = None;
        }
        let (first, middle, last) = (Arc::new(first), Arc::new(middle), Arc::new(last));
        let blocks = if interruption == "block" {
            vec![block(0, vec![first, middle], vec![2]), block(2, vec![last], vec![])]
        } else {
            vec![block(0, vec![first, middle, last], vec![])]
        };
        let result = constants(&body("constants", 0, blocks));
        assert_eq!(
            result.insns().iter().filter(|one| one.what.as_ref() == Some(&what)).count(),
            if ["none", "extend"].contains(&interruption) { 1 } else { 2 },
            "{interruption}"
        );
    }
}

#[test]
fn test_a_string_fill_reading_the_direction_flag_leaves_zero_as_xor() {
    // `rep stosb` reads DF; xor leaves DF as it is.
    let (ax, bx) = (rl(Register::AX, 2), rl(Register::BX, 2));
    let make = |at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>| {
        Arc::new(insn(at, Some((at, at)), Some(semt(op, name, dests, sources, target)), vec![], vec![]))
    };
    let blocks = vec![
        block(
            0,
            vec![
                make(0, Operation::Move, "mov", vec![ax.clone()], vec![im(0, 2)], None),
                make(1, Operation::Jump, "jmp", vec![], vec![], Some(2)),
            ],
            vec![2],
        ),
        block(
            2,
            vec![
                make(2, Operation::Fill, "stosb", vec![Loc::Mem(Mem::new(None, 0))], vec![], None),
                make(3, Operation::Compare, "cmp", vec![], vec![ax, bx], None),
                make(4, Operation::Jump, "jmp", vec![], vec![], Some(0)),
            ],
            vec![0],
        ),
    ];
    let result = zeroes(&body("zero", 0, blocks)).blocks[0].insns[0].clone();
    let what = result.what.as_ref().unwrap();
    assert_eq!((what.op, what.name.as_deref()), (Operation::Binary, Some("xor")));
}

// ---------------------------------------------------------- test_machine_copyprop

fn moved(at: i64, dest: Loc, source: Loc) -> Insn {
    insn(at, Some((at, at)), Some(sem(Operation::Move, "mov", vec![dest], vec![source])), vec![], vec![])
}

#[test]
fn test_copy_at_join_requires_agreement_on_every_path() {
    // An AX=BX copy survived a diamond despite unchanged contents; writing AH must keep it.
    // "conditional" is not ported: it hangs a decoded node on `Insn.op`, which a `mir::Op` cannot carry.
    for reverse_order in [false, true] {
        for change in ["none", "other", "partial", "source", "unknown", "loop"] {
            let (dest, source) = (rl(Register::AX, 2), rl(Register::BX, 2));
            let (first, last) = (moved(0, dest.clone(), source.clone()), moved(30, dest, source));
            let branch = insn(1, Some((1, 1)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(20))), vec![], vec![]);
            let mut middle = vec![];
            if ["other", "partial", "source", "loop"].contains(&change) {
                let register = match change {
                    "other" => Register::CX,
                    "partial" | "loop" => Register::AH,
                    _ => Register::BX,
                };
                let width = if register == Register::AH { 1 } else { 2 };
                middle.push(Arc::new(moved(20, rl(register, width), im(7, width))));
            }
            if change == "unknown" {
                middle.push(Arc::new(Insn { at: 20, what: None, ..last.clone() }));
            }
            let mut blocks = vec![
                block(0, vec![Arc::new(first), Arc::new(branch)], vec![10, 20]),
                block(10, vec![], vec![30]),
                block(20, middle, vec![30]),
                block(30, vec![Arc::new(last.clone())], if change == "loop" { vec![20, 40] } else { vec![] }),
            ];
            if change == "loop" {
                blocks.push(block(40, vec![], vec![]));
            }
            if reverse_order {
                blocks.reverse();
            }
            let result = transform(body("copies", 0, blocks));
            assert_eq!(
                result.insns().iter().any(|one| one.at == 30 && one.what == last.what),
                !["none", "other"].contains(&change),
                "{change} {reverse_order}"
            );
        }
    }
}

#[test]
fn test_high_byte_write_keeps_low_byte_copy_available() {
    // Writing AH does not invalidate a known AL=BL relation across a block edge.
    let first = moved(0, rl(Register::AX, 2), rl(Register::BX, 2));
    let high = moved(1, rl(Register::AH, 1), im(7, 1));
    let low = moved(10, rl(Register::AL, 1), rl(Register::BL, 1));
    let result = transform(body(
        "lanes",
        0,
        vec![block(0, vec![Arc::new(first), Arc::new(high)], vec![10]), block(10, vec![Arc::new(low.clone())], vec![])],
    ));
    assert!(!result.insns().iter().any(|one| one.at == 10 && one.what == low.what));
}

#[test]
fn test_copy_source_is_forwarded_to_an_explicit_use() {
    // ls_face_key spent `mov bx,ax` only to compare BX; the loop paid one instruction each trip.
    let (ax, bx) = (rl(Register::AX, 2), rl(Register::BX, 2));
    let copied = moved(0, bx.clone(), ax.clone());
    let compared = insn(1, Some((1, 1)), Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(116, 2)])), vec![], vec![]);
    let overwritten = moved(2, bx, im(7, 2));
    let result = transform(body(
        "ls_face_key",
        0,
        vec![block(0, vec![Arc::new(copied.clone()), Arc::new(compared.clone()), Arc::new(overwritten)], vec![])],
    ));

    let insns = result.insns();
    assert!(!insns.iter().any(|one| one.what == copied.what));
    assert!(insns.iter().any(|one| one.at == compared.at && one.what.as_ref().unwrap().sources[0] == ax));
}

#[test]
fn test_copy_forwarding_across_a_join_requires_every_lane_on_every_path() {
    // A reaching copy crosses a diamond only while neither full nor partial register is clobbered.
    for changed in [None, Some(Register::AX), Some(Register::AH), Some(Register::BX), Some(Register::BH)] {
        let (ax, bx) = (rl(Register::AX, 2), rl(Register::BX, 2));
        let copied = moved(0, bx.clone(), ax.clone());
        let branch = insn(1, Some((1, 1)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(20))), vec![], vec![]);
        let mut middle = vec![];
        if let Some(changed) = changed {
            let width = if [Register::AH, Register::BH].contains(&changed) { 1 } else { 2 };
            middle.push(Arc::new(moved(20, rl(changed, width), im(7, width))));
        }
        let compared =
            insn(30, Some((30, 30)), Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(116, 2)])), vec![], vec![]);
        let result = transform(body(
            "join",
            0,
            vec![
                block(0, vec![Arc::new(copied), Arc::new(branch)], vec![10, 20]),
                block(10, vec![], vec![30]),
                block(20, middle, vec![30]),
                block(30, vec![Arc::new(compared.clone())], vec![]),
            ],
        ));
        let actual = result
            .insns()
            .iter()
            .find(|one| one.at == compared.at)
            .map(|one| one.what.as_ref().unwrap().sources[0].clone())
            .unwrap();
        assert_eq!(actual, if changed.is_none() { ax } else { bx }, "{changed:?}");
    }
}

#[test]
fn test_copy_forwarding_does_not_rename_a_fixed_register_use() {
    // GCC/LLVM both recheck constraints: CWD still reads AX even when AX is a copy of BX.
    let (ax, bx, dx) = (rl(Register::AX, 2), rl(Register::BX, 2), rl(Register::DX, 2));
    let copied = moved(0, ax.clone(), bx);
    let fixed = insn(1, Some((1, 1)), Some(sem(Operation::Extend, "cwd", vec![dx], vec![ax.clone()])), vec![], vec![]);
    let result = transform(body("fixed", 0, vec![block(0, vec![Arc::new(copied), Arc::new(fixed.clone())], vec![])]));
    let found = result.insns().iter().find(|one| one.at == fixed.at).unwrap().what.as_ref().unwrap().sources[0].clone();
    assert_eq!(found, ax);
}

#[test]
fn test_copy_forwarding_maps_matching_subregister_lanes() {
    // An AX=BX fact forwards AL to BL after AH changes; the surviving low-byte fact is sufficient.
    let copied = moved(0, rl(Register::AX, 2), rl(Register::BX, 2));
    let high = moved(1, rl(Register::AH, 1), im(7, 1));
    let compared = insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(Register::AL, 1), im(3, 1)])),
        vec![],
        vec![],
    );
    let result = transform(body(
        "subregister",
        0,
        vec![block(0, vec![Arc::new(copied), Arc::new(high), Arc::new(compared.clone())], vec![])],
    ));
    let found = result.insns().iter().find(|one| one.at == compared.at).unwrap().what.as_ref().unwrap().sources[0].clone();
    assert_eq!(found, rl(Register::BL, 1));
}

// ------------------------------------------------------------ test_memory_folding

fn plain(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>) -> Arc<Insn> {
    Arc::new(insn(at, None, Some(semt(op, name, dests, sources, target)), vec![], vec![]))
}

fn delayed(head: Vec<Arc<Insn>>, eax: &Loc, ecx: &Loc) -> LirBody {
    let overwrite = plain(4, Operation::Move, "mov", vec![eax.clone()], vec![ecx.clone()], None);
    let jump = plain(5, Operation::Jump, "jmp", vec![], vec![], Some(0));
    body("delayed-memory-read", 0, vec![block(0, head, vec![1]), block(1, vec![overwrite, jump], vec![0])])
}

#[test]
fn test_memory_round_trip_folds_across_an_independent_operand_load() {
    // C nbody used four instructions where BCC writes `mov vel; add [pos],reg`.
    // Python printed these through `masm`; the reprs are of the semantics it printed.
    let (eax, ecx) = (rl(Register::EAX, 4), rl(Register::ECX, 4));
    let position = Loc::Mem(mem(frame(-4), 4, Register::BP, 0, 2));
    let velocity = Loc::Mem(mem(frame(-8), 4, Register::BP, 0, 2));
    let head = vec![
        plain(0, Operation::Move, "mov", vec![eax.clone()], vec![position.clone()], None),
        plain(1, Operation::Move, "mov", vec![ecx.clone()], vec![velocity], None),
        plain(2, Operation::Binary, "add", vec![eax.clone()], vec![eax.clone(), ecx.clone()], None),
        plain(3, Operation::Move, "mov", vec![position], vec![eax.clone()], None),
    ];

    let result = fused(&delayed(head, &eax, &ecx)).blocks[0].clone();
    let printed: Vec<String> = whats(&result.insns).iter().map(Repr::repr).collect();

    let pos = "Mem(addr=[bp-0x4], width=4, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
               selector=None, index=None, scale=1, index_through=0)";
    let vel = "Mem(addr=[bp-0x8], width=4, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
               selector=None, index=None, scale=1, index_through=0)";
    assert_eq!(printed, [
        format!(
            "Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=(Reg(register=38, width=4),), \
             sources=({vel},), target=None, indirect=False)"
        ),
        format!(
            "Semantics(op=<Operation.BINARY: 'binary'>, name='add', dests=({pos},), \
             sources=({pos}, Reg(register=38, width=4)), target=None, indirect=False)"
        ),
    ]);
}

#[test]
fn test_single_use_loaded_addend_folds_into_the_arithmetic_operand() {
    // Frontend-parity MEMORY emitted C's `mov cx,[si]; add ax,cx`.
    let (ax, cx) = (rl(Register::AX, 2), rl(Register::CX, 2));
    let delta = Loc::Mem(mem(Some(Addr { base: Register::SI, ..Addr::new(Space::Segment, 0) }), 2, Register::SI, 0, 0));
    let load = plain(0, Operation::Move, "mov", vec![cx.clone()], vec![delta.clone()], None);
    let addition = plain(1, Operation::Binary, "add", vec![ax.clone()], vec![ax.clone(), cx], None);
    let mut returned = op(2, Operation::Return, "", Kind::Return);
    returned.reads_complete = true;
    let finish = Arc::new(Insn {
        op: Some(Arc::new(returned)),
        ..insn(2, None, Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![])
    });
    let input = body("loaded-addend", 0, vec![block(0, vec![load, addition, Arc::clone(&finish)], vec![])]);

    let physical = whats(&fused(&input).insns());

    assert_eq!(physical, [sem(Operation::Binary, "add", vec![ax.clone()], vec![ax, delta]), finish.what.clone().unwrap()]);
}

fn narrow_load_parts() -> (Mem, Arc<Insn>, Arc<Insn>) {
    let cell = mem(frame(-8), 2, Register::BP, 0, 2);
    let narrow = Arc::new(insn(
        0,
        None,
        Some(sem(Operation::Move, "mov", vec![rl(Register::CX, 2)], vec![Loc::Mem(cell.clone())])),
        vec![1],
        vec![],
    ));
    let wide = Arc::new(insn(
        1,
        None,
        Some(sem(Operation::Extend, "movsx", vec![rl(Register::EDX, 4)], vec![rl(Register::CX, 2)])),
        vec![2],
        vec![1],
    ));
    (cell, narrow, wide)
}

#[test]
fn test_narrow_load_folds_into_its_only_widening_use() {
    // C matmul emitted `mov cx,[array]; movsx edx,cx` eight times.
    let (cell, narrow, wide) = narrow_load_parts();
    let used = plain(
        2,
        Operation::Binary,
        "add",
        vec![rl(Register::EAX, 4)],
        vec![rl(Register::EAX, 4), rl(Register::EDX, 4)],
        None,
    );
    let input = body("load-extend", 0, vec![block(0, vec![narrow, wide, used], vec![])]);

    let result = extensions(&input);
    let emitted: Vec<Arc<Insn>> =
        result.insns().into_iter().filter(|one| one.what.as_ref().unwrap().op != Operation::Nothing).collect();

    assert_eq!(emitted[0].what, Some(sem(Operation::Extend, "movsx", vec![rl(Register::EDX, 4)], vec![Loc::Mem(cell)])));
    assert_eq!(emitted[0].defines, [2]);
    assert_eq!(emitted.len(), 2);
    assert!(verify::verify(&result, false).is_empty());
}

#[test]
fn test_shared_narrow_load_is_not_folded_into_one_widening_use() {
    // The load must remain when another instruction still reads its narrow value.
    let (_cell, narrow, wide) = narrow_load_parts();
    let other = Arc::new(insn(2, None, Some(sem(Operation::Push, "push", vec![], vec![rl(Register::CX, 2)])), vec![], vec![1]));
    let input = body("shared-load", 0, vec![block(0, vec![narrow, wide, other], vec![])]);
    assert_eq!(extensions(&input), input);
}

#[test]
fn test_one_use_compare_folds_before_a_complete_return() {
    // indexed.lru_use loaded a sign test into DI before immediately returning.
    let di = rl(Register::DI, 2);
    let cell = Loc::Mem(mem(frame(-4), 2, Register::BP, 0, 2));
    let load = Arc::new(insn(1, None, Some(sem(Operation::Move, "mov", vec![di.clone()], vec![cell.clone()])), vec![1], vec![]));
    let compare = Arc::new(insn(2, None, Some(sem(Operation::Compare, "cmp", vec![], vec![di, im(0, 2)])), vec![], vec![1]));
    let branch = plain(3, Operation::Branch, "jge", vec![], vec![], Some(2));
    let mut returned = op(4, Operation::Return, "", Kind::Return);
    returned.reads_complete = true;
    let ret = Arc::new(Insn {
        op: Some(Arc::new(returned)),
        ..insn(4, None, Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![])
    });
    let input = body(
        "complete-return-fold",
        0,
        vec![
            block(0, vec![load, compare, Arc::clone(&branch)], vec![1, 2]),
            block(1, vec![Arc::clone(&ret)], vec![]),
            block(2, vec![ret], vec![]),
        ],
    );

    let result = fused(&input);

    assert_eq!(whats(&result.blocks[0].insns), [
        sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)]),
        branch.what.clone().unwrap(),
    ]);
}

#[test]
fn test_dead_compare_load_may_overwrite_its_own_address_register() {
    // lru_use's bnext test used DI for both the pointer and loaded value.
    let di = rl(Register::DI, 2);
    let cell = Loc::Mem(mem(Some(Addr { base: Register::DI, ..Addr::new(Space::Segment, 0) }), 2, Register::DI, 0, 2));
    let load = plain(1, Operation::Move, "mov", vec![di.clone()], vec![cell.clone()], None);
    let compare = plain(2, Operation::Compare, "cmp", vec![], vec![di.clone(), im(0, 2)], None);
    let overwrite = plain(3, Operation::Move, "mov", vec![di], vec![rl(Register::AX, 2)], None);
    let input = body(
        "self-addressed-compare",
        0,
        vec![block(0, vec![load, compare, overwrite, plain(4, Operation::Jump, "jmp", vec![], vec![], Some(0))], vec![0])],
    );

    let physical = whats(&fused(&input).insns());

    assert_eq!(physical[0], sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)]));
}

#[test]
fn test_memory_round_trip_does_not_cross_a_dependent_or_writing_instruction() {
    // Delayed memory folding must not change which cell or value an add reads.
    for hazard in ["uses loaded value", "changes address", "writes memory"] {
        let (eax, ebx, ecx) = (rl(Register::EAX, 4), rl(Register::EBX, 4), rl(Register::ECX, 4));
        let through = if hazard == "changes address" { Register::BX } else { Register::BP };
        let position = Loc::Mem(mem(
            Some(if through == Register::BX {
                Addr { base: through, ..Addr::new(Space::Segment, 0) }
            } else {
                Addr::new(Space::Frame, -4)
            }),
            4,
            through,
            0,
            2,
        ));
        let velocity = Loc::Mem(mem(frame(-8), 4, Register::BP, 0, 2));
        let mut other = ecx.clone();
        let between = match hazard {
            "uses loaded value" => {
                plain(1, Operation::Move, "mov", vec![ecx.clone()], vec![Loc::Mem(mem(None, 4, Register::EAX, 0, 4))], None)
            }
            "changes address" => {
                other = ebx.clone();
                plain(1, Operation::Move, "mov", vec![ebx], vec![velocity], None)
            }
            _ => plain(1, Operation::Move, "mov", vec![position.clone()], vec![ecx.clone()], None),
        };
        let head = vec![
            plain(0, Operation::Move, "mov", vec![eax.clone()], vec![position.clone()], None),
            between,
            plain(2, Operation::Binary, "add", vec![eax.clone()], vec![eax.clone(), other], None),
            plain(3, Operation::Move, "mov", vec![position], vec![eax.clone()], None),
        ];
        assert_eq!(fused(&delayed(head.clone(), &eax, &ecx)).blocks[0].insns, head, "{hazard}");
    }
}

// --------------------------------------------------------------- test_addressforms

#[test]
fn test_mandel_sum_uses_67h_lea_before_preserving_a_copy() {
    // Mandelbrot emitted `mov sum,xx; add sum,yy` before its escape test.
    for width in [2u32, 4] {
        let registers = if width == 2 {
            [Register::DI, Register::DX, Register::SI]
        } else {
            [Register::EDI, Register::EDX, Register::ESI]
        };
        let [dest, left, right] = registers.map(|register| rl(register, width));
        let copy = Insn {
            widths: vec![(1, width), (3, width)],
            ..insn(47, Some((47, 47)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![left])), vec![3], vec![1])
        };
        let addition = Insn {
            widths: vec![(2, width), (3, width), (4, width)],
            ..insn(
                47,
                Some((47, 48)),
                Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest.clone(), right])),
                vec![4],
                vec![3, 2],
            )
        };
        let compare = Insn {
            widths: vec![(4, width)],
            ..insn(48, Some((48, 48)), Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(1024, width)])), vec![], vec![4])
        };
        let input = body("mandel", 47, vec![block(47, vec![Arc::new(copy), Arc::new(addition), Arc::new(compare)], vec![])]);

        let result = addresses(&input, "386").unwrap().insns();

        assert_eq!(names(&result), ["lea", "cmp"]);
        assert_eq!(result[0].defines, [4]);
        assert_eq!(result[0].uses, [1, 2]);
        assert_eq!(result[0].widths, [(1, width), (2, width), (4, width)]);
        let Loc::Address(address) = &result[0].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
        let mut pair = [address.through, address.index];
        pair.sort();
        assert_eq!(pair, [Register::EDX, Register::ESI]);
        let encoded = code(result[0].what.as_ref().unwrap());
        assert!(encoded[..2].contains(&0x67));
    }
}

#[test]
fn test_sum_lea_preserves_observed_add_flags() {
    // A conditional branch after the sum still reads ADD's flags.
    let [dest, left, right] = [Register::EDI, Register::EDX, Register::ESI].map(|register| rl(register, 4));
    let copy = insn(47, Some((47, 47)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![left])), vec![3], vec![1]);
    let addition = insn(
        47,
        Some((47, 48)),
        Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest, right])),
        vec![4],
        vec![3, 2],
    );
    let branch = insn(48, Some((48, 48)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(60))), vec![], vec![]);
    let input = body("flagged", 47, vec![block(47, vec![Arc::new(copy), Arc::new(addition), Arc::new(branch)], vec![60])]);

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "add", "je"]);
}

#[test]
fn test_sum_lea_preserves_a_copy_result_read_after_the_add() {
    // Modern nbody lost value 333 and stopped in the LIR verifier.
    let (destination, left, right, saved) =
        (rl(Register::EAX, 4), rl(Register::EBP, 4), rl(Register::EDI, 4), rl(Register::EDX, 4));
    let copy =
        insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![destination.clone()], vec![left])), vec![3], vec![1]);
    let addition = insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Binary, "add", vec![destination.clone()], vec![destination.clone(), right])),
        vec![1],
        vec![1, 2],
    );
    let preserve =
        insn(3, Some((3, 3)), Some(sem(Operation::Move, "mov", vec![saved], vec![destination.clone()])), vec![4], vec![3]);
    let compare =
        insn(4, Some((4, 4)), Some(sem(Operation::Compare, "cmp", vec![], vec![destination, im(0, 4)])), vec![], vec![1]);
    let input = LirBody {
        inputs: [1, 2].into(),
        ..body(
            "live-copy-sum",
            1,
            vec![block(1, vec![Arc::new(copy), Arc::new(addition), Arc::new(preserve), Arc::new(compare)], vec![])],
        )
    };

    let transformed = addresses(&input, "386").unwrap();

    assert_eq!(names(&transformed.insns()), ["mov", "add", "mov", "cmp"]);
    assert!(verify::verify(&transformed, false).is_empty());
}

fn _constant_sum_body(amount: i64, symbolic_add: bool) -> LirBody {
    let (dest, source) = (rl(Register::BX, 2), rl(Register::AX, 2));
    let copy = Insn {
        widths: vec![(1, 2), (3, 2)],
        ..insn(10, Some((10, 10)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])), vec![3], vec![1])
    };
    let addition = Insn {
        widths: vec![(3, 2), (4, 2)],
        symbol: if symbolic_add { Some(true) } else { None },
        ..insn(
            10,
            Some((10, 11)),
            Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest.clone(), im(amount, 2)])),
            vec![4],
            vec![3],
        )
    };
    let store = Insn {
        widths: vec![(4, 2)],
        ..insn(
            11,
            Some((11, 11)),
            Some(sem(
                Operation::Move,
                "mov",
                vec![Loc::Mem(Mem { through: Register::BP, offset: -4, ..Mem::new(frame(-4), 2) })],
                vec![dest],
            )),
            vec![],
            vec![4],
        )
    };
    let compare = Insn {
        widths: vec![(1, 2)],
        ..insn(12, Some((12, 12)), Some(sem(Operation::Compare, "cmp", vec![], vec![source, im(0, 2)])), vec![], vec![1])
    };
    body(
        "constant_sum",
        10,
        vec![block(10, vec![Arc::new(copy), Arc::new(addition), Arc::new(store), Arc::new(compare)], vec![])],
    )
}

#[test]
fn test_constant_sum_uses_67h_lea_without_code_growth() {
    // Matmul initialized each local element with `mov bx,ax; add bx,1`.
    for (amount, displacement) in [(1, 1), (0xFFFF, -1)] {
        let result = addresses(&_constant_sum_body(amount, false), "386").unwrap().insns();

        assert_eq!(names(&result), ["lea", "mov", "cmp"]);
        assert_eq!(result[0].what.as_ref().unwrap().sources, [Loc::Address(Address {
            through: Register::EAX,
            offset: displacement,
            ..Address::new(None)
        })]);
        let Loc::Address(address) = &result[0].what.as_ref().unwrap().sources[0] else { unreachable!() };
        // `Address` equality compares only `addr`; Python's does too.
        assert_eq!((address.through, address.offset), (Register::EAX, displacement));
        let encoded = code(result[0].what.as_ref().unwrap());
        assert!(encoded[..2].contains(&0x67));
    }
}

#[test]
fn test_constant_sum_keeps_a_shorter_move_add_encoding() {
    // A 32-bit LEA displacement must not grow a shorter word-immediate pair.
    let result = addresses(&_constant_sum_body(0x1234, false), "386").unwrap().insns();
    assert_eq!(names(&result), ["mov", "add", "mov", "cmp"]);
}

#[test]
fn test_constant_sum_preserves_unrolled_source_anchor_on_lea() {
    // Matmul's unrolled ADD clone owned its conservative source anchor.
    let result = addresses(&_constant_sum_body(1, true), "386").unwrap().insns();
    assert_eq!(names(&result), ["lea", "mov", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
}

#[test]
fn test_constant_sum_does_not_drop_a_predecessor_source_anchor() {
    // A fold cannot discard source ownership held by the removed copy.
    let input = _constant_sum_body(1, false);
    let mut insns = input.blocks[0].insns.clone();
    insns[0] = Arc::new(Insn { symbol: Some(true), ..(*insns[0]).clone() });
    let input = LirBody { blocks: vec![LirBlock { insns, ..input.blocks[0].clone() }], ..input };

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "add", "mov", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
}

// ----------------------------------------------------- test_dead_address_arithmetic

#[test]
fn test_folded_culling_address_drops_only_unobserved_arithmetic() {
    // r_walk's shared +8 address was folded into its loads but left MOV AX,SI / ADD AX,8.
    for flag_user in [Some("add"), Some("adc"), None] {
        let (ax, si, di) = (rl(Register::AX, 2), rl(Register::SI, 2), rl(Register::DI, 2));
        let copy = Arc::new(moved(0, ax.clone(), si.clone()));
        let address = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "add", vec![ax.clone()], vec![ax.clone(), im(8, 2)])),
            vec![],
            vec![],
        ));
        let flags = Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, flag_user.unwrap_or("add"), vec![di.clone()], vec![di, im(4, 2)])),
            vec![],
            vec![],
        ));
        let overwrite = Arc::new(moved(3, ax, si));
        let operations = if flag_user.is_some() {
            vec![Arc::clone(&copy), Arc::clone(&address), flags, Arc::clone(&overwrite)]
        } else {
            vec![Arc::clone(&copy), Arc::clone(&address), Arc::clone(&overwrite)]
        };
        let result = overwritten(&body("cull", 0, vec![block(0, operations, vec![])])).insns();
        assert_eq!(!result.contains(&address), flag_user == Some("add"), "{flag_user:?}");
        assert_eq!(!result.contains(&copy), flag_user == Some("add"), "{flag_user:?}");
        assert!(result.contains(&overwrite));
    }
}

// ------------------------------------------------------------------ test_parcopy

fn group_move(into: Loc, out_of: Loc, group: Option<i64>, at: i64) -> Arc<Insn> {
    Arc::new(Insn { group, ..insn(at, Some((at, at)), Some(sem(Operation::Move, "mov", vec![into], vec![out_of])), vec![], vec![]) })
}

fn one_block(insns: Vec<Arc<Insn>>) -> LirBody {
    body("one", 0, vec![block(0, insns, vec![])])
}

fn frame_slots(width: u32) -> (Loc, Loc) {
    (Loc::Mem(mem(frame(-4), width, Register::BP, 0, 2)), Loc::Mem(mem(frame(-8), width, Register::BP, 0, 2)))
}

fn reset() -> Arc<Insn> {
    Arc::new(insn(0x102, Some((0x102, 0x102)), Some(sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![im(0, 4)])), vec![], vec![]))
}

#[test]
fn test_frame_copy_uses_a_dead_register_before_the_stack() {
    // Mandel reset its column recurrence with PUSH-memory/POP-memory each row.
    let (source, destination) = frame_slots(4);
    let scheduled =
        parcopy::scheduled(&one_block(vec![group_move(destination.clone(), source.clone(), Some(1), 0x100), reset()])).unwrap();

    let instructions = frame_copies(&scheduled, "386").unwrap().blocks[0].insns.clone();

    assert_eq!(names(&instructions), ["mov", "mov", "mov"]);
    assert_eq!(instructions[0].what, Some(sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![source])));
    assert_eq!(instructions[1].what, Some(sem(Operation::Move, "mov", vec![destination], vec![rl(Register::EAX, 4)])));
}

#[test]
fn test_frame_copy_keeps_the_stack_when_no_register_is_dead() {
    // A scratch shuttle may not overwrite a value live out of the copy.
    let (source, destination) = frame_slots(2);
    let scheduled = parcopy::scheduled(&one_block(vec![group_move(destination, source, Some(1), 0x100)])).unwrap();

    let instructions = frame_copies(&scheduled, "386").unwrap().blocks[0].insns.clone();

    assert_eq!(names(&instructions), ["push", "pop"]);
}

#[test]
fn test_source_push_pop_is_not_treated_as_a_parallel_copy() {
    // Only parcopy's synthetic pair may lose its observable stack traffic.
    let (source, destination) = frame_slots(2);
    let mut pair = parcopy::scheduled(&one_block(vec![group_move(destination, source, Some(1), 0x100)])).unwrap().blocks[0]
        .insns
        .clone();
    pair[1] = Arc::new(Insn { at: 0x101, covers: Some((0x101, 0x102)), ..(*pair[1]).clone() });
    pair.push(reset());

    let instructions = frame_copies(&one_block(pair), "386").unwrap().blocks[0].insns.clone();

    assert_eq!(names(&instructions[..2]), ["push", "pop"]);
}

// ------------------------------------------------------------ test_postallocation

#[test]
fn test_dword_constant_is_narrowed_when_the_abi_reads_only_its_low_word() {
    // C SCALAR emitted `mov eax,1789` where BASIC needed only AX.
    let value = mir::Value::new(1, 1);
    let source = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![rl(Register::EAX, 4)], vec![im(1789, 4)])),
        vec![value.id],
        vec![],
    );
    let returned = mir::Op {
        kind: Kind::Return,
        args: vec![mir::Arg::Held(mir::Held { value, width: 2 })],
        reads_complete: true,
        ..mir::Op::new(2, OpCode::Operation(Operation::Return), "ret", vec![], vec![value])
    };
    let finish = Insn {
        requires: vec![(Held { value: value.id, width: 2 }, Register::AX)],
        op: Some(Arc::new(returned)),
        ..insn(2, Some((2, 2)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![value.id])
    };
    // Source/symbol ownership anchors from the unrolled frontend body must be
    // transparent to physical liveness even though they constrain layout.
    let anchor = Insn { symbol: Some(true), ..insn(2, Some((2, 2)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![]) };
    let input = body("return-low", 1, vec![block(1, vec![Arc::new(source), Arc::new(anchor), Arc::new(finish)], vec![])]);

    let result = narrowed_moves(&input);

    assert_eq!(result.insns()[0].what, Some(sem(Operation::Move, "mov", vec![rl(Register::AX, 2)], vec![im(1789, 2)])));
}

#[test]
fn test_dword_constant_stays_wide_when_any_upper_lane_is_live() {
    let wide = rl(Register::EAX, 4);
    let source = insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![wide.clone()], vec![im(1789, 4)])), vec![1], vec![]);
    let used = insn(2, Some((2, 2)), Some(sem(Operation::Compare, "cmp", vec![], vec![wide, im(0, 4)])), vec![], vec![1]);
    let input = body("return-wide", 1, vec![block(1, vec![Arc::new(source), Arc::new(used)], vec![])]);

    assert_eq!(narrowed_moves(&input), input);
}

#[test]
fn test_dword_fixed_register_argument_keeps_all_value_lanes_live() {
    // Native nbody left its last Y velocity undamped: `mov bx,8192` with stale upper EBX bits.
    let source =
        insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![rl(Register::EBX, 4)], vec![im(8192, 4)])), vec![1], vec![]);
    let call = Insn {
        clobbers: [Register::EAX, Register::EBX, Register::ECX, Register::EDX].into(),
        requires: vec![(Held { value: 1, width: 4 }, Register::BX)],
        ..insn(2, Some((2, 2)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![1])
    };
    let input = body("wide-fixed-argument", 1, vec![block(1, vec![Arc::new(source), Arc::new(call)], vec![])]);

    assert_eq!(narrowed_moves(&input), input);
}

fn extract_parts(source: Loc, high: Loc, discarded: Loc, last: Insn) -> Vec<Arc<Insn>> {
    let marker = op(10, Operation::Restore, "extract", Kind::Extract);
    vec![
        Arc::new(Insn {
            op: Some(Arc::new(marker)),
            ..insn(10, Some((10, 10)), Some(sem(Operation::Push, "push", vec![], vec![source])), vec![], vec![1])
        }),
        Arc::new(insn(10, Some((10, 10)), Some(sem(Operation::Pop, "pop", vec![discarded], vec![])), vec![2], vec![])),
        Arc::new(insn(10, Some((10, 10)), Some(sem(Operation::Pop, "pop", vec![high.clone()], vec![])), vec![3], vec![])),
        Arc::new(insn(11, Some((11, 11)), Some(sem(Operation::Compare, "cmp", vec![], vec![high, im(0, 2)])), vec![], vec![3])),
        Arc::new(last),
    ]
}

#[test]
fn test_register_high_extract_uses_one_double_shift_for_dx_ax_return() {
    // Frontend-parity ALGEBRA returned its high word with push/pop/pop.
    let source = rl(Register::EBX, 4);
    let last = insn(12, Some((12, 12)), Some(sem(Operation::Move, "mov", vec![rl(Register::EDX, 4)], vec![im(0, 4)])), vec![4], vec![]);
    let parts = extract_parts(source.clone(), rl(Register::DX, 2), rl(Register::BX, 2), last);
    let input = body("return-high", 10, vec![block(10, parts, vec![])]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shld", "cmp", "mov"]);
    assert_eq!(
        result[0].what,
        Some(sem(Operation::Funnel, "shld", vec![rl(Register::EDX, 4)], vec![rl(Register::EDX, 4), source, im(16, 1)]))
    );
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_selected_move_shift_high_extract_uses_the_same_double_shift() {
    // Frontend-parity ALGEBRA's C path retained MOV EDX,ECX; SHR EDX,16.
    let (source, high) = (rl(Register::ECX, 4), rl(Register::EDX, 4));
    let mut returned = op(12, Operation::Return, "return", Kind::Return);
    returned.reads_complete = true;
    let parts = vec![
        Arc::new(insn(10, Some((10, 10)), Some(sem(Operation::Move, "mov", vec![high.clone()], vec![source.clone()])), vec![3], vec![1])),
        Arc::new(insn(
            11,
            Some((11, 11)),
            Some(sem(Operation::Binary, "shr", vec![high.clone()], vec![high.clone(), im(16, 1)])),
            vec![3],
            vec![3],
        )),
        Arc::new(Insn {
            op: Some(Arc::new(returned)),
            requires: vec![(Held { value: 4, width: 2 }, Register::AX), (Held { value: 3, width: 2 }, Register::DX)],
            ..insn(
                12,
                Some((12, 12)),
                Some(sem(Operation::Return, "retf", vec![], vec![rl(Register::AX, 2), rl(Register::DX, 2)])),
                vec![],
                vec![4, 3],
            )
        }),
    ];
    let input = body("c-return-high", 10, vec![block(10, parts, vec![])]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shld", "retf"]);
    assert_eq!(result[0].what, Some(sem(Operation::Funnel, "shld", vec![high.clone()], vec![high, source, im(16, 1)])));
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_register_high_extract_shifts_a_dying_return_root_in_place() {
    // Frontend-parity LOOP returned EDX through push/pop/pop.
    let source = rl(Register::EDX, 4);
    let last = insn(12, Some((12, 12)), Some(sem(Operation::Move, "mov", vec![source.clone()], vec![im(0, 4)])), vec![4], vec![]);
    let parts = extract_parts(source.clone(), rl(Register::DX, 2), rl(Register::BX, 2), last);
    let input = body("return-high-in-place", 10, vec![block(10, parts, vec![])]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shr", "cmp", "mov"]);
    assert_eq!(result[0].what, Some(sem(Operation::Binary, "shr", vec![source.clone()], vec![source, im(16, 1)])));
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_dead_flags_crossing_increment_do_not_block_zero_idiom() {
    // PARITYCONTROL kept `mov eax,0` because a later DEC preserved dead CF.
    let (eax, bx, cx) = (rl(Register::EAX, 4), rl(Register::BX, 2), rl(Register::CX, 2));
    let insns = vec![
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![eax.clone()], vec![im(0, 4)])), vec![1], vec![])),
        Arc::new(insn(1, Some((1, 1)), Some(sem(Operation::Unary, "dec", vec![bx.clone()], vec![bx])), vec![2], vec![2])),
        Arc::new(insn(2, Some((2, 2)), Some(sem(Operation::Binary, "and", vec![cx.clone()], vec![cx.clone(), cx])), vec![3], vec![3])),
        Arc::new(insn(3, Some((3, 3)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(1))), vec![], vec![])),
    ];
    let input = body("zero-before-dec", 0, vec![block(0, insns, vec![1])]);

    let result = zeroes(&input);

    assert_eq!(result.insns()[0].what, Some(sem(Operation::Binary, "xor", vec![eax.clone()], vec![eax.clone(), eax])));
}

#[test]
fn test_return_high_extraction_drops_redundant_low_word_shuttle() {
    // PARITYMEMORY returned EAX as AX:DX through two needless BX moves.
    let (eax, ax, bx, edx, dx) =
        (rl(Register::EAX, 4), rl(Register::AX, 2), rl(Register::BX, 2), rl(Register::EDX, 4), rl(Register::DX, 2));
    let parts = vec![
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![ax.clone()])), vec![2], vec![1])),
        Arc::new(insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![edx.clone()], vec![eax])), vec![3], vec![1])),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, "shr", vec![edx.clone()], vec![edx, im(16, 1)])),
            vec![4],
            vec![3],
        )),
        Arc::new(insn(3, Some((3, 3)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![bx])), vec![5], vec![2])),
        Arc::new(Insn {
            clobbers: [Register::EBX].into(),
            ..insn(4, Some((4, 4)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])
        }),
        Arc::new(Insn {
            requires: vec![(Held { value: 5, width: 2 }, Register::AX), (Held { value: 4, width: 2 }, Register::DX)],
            ..insn(5, Some((5, 5)), Some(sem(Operation::Return, "retf", vec![], vec![ax, dx])), vec![], vec![5, 4])
        }),
    ];
    let input = body("return-pair", 0, vec![block(0, parts.clone(), vec![])]);

    let result = transform(input);

    assert_eq!(whats(&result.insns()), [
        parts[1].what.clone().unwrap(),
        parts[2].what.clone().unwrap(),
        parts[5].what.clone().unwrap(),
    ]);
}

#[test]
fn test_unit_add_selects_inc_only_when_carry_is_dead() {
    // Frontend-parity LOOP used C's ADD 1 but BASIC's equivalent INC.
    let counter = rl(Register::CX, 2);
    let add = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "add", vec![counter.clone()], vec![counter.clone(), im(1, 2)])),
        vec![1],
        vec![1],
    ));
    let compare =
        Arc::new(insn(2, Some((2, 2)), Some(sem(Operation::Compare, "cmp", vec![], vec![counter, im(8, 2)])), vec![], vec![1]));
    let dead = body("counter", 1, vec![block(1, vec![Arc::clone(&add), compare], vec![])]);
    let branch = Arc::new(insn(2, Some((2, 2)), Some(semt(Operation::Branch, "jb", vec![], vec![], Some(3))), vec![], vec![]));
    let live = body("carry", 1, vec![block(1, vec![add, branch], vec![3]), block(3, vec![], vec![])]);

    assert_eq!(names(&increments(&dead).insns())[0], "inc");
    assert_eq!(names(&increments(&live).insns())[0], "add");
}

fn _pair(operation: Operation, name: &str, tail: Vec<Arc<Insn>>) -> (LirBody, Insn) {
    let (left, right) = (rl(Register::EBX, 4), rl(Register::ECX, 4));
    let combined = insn(1, Some((1, 3)), Some(sem(operation, name, vec![left.clone()], vec![left.clone(), right.clone()])), vec![10], vec![1, 2]);
    let copied = insn(3, Some((3, 3)), Some(sem(Operation::Move, "mov", vec![right], vec![left])), vec![11], vec![10]);
    let insns = [vec![Arc::new(combined), Arc::new(copied.clone())], tail].concat();
    (body("pair", 0, vec![block(0, insns, vec![])]), copied)
}

#[test]
fn test_commutative_result_copy_uses_the_dying_other_operand() {
    // Mandel's inner loop emitted `imul ebx,ecx; mov ecx,ebx`.
    for (operation, name) in [
        (Operation::Binary, "add"),
        (Operation::Binary, "and"),
        (Operation::Binary, "or"),
        (Operation::Binary, "xor"),
        (Operation::Multiply, "imul"),
    ] {
        let (left, right) = (rl(Register::EBX, 4), rl(Register::ECX, 4));
        let shift = Arc::new(insn(
            3,
            Some((3, 5)),
            Some(sem(Operation::Binary, "sar", vec![right.clone()], vec![right.clone(), im(7, 1)])),
            vec![12],
            vec![11],
        ));
        let overwrite =
            Arc::new(insn(5, Some((5, 7)), Some(sem(Operation::Move, "mov", vec![left.clone()], vec![im(0, 4)])), vec![13], vec![]));
        let (input, copied) = _pair(operation, name, vec![Arc::clone(&shift), Arc::clone(&overwrite)]);

        let result = transferred(&input).insns();

        assert_eq!(result[0].what, Some(sem(operation, name, vec![right.clone()], vec![right, left])));
        assert_eq!(result[1].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(result[1].defines, copied.defines);
        assert_eq!(result[2..], [shift, overwrite]);
    }
}

#[test]
fn test_commutative_result_copy_keeps_a_still_live_first_operand() {
    // Changing which multiply input is destroyed is legal only when the old destination dies.
    let used = Arc::new(insn(
        3,
        Some((3, 5)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(Register::EBX, 4), im(0, 4)])),
        vec![],
        vec![10],
    ));
    let (input, _copied) = _pair(Operation::Multiply, "imul", vec![used]);

    assert_eq!(transferred(&input), input);
}

#[test]
fn test_commutative_result_copy_keeps_source_owned_copy_bytes() {
    // A real input instruction is not the synthetic transfer this rewrite may erase.
    let overwrite =
        Arc::new(insn(5, Some((5, 7)), Some(sem(Operation::Move, "mov", vec![rl(Register::EBX, 4)], vec![im(0, 4)])), vec![13], vec![]));
    let (input, copied) = _pair(Operation::Multiply, "imul", vec![Arc::clone(&overwrite)]);
    let owned = Arc::new(Insn { covers: Some((3, 5)), ..copied });
    let input = LirBody {
        blocks: vec![LirBlock { insns: vec![Arc::clone(&input.insns()[0]), owned, overwrite], ..input.blocks[0].clone() }],
        ..input
    };

    assert_eq!(transferred(&input), input);
}

fn _high_extract_body(tail: Vec<Arc<Insn>>) -> LirBody {
    let value = mir::Value::new(1, 1);
    let source = mir::Value::new(2, 1);
    let operation = Arc::new(mir::Op {
        kind: Kind::Shr,
        args: vec![mir::Arg::Held(mir::Held { value: source, width: 4 }), mir::Arg::Const(mir::Const::new(16, 1))],
        results: vec![mir::Arg::Held(mir::Held { value, width: 4 })],
        ..mir::Op::new(1, OpCode::Operation(Operation::Binary), "shr", vec![value], vec![source])
    });
    let wide = rl(Register::EDX, 4);
    let cell = Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-4), 4) });
    let load = Insn {
        op: Some(Arc::clone(&operation)),
        symbol: Some(false),
        ..insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![wide.clone()], vec![cell])), vec![1], vec![])
    };
    let shift = Insn {
        op: Some(operation),
        ..insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "shr", vec![wide.clone()], vec![wide, im(16, 1)])),
            vec![1],
            vec![1],
        )
    };
    body("extract", 0, vec![block(0, [vec![Arc::new(load), Arc::new(shift)], tail].concat(), vec![])])
}

fn _return_high() -> Arc<Insn> {
    let value = mir::Value::new(1, 1);
    let operation = mir::Op {
        kind: Kind::Return,
        args: vec![mir::Arg::Held(mir::Held { value, width: 2 })],
        reads_complete: true,
        ..mir::Op::new(2, OpCode::Operation(Operation::Return), "", vec![], vec![value])
    };
    Arc::new(Insn {
        op: Some(Arc::new(operation)),
        requires: vec![(Held { value: 1, width: 2 }, Register::DX)],
        ..insn(2, Some((2, 3)), Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![1])
    })
}

#[test]
fn test_synthetic_high_extract_loads_only_the_high_word() {
    // Mandel loaded a spilled dword and shifted it solely to return the high word.
    let returned = _return_high();

    let result = high_extracts(&_high_extract_body(vec![Arc::clone(&returned)]), "386").unwrap().insns();

    let (load, anchor) = (&result[0], &result[1]);
    assert_eq!(
        load.what,
        Some(sem(
            Operation::Move,
            "mov",
            vec![rl(Register::DX, 2)],
            vec![Loc::Mem(Mem { through: Register::BP, ..Mem::new(frame(-2), 2) })],
        ))
    );
    assert_eq!(anchor.what.as_ref().unwrap().op, Operation::Nothing);
    assert_eq!(anchor.defines, [1]);
    assert_eq!(result[2..], [returned]);
}

#[test]
fn test_high_extract_keeps_observable_wide_load_or_shift_effects() {
    // Narrowing is legal only for a synthetic load with dead upper lanes and flags.
    for hazard in ["full-result", "flags", "source-load"] {
        let tail = match hazard {
            "full-result" => vec![
                Arc::new(insn(
                    2,
                    Some((2, 4)),
                    Some(sem(Operation::Compare, "cmp", vec![], vec![rl(Register::EDX, 4), im(0, 4)])),
                    vec![],
                    vec![1],
                )),
                _return_high(),
            ],
            "flags" => vec![Arc::new(insn(2, Some((2, 4)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(9))), vec![], vec![]))],
            _ => vec![_return_high()],
        };
        let mut input = _high_extract_body(tail);
        if hazard == "source-load" {
            let mut insns = input.blocks[0].insns.clone();
            insns[0] = Arc::new(Insn { covers: Some((1, 3)), symbol: None, ..(*insns[0]).clone() });
            input = LirBody { blocks: vec![LirBlock { insns, ..input.blocks[0].clone() }], ..input };
        }

        assert_eq!(high_extracts(&input, "386").unwrap(), input, "{hazard}");
    }
}

// ------------------------------------------------------------------- test_prologue

fn procedure() -> LirBody {
    let instruction = |at: i64, what: Semantics| insn(at, Some((at, at + 1)), Some(what), vec![], vec![]);
    let held = Loc::Held(Held { value: 1, width: 2 });
    let insns = vec![
        Arc::new(instruction(0, sem(Operation::Move, "mov", vec![held.clone()], vec![im(6, 2)]))),
        Arc::new(Insn {
            requires: vec![(Held { value: 1, width: 2 }, Register::CX)],
            ..instruction(1, sem(Operation::Call, "call", vec![], vec![held]))
        }),
        Arc::new(instruction(2, sem(Operation::Call, "call", vec![], vec![]))),
        Arc::new(instruction(3, sem(Operation::Return, "retf", vec![], vec![]))),
    ];
    body("procedure", 0, vec![block(0, insns, vec![])])
}

/// Python's phases share one frame; a copy taken before allocation saw no
/// spill slots and kept a dead `sub sp` reservation.
#[test]
fn test_peephole_sees_slots_added_after_it_was_built() {
    let shared = Rc::new(RefCell::new(Frame::new(-16)));
    let phase = Peephole::new(Some(Rc::clone(&shared)), "386").unwrap();
    shared.borrow_mut().cell(1i64, 2).unwrap();
    let calls = IndexMap::from_iter([(1, "B$ENRA".to_owned()), (2, "B$EXSA".to_owned())]);
    let reserved = prologue::reserved(&procedure(), &shared.borrow(), Some(&calls)).unwrap();
    let result = phase._frame(reserved);
    assert_eq!(result.insns().iter().filter(|one| one.frame_adjust).count(), 0);
}

#[test]
fn test_empty_spill_reservation_is_removed_only_without_remaining_uses() {
    // QB FPCSE retained SUB SP,2 after its final dead spill reload vanished.
    for used in ["none", "load", "address", "opaque"] {
        let mut slots = Frame::new(-16);
        let cell = slots.cell(1i64, 2).unwrap();
        let mut input = procedure();
        if used != "none" {
            let what = (used != "opaque").then(|| {
                sem(
                    Operation::Move,
                    "mov",
                    vec![rl(Register::AX, 2)],
                    vec![if used == "load" {
                        Loc::Mem(cell.clone())
                    } else {
                        Loc::Imm(Imm { value: 0, width: 2, address: cell.addr })
                    }],
                )
            });
            let one = Arc::new(insn(4, Some((4, 4)), what, vec![], vec![]));
            input.blocks[0].insns.push(one);
        }
        let calls = IndexMap::from_iter([(1, "B$ENRA".to_owned()), (2, "B$EXSA".to_owned())]);
        let reserved = prologue::reserved(&input, &slots, Some(&calls)).unwrap();
        let result = Peephole::new(Some(Rc::new(RefCell::new(slots))), "386").unwrap()._frame(reserved);
        assert_eq!(
            result.insns().iter().filter(|one| one.frame_adjust).count(),
            if used == "none" { 0 } else { 2 },
            "{used}"
        );
    }
}
