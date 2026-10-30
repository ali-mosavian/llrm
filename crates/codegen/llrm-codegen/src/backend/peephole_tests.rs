//! Ports of the peephole tests: tests/test_peephole.py and the peephole-only
//! tests of test_machine_copyprop, test_memory_folding, test_addressforms,
//! test_dead_address_arithmetic, test_parcopy, test_postallocation and
//! test_prologue.  Checks Python made through `masm` compare the `repr` of
//! the semantics Python printed instead: `masm` is not ported.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use llrm_lir::registers::RegId;

use super::*;
use crate::backend::frame::Frame;
use crate::backend::{machinecp, prologue, verify};
use crate::model::ir::Addr;
use crate::support::hash::HashMap;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::Repr;

/// The 16-bit x86 rules, as the phase takes them by default.
fn rules() -> &'static crate::backend::peep::Rules {
    &crate::backend::targets::x86_m16::RULES
}

fn r(
    register: RegId,
    width: u32,
) -> Reg {
    Reg { register, width }
}

fn rl(
    register: RegId,
    width: u32,
) -> Loc {
    Loc::Reg(r(register, width))
}

fn im(
    value: i64,
    width: u32,
) -> Loc {
    Loc::Imm(imm(value, width))
}

fn sem(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    semantics(op, name, dests, sources)
}

fn semt(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    target: Option<i64>,
) -> Semantics {
    Semantics { target, ..semantics(op, name, dests, sources) }
}

fn insn(
    at: i64,
    covers: Option<(i64, i64)>,
    what: Option<Semantics>,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Insn {
    Insn::new(at, covers, what, defines, uses)
}

fn block(
    at: i64,
    insns: Vec<Arc<Insn>>,
    succ: Vec<i64>,
) -> LirBlock {
    LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
}

fn body(
    name: &str,
    entry: i64,
    blocks: Vec<Block>,
) -> LirBody {
    LirBody::new(name, entry, blocks, IndexMap::default(), IndexMap::default())
}

fn mem(
    addr: Option<Addr>,
    width: u32,
    through: RegId,
    offset: i64,
    disp_width: u32,
) -> Mem {
    Mem { through, offset, disp_width, ..Mem::new(addr, width) }
}

fn frame(disp: i64) -> Option<Addr> {
    Some(Addr::new(Space::Frame, disp))
}

fn names(insns: &[Arc<Insn>]) -> Vec<String> {
    insns.iter().map(|one| one.what.as_ref().unwrap().name.as_deref().map(str::to_owned).unwrap()).collect()
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

// ----------------------------------------------------------------
// test_peephole

#[test]
fn test_screen_argument_reuses_its_required_register_constant() {
    // SCREEN's duplicate PUSH 1/MOV AX,1 contributed to E1M1 exhausting its far
    // heap.
    for (width, register, value) in [(2, RegId::AX, 1), (4, RegId::ECX, 0x1234_5678)] {
        let operand = im(value, width);
        let dest = rl(register, width);
        let push =
            insn(0, Some((0, 1)), Some(sem(Operation::Push, "push", vec![], vec![operand.clone()])), vec![], vec![]);
        let moved = insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![operand])),
            vec![7],
            vec![],
        );
        let result = transform(body(
            "screen",
            0,
            vec![block(0, vec![Arc::new(push.clone()), Arc::new(moved.clone())], vec![]).into()],
        ));
        let expected = [moved.what.clone().unwrap(), Semantics { sources: vec![dest], ..push.what.clone().unwrap() }];
        let insns = result.insns();
        assert_eq!(
            insns.iter().flat_map(|one| code(one.what.as_ref().unwrap())).collect::<Vec<u8>>(),
            expected.iter().flat_map(code).collect::<Vec<u8>>()
        );
        assert_eq!(insns[0].defines, [7]);
        assert_eq!(insns[1].uses, [7]);
    }
}

#[test]
fn test_argument_materialization_does_not_cross_observable_boundaries() {
    // SCREEN's stack argument must not change when its register setup cannot
    // move before it.
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
            Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![literal])),
            vec![7],
            vec![],
        );
        match barrier {
            "different" => moved.what.as_mut().unwrap().sources = vec![im(2, 2)],
            "width" => moved.what.as_mut().unwrap().dests = vec![rl(RegId::EAX, 4)],
            "stack" | "frame" => {
                moved.what.as_mut().unwrap().dests =
                    vec![rl(if barrier == "stack" { RegId::SP } else { RegId::BP }, 2)];
            }
            "relocation" => {
                let symbol = Loc::Imm(Imm {
                    value: 1,
                    width: 2,
                    address: Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }),
                });
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
            "clobber" => push.clobbers = [RegId::AX].into(),
            "requires" => push.requires = vec![(Held { value: 5, width: 2 }, RegId::AX)],
            _ => {}
        }
        let (push, moved) = (Arc::new(push), Arc::new(moved));
        let blocks = if barrier == "block" {
            vec![block(0, vec![push], vec![1]), block(1, vec![moved], vec![])]
        } else {
            vec![block(0, vec![push, moved], vec![])]
        };
        let input = body("screen", 0, crate::model::lir::shared(blocks));
        assert_eq!(pushed_constants(rules(), &input), input, "{barrier}");
    }
}

#[test]
fn test_forwarded_register_copy_retains_its_virtual_definition() {
    // PITSNAP's IN read value#188 after copy propagation removed its AL setup.
    let (al, ah) = (rl(RegId::AL, 1), rl(RegId::AH, 1));
    let establish = insn(1, Some((1, 2)), Some(sem(Operation::Move, "mov", vec![al], vec![ah])), vec![], vec![]);
    let define = Insn { at: 2, covers: Some((2, 3)), defines: vec![2], ..establish.clone() };
    let consume = insn(3, Some((3, 4)), None, vec![3], vec![2]);
    let input = body(
        "forwarded-copy-definition",
        1,
        vec![block(1, vec![Arc::new(establish), Arc::new(define), Arc::new(consume)], vec![]).into()],
    );

    let done = machinecp::propagated(&input);

    assert!(verify::verify(&done, false).is_empty());
    assert_eq!(
        done.insns().iter().filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Move)).count(),
        1
    );
}

/// `mov t,a ; mov a,x ; add a,a,t` became `mov t,a ; add a,a,x` and the second
/// copy, which defines a value a later call requires in EAX, was gone: under
/// LLRM_VERIFY the Nib runtime's `format.put` (m32, a freed frame register)
/// failed with "value is read but never defined".
#[test]
fn test_commuted_accumulator_leaves_the_dropped_copys_value_defined() {
    let [accumulator, temporary, term] = [RegId::ECX, RegId::ESI, RegId::EAX].map(|register| r(register, 4));
    let sems = [
        sem(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Reg(accumulator)]),
        sem(Operation::Move, "mov", vec![Loc::Reg(accumulator)], vec![Loc::Reg(term)]),
        sem(Operation::Binary, "add", vec![Loc::Reg(accumulator)], vec![Loc::Reg(accumulator), Loc::Reg(temporary)]),
    ];
    let make = |at: i64, what: Option<Semantics>, defines: Vec<u32>, uses: Vec<u32>| {
        Arc::new(insn(at, Some((at, at + 1)), what, defines, uses))
    };
    let insns = vec![
        make(0, None, vec![0, 3], vec![]),
        make(1, Some(sems[0].clone()), vec![1], vec![0]),
        make(2, Some(sems[1].clone()), vec![2], vec![3]),
        make(3, Some(sems[2].clone()), vec![4], vec![0, 1]),
        make(4, None, vec![], vec![2]),
    ];
    let input = body("dropped-copy", 0, vec![block(0, insns, vec![]).into()]);
    let done = commuted(rules(), &input);
    assert_ne!(done, input, "the rule fired");
    let said = verify::verify(&done, false);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn test_commuted_accumulator_keeps_the_saved_value() {
    // NBODY's saved accumulator must remain valid even when a later instruction
    // reads it.
    for name in ["add", "and", "or", "xor", "sub", "adc"] {
        for width in [2u32, 4] {
            let registers =
                if width == 2 { [RegId::CX, RegId::SI, RegId::AX] } else { [RegId::ECX, RegId::ESI, RegId::EAX] };
            let [accumulator, temporary, term] = registers.map(|register| r(register, width));
            let semantics = [
                sem(Operation::Move, "mov", vec![Loc::Reg(temporary)], vec![Loc::Reg(accumulator)]),
                sem(Operation::Move, "mov", vec![Loc::Reg(accumulator)], vec![Loc::Reg(term)]),
                sem(
                    Operation::Binary,
                    name,
                    vec![Loc::Reg(accumulator)],
                    vec![Loc::Reg(accumulator), Loc::Reg(temporary)],
                ),
            ];
            let insns: Vec<Arc<Insn>> = semantics
                .iter()
                .enumerate()
                .map(|(index, what)| {
                    let index = index as i64;
                    Arc::new(insn(index, Some((index, index + 1)), Some(what.clone()), vec![], vec![]))
                })
                .collect();
            let input = body("accumulator", 0, vec![block(0, insns.clone(), vec![]).into()]);
            let result = commuted(rules(), &input);
            if ["sub", "adc"].contains(&name) {
                assert_eq!(result, input);
                continue;
            }
            // The dropped copy stays as an anchor (see the test below).
            let out: Vec<_> = result
                .insns()
                .into_iter()
                .filter(|one| one.what.as_ref().is_some_and(|what| what.op != Operation::Nothing))
                .collect();
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

/// `push 112; push 112` joined into `pushd 7340144` at -Os: 6 bytes for 4.
/// Tuned for size two word pushes join only where the dword is no longer (`push
/// 0; push 0` is `pushd 0`, 3 for 4).
#[test]
fn test_word_push_pair_joins_tuned_for_size_only_where_the_dword_is_no_longer() {
    for (high, low, joined, size) in
        [(0x70i64, 0x70i64, false, true), (0x70, 0x70, true, false), (0, 0, true, true), (0x3F80, 0, false, true)]
    {
        let push = |at: i64, number: i64| {
            Arc::new(insn(
                at,
                Some((at, at + 3)),
                Some(sem(Operation::Push, "push", vec![], vec![im(number, 2)])),
                vec![],
                vec![],
            ))
        };
        let input = body("arguments", 0, vec![block(0, vec![push(0, high), push(3, low)], vec![]).into()]);

        let result = pushes(rules(), &input, crate::backend::cpu::tuned("486", size).unwrap()).insns();

        assert_eq!(result.len(), if joined { 1 } else { 2 }, "{high:#x}:{low:#x} size={size}");
    }
}

/// `mov cx, 8; push cx` with `cx` dead after the push is `push 8`: the dag
/// pushed each constant of a QB runtime call through a register, 15 bytes
/// where the old selector's pushes were 8 (nbody.bas, +309 B under dag).
#[test]
fn test_a_constant_made_only_to_be_pushed_is_the_pushes_immediate() {
    let cx = rl(RegId::CX, 2);
    let made = plain(0, Operation::Move, "mov", vec![cx.clone()], vec![im(8, 2)], None);
    let pushed = plain(1, Operation::Push, "push", vec![], vec![cx], None);
    let finish = Arc::new(Insn {
        reads_complete: true,
        ..insn(2, None, Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![])
    });
    let input = body("argument", 0, vec![block(0, vec![made, pushed, finish], vec![]).into()]);

    let result = fused(rules(), &input).insns();

    let whats: Vec<_> = result.iter().filter_map(|one| one.what.as_ref()).collect();
    assert!(whats.iter().all(|what| what.name.as_deref() != Some("mov")), "{whats:?}");
    let push = whats.iter().find(|what| what.op == Operation::Push).expect("the push stays");
    assert!(matches!(push.sources.as_slice(), [Loc::Imm(Imm { value: 8, .. })]), "{push:?}");
}

/// `mov eax, 0; push ax`: a zero made wide and pushed narrow stayed a register
/// (`xor ax, ax; push ax`, 3 bytes against 2 for `push 0`; cmpord.bas: 24 of
/// them, 559 `xor` over the corpus under dag).
#[test]
fn test_a_wide_constant_pushed_narrow_is_the_pushes_immediate() {
    let eax = rl(RegId::AX, 4);
    let made = plain(0, Operation::Move, "mov", vec![eax], vec![im(0, 4)], None);
    let pushed = plain(1, Operation::Push, "push", vec![], vec![rl(RegId::AX, 2)], None);
    let finish = Arc::new(Insn {
        reads_complete: true,
        ..insn(2, None, Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![])
    });
    let input = body("argument", 0, vec![block(0, vec![made, pushed, finish], vec![]).into()]);

    let result = fused(rules(), &input).insns();

    let whats: Vec<_> = result.iter().filter_map(|one| one.what.as_ref()).collect();
    assert!(whats.iter().all(|what| what.name.as_deref() != Some("mov")), "{whats:?}");
    let push = whats.iter().find(|what| what.op == Operation::Push).expect("the push stays");
    assert!(matches!(push.sources.as_slice(), [Loc::Imm(Imm { value: 0, .. })]), "{push:?}");
}

/// `mov ax, DGROUP; push ax`: a segment address made only to be pushed stayed
/// a register (4 bytes against 3 for `pushw DGROUP`; gvnjn.bas, and every
/// runtime call that takes a far pointer: two per call under dag).
#[test]
fn test_a_segment_made_only_to_be_pushed_is_the_pushes_immediate() {
    let segment =
        Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }) });
    let ax = rl(RegId::AX, 2);
    let made = plain(0, Operation::Move, "mov", vec![ax.clone()], vec![segment], None);
    let pushed = plain(1, Operation::Push, "push", vec![], vec![ax], None);
    let finish = Arc::new(Insn {
        reads_complete: true,
        ..insn(2, None, Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![])
    });
    let input = body("argument", 0, vec![block(0, vec![made, pushed, finish], vec![]).into()]);

    let result = fused(rules(), &input).insns();

    let whats: Vec<_> = result.iter().filter_map(|one| one.what.as_ref()).collect();
    assert!(whats.iter().all(|what| what.name.as_deref() != Some("mov")), "{whats:?}");
    let push = whats.iter().find(|what| what.op == Operation::Push).expect("the push stays");
    assert!(matches!(push.sources.as_slice(), [Loc::Imm(Imm { address: Some(_), .. })]), "{push:?}");
}

#[test]
fn test_constant_push_pair_preserves_stack_bytes() {
    for (high, low) in [(0x43F3i64, 0xC000i64), (-1, -2), (0, 0), (0x8000, 0x7FFF)] {
        let push = |at: i64, number: i64| {
            Arc::new(insn(
                at,
                Some((at, at + 3)),
                Some(sem(Operation::Push, "push", vec![], vec![im(number, 2)])),
                vec![],
                vec![],
            ))
        };
        let pair = vec![push(0, high), push(3, low)];
        let result = pushes(
            rules(),
            &body("arguments", 0, vec![block(0, pair, vec![]).into()]),
            crate::backend::cpu::tuned("486", false).unwrap(),
        )
        .insns();
        assert!(result.len() == 1 && result[0].covers == Some((0, 6)));
        let Loc::Imm(operand) = &result[0].what.as_ref().unwrap().sources[0] else { unreachable!() };
        let expected: Vec<u8> =
            ((low & 0xFFFF) as u16).to_le_bytes().into_iter().chain(((high & 0xFFFF) as u16).to_le_bytes()).collect();
        assert_eq!(u32::try_from(operand.value).unwrap().to_le_bytes().to_vec(), expected);
        let made = code(result[0].what.as_ref().unwrap());
        let decoded = Decoder::new(16, &made, DecoderOptions::NONE).decode();
        assert_eq!(decoded.stack_pointer_increment(), -4);
    }
}

#[test]
fn test_constant_push_fusion_stops_at_boundaries() {
    for barrier in ["relocation", "block", "instruction"] {
        let mut first =
            insn(0, Some((0, 3)), Some(sem(Operation::Push, "push", vec![], vec![im(1, 2)])), vec![], vec![]);
        let second = Insn { at: 3, covers: Some((3, 6)), ..first.clone() };
        match barrier {
            "relocation" => {
                first.what.as_mut().unwrap().sources = vec![Loc::Imm(Imm {
                    value: 1,
                    width: 2,
                    address: Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }),
                })];
            }
            _ => {}
        }
        let (first, second) = (Arc::new(first), Arc::new(second));
        let insns = if barrier == "instruction" {
            let nop = insn(3, Some((3, 3)), Some(sem(Operation::Nothing, "nop", vec![], vec![])), vec![], vec![]);
            vec![Arc::clone(&first), Arc::new(nop), Arc::clone(&second)]
        } else {
            vec![Arc::clone(&first), Arc::clone(&second)]
        };
        let blocks = if barrier == "block" {
            vec![block(0, vec![first], vec![3]), block(3, vec![second], vec![])]
        } else {
            vec![block(0, insns, vec![])]
        };
        let input = body("boundary", 0, crate::model::lir::shared(blocks));
        assert_eq!(pushes(rules(), &input, crate::backend::cpu::tuned("486", false).unwrap()), input, "{barrier}");
    }
}

/// Source bytes never keep code alive. Two pushes whose bytes were not
/// contiguous stayed two, because `without` refused to drop the second.
#[test]
fn test_a_byte_gap_does_not_stop_push_fusion() {
    let first = insn(0, Some((0, 3)), Some(sem(Operation::Push, "push", vec![], vec![im(1, 2)])), vec![], vec![]);
    let second = Insn { at: 4, covers: Some((4, 7)), ..first.clone() };
    let fused = pushes(
        rules(),
        &body("gap", 0, vec![block(0, vec![Arc::new(first.clone()), Arc::new(second.clone())], vec![]).into()]),
        crate::backend::cpu::tuned("486", false).unwrap(),
    );
    let code: Vec<_> = fused.insns().into_iter().filter(|one| !one.is_meta()).collect();
    assert_eq!(code.len(), 1, "{code:?}");
    assert_eq!(fused.owned_bytes(), [0, 1, 2, 4, 5, 6]);

    // Nor does a marker between them.
    let marker = insn(3, Some((3, 4)), Some(lir::inert()), vec![], vec![]);
    let fused = pushes(
        rules(),
        &body("marked", 0, vec![block(0, vec![Arc::new(first), Arc::new(marker), Arc::new(second)], vec![]).into()]),
        crate::backend::cpu::tuned("486", false).unwrap(),
    );
    assert_eq!(fused.insns().iter().filter(|one| !one.is_meta()).count(), 1, "{:?}", fused.insns());
    assert_eq!(fused.owned_bytes(), [0, 1, 2, 3, 4, 5, 6]);
}

#[test]
fn test_dead_reload_requires_allocator_ownership_and_no_read() {
    // FPCSE's dead spill is removable, but source loads and live spills are
    // not.
    for owned in [false, true] {
        for read in [false, true] {
            let ax = rl(RegId::AX, 2);
            let slot = Frame::new(0).cell(1i64, 2).unwrap();
            let load = Arc::new(Insn {
                spill_reload: owned,
                ..insn(
                    0,
                    Some((0, 0)),
                    Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![Loc::Mem(slot)])),
                    vec![],
                    vec![],
                )
            });
            let used = Arc::new(insn(
                1,
                Some((1, 1)),
                Some(sem(Operation::Move, "mov", vec![rl(RegId::BX, 2)], vec![ax.clone()])),
                vec![],
                vec![],
            ));
            let write = Arc::new(insn(
                2,
                Some((2, 2)),
                Some(sem(Operation::Move, "mov", vec![ax], vec![im(4, 2)])),
                vec![],
                vec![],
            ));
            let insns = if read { vec![Arc::clone(&load), used, write] } else { vec![Arc::clone(&load), write] };
            let result = overwritten(&body("reload", 0, vec![block(0, insns, vec![]).into()]));
            assert_eq!(!result.insns().contains(&load), owned && !read, "{owned} {read}");
        }
    }
}

#[test]
fn test_overwritten_reload_keeps_virtual_definition() {
    // mdl_draw_tris kept a zero-cost copy of a spilled selector after its
    // reload was physically dead.
    let ax = rl(RegId::AX, 2);
    let slot = Frame::new(0).cell(1i64, 2).unwrap();
    let load = Insn {
        spill_reload: true,
        ..insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![Loc::Mem(slot)])),
            vec![1],
            vec![],
        )
    };
    let copy = lir::anchor(Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![ax.clone()])),
        vec![2],
        vec![1],
    )));
    let overwrite = insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![ax], vec![im(0, 2)])), vec![], vec![]);
    let input = body("reload", 0, vec![block(0, vec![Arc::new(load), copy, Arc::new(overwrite)], vec![]).into()]);

    let result = overwritten(&input);
    assert!(verify::verify(&result, false).is_empty());
    assert!(result.insns().iter().any(|one| one.what.as_ref().unwrap().op == Operation::Nothing && one.defines == [1]));
}

#[test]
fn test_overwritten_register_copy_respects_byte_reads() {
    // FPDEEP retains AX/SI allocation shuffles overwritten before any use.
    for (middle, removed) in [(RegId::CX, true), (RegId::AL, false), (RegId::AH, false)] {
        let (ax, si) = (rl(RegId::AX, 2), rl(RegId::SI, 2));
        let copy = Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![si])),
            vec![],
            vec![],
        ));
        let width = if [RegId::AL, RegId::AH].contains(&middle) { 1 } else { 2 };
        let read = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(
                Operation::Move,
                "mov",
                vec![rl(if width == 1 { RegId::BL } else { RegId::BX }, width)],
                vec![rl(middle, width)],
            )),
            vec![],
            vec![],
        ));
        let overwrite = Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Move, "mov", vec![ax], vec![im(4, 2)])),
            vec![],
            vec![],
        ));
        let result =
            overwritten(&body("copies", 0, vec![block(0, vec![Arc::clone(&copy), read, overwrite], vec![]).into()]));
        assert_eq!(!result.insns().contains(&copy), removed, "{middle:?}");
    }
}

#[test]
fn test_word_copy_survives_partial_overwrite() {
    // Writing AL or AH alone cannot make a prior AX definition dead.
    for dest in [RegId::AL, RegId::AH] {
        let (ax, si) = (rl(RegId::AX, 2), rl(RegId::SI, 2));
        let copy =
            Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax], vec![si])), vec![], vec![]));
        let write = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![rl(dest, 1)], vec![im(0, 1)])),
            vec![],
            vec![],
        ));
        let input = body("partial", 0, vec![block(0, vec![copy, write], vec![]).into()]);
        assert_eq!(overwritten(&input), input);
    }
}

#[test]
#[ignore = "fails in Python at 5c22b69b too (shl case)"]
fn test_index_lea_preserves_observed_shift_flags() {
    // ADDRM's copy/shift can become LEA only before a complete flag overwrite.
    for following in ["add", "adc", "inc", "shl", "call", "je"] {
        let (dest, source) = (rl(RegId::SI, 2), rl(RegId::BX, 2));
        let copy = insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])),
            vec![2],
            vec![1],
        );
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
        let input =
            body("index", 0, vec![block(0, vec![Arc::new(copy), Arc::new(shift), Arc::new(last)], vec![]).into()]);
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
        let (dest, source) = (rl(RegId::CX, 2), rl(RegId::AX, 2));
        let copy = insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source])),
            vec![2],
            vec![1],
        );
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
            vec![block(0, vec![Arc::new(copy), Arc::new(shift), Arc::new(compare)], vec![]).into()],
        );

        let result = addresses(&input, cpu).unwrap().insns();

        assert_eq!(names(&result), expected, "{cpu}");
    }
}

#[test]
fn test_source_owned_scale_converges_with_a_synthetic_frontend() {
    // Frontend-parity ALGEBRA left BASIC's `mov; shl 2; add` intact.
    let (source, dest) = (rl(RegId::EDX, 4), rl(RegId::ECX, 4));
    let copy = insn(
        0x90,
        Some((0x90, 0x90)),
        Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])),
        vec![77],
        vec![72],
    );
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
    let compare = insn(
        0x95,
        Some((0x95, 0x95)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(0, 4)])),
        vec![],
        vec![77],
    );
    let input = body(
        "frontend-scale",
        0x90,
        vec![
            block(
                0x90,
                vec![Arc::new(copy.clone()), Arc::new(shift), Arc::new(addition.clone()), Arc::new(compare)],
                vec![],
            )
            .into(),
        ],
    );

    crate::backend::liverows::SETS_BUILT.with(|one| one.set(0));
    crate::backend::liverows::SETS_BUILT.with(|one| one.set(0));
    let result = addresses(&input, "386").unwrap().insns();
    let built = crate::backend::liverows::SETS_BUILT.with(|one| one.get());
    assert_eq!(built, 1, "sets built: addresses reads block exits only");

    assert_eq!(names(&result), ["lea", "cmp"]);
    assert_eq!(result[0].covers, Some((0x90, 0x95)));
    assert_eq!(result[0].defines, addition.defines);
    assert_eq!(result[0].uses, copy.uses);
}

/// Neither a marker between the parts nor bytes that do not meet stop the
/// LEA. Both did: lngmxx kept `lea` and seven `add`s for `lea [edx+edx*8]`.
#[test]
fn test_scaled_lea_crosses_markers_and_byte_gaps() {
    let (source, dest) = (rl(RegId::EDX, 4), rl(RegId::ECX, 4));
    let copy = insn(
        0x90,
        Some((0x90, 0x90)),
        Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])),
        vec![77],
        vec![72],
    );
    let shift = insn(
        0x90,
        Some((0x90, 0x95)),
        Some(sem(Operation::Binary, "shl", vec![dest.clone()], vec![dest.clone(), im(2, 1)])),
        vec![77],
        vec![77],
    );
    let marker = insn(0x95, Some((0x95, 0x97)), Some(lir::inert()), vec![], vec![]);
    let addition = insn(
        0x99,
        Some((0x99, 0x9b)),
        Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest.clone(), source])),
        vec![77],
        vec![77, 72],
    );
    let compare = insn(
        0x9b,
        Some((0x9b, 0x9b)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(0, 4)])),
        vec![],
        vec![77],
    );
    let parts = [copy, shift, marker, addition, compare].map(Arc::new).to_vec();
    let input = body("marked-scale", 0x90, vec![block(0x90, parts, vec![]).into()]);

    let result = addresses(&input, "386").unwrap();

    let code: Vec<Arc<Insn>> = result.insns().into_iter().filter(|one| !one.is_meta()).collect();
    assert_eq!(names(&code), ["lea", "cmp"]);
    assert_eq!(result.owned_bytes(), input.owned_bytes());
}

#[test]
fn test_scaled_lea_does_not_require_another_shift() {
    // HOTLPX's scale idiom needed three instructions when followed by CMP
    // instead of SHL.
    for amount in [1i64, 2, 3] {
        for following in ["cmp", "adc", "je"] {
            for width in [2u32, 4] {
                let dest = rl(if width == 2 { RegId::BX } else { RegId::EBX }, width);
                let source = rl(if width == 2 { RegId::CX } else { RegId::ECX }, width);
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
                let input = body("scale", 0, vec![block(0, vec![copy, shift, add, last], vec![]).into()]);
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
                    let bytes = code(result[0].what.as_ref().unwrap());
                    let emitted = iced_x86::Decoder::with_ip(16, &bytes, 0, iced_x86::DecoderOptions::NONE).decode();
                    assert_eq!(emitted.memory_index_scale(), 1 << amount);
                }
            }
        }
    }
}

#[test]
fn test_loaded_scaled_add_uses_67h_lea() {
    // Matmul emitted `mov temp,[spill]; shl temp,1; add sum,temp`.
    let (temporary, total) = (rl(RegId::EAX, 4), rl(RegId::EDX, 4));
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-4), 4) });
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
    let compare =
        insn(3, Some((3, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])), vec![], vec![2]);
    let input = body(
        "matmul-scale",
        0,
        vec![block(0, vec![Arc::new(load), Arc::new(shift), Arc::new(add), Arc::new(compare)], vec![]).into()],
    );

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "lea", "cmp"]);
    let Loc::Address(address) = &result[1].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
    assert_eq!(address.through, RegId::EDX);
    assert_eq!(address.index_through, RegId::EAX);
    assert_eq!(address.scale, 2);
}

#[test]
fn test_loaded_scaled_add_skips_metadata_only_anchors() {
    // Matmul retained `load; shl; add` when metadata anchors separated it.
    let (temporary, total) = (rl(RegId::EBX, 4), rl(RegId::EAX, 4));
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-8), 4) });
    let load = Insn {
        symbol: Some(true),
        ..insn(
            148,
            Some((148, 148)),
            Some(sem(Operation::Move, "mov", vec![temporary.clone()], vec![cell])),
            vec![1],
            vec![],
        )
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
    let compare = insn(
        157,
        Some((157, 157)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])),
        vec![],
        vec![2],
    );
    let input = body(
        "matmul-anchored-scale",
        148,
        vec![
            block(
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
            )
            .into(),
        ],
    );

    let transformed = addresses(&input, "386").unwrap();
    let result = transformed.insns();

    assert_eq!(names(&result), ["mov", "", "", "", "lea", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
    assert!(result[1] == anchors[0] && result[3] == anchors[1]);
    assert!(result[2].symbol == Some(false) && result[2].defines == shift.defines);
    assert_eq!(result[4].symbol, Some(true));
    let Loc::Address(address) = &result[4].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
    assert_eq!((address.through, address.index_through, address.scale), (RegId::EAX, RegId::EBX, 2));
    assert!(verify::verify(&transformed, false).is_empty());
}

#[test]
fn test_loaded_scaled_add_preserves_a_shifted_value_live_into_a_successor() {
    // A block-local use count lost a shifted temporary read by its successor.
    let (temporary, total, saved) = (rl(RegId::EAX, 4), rl(RegId::EDX, 4), rl(RegId::ECX, 4));
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-4), 4) });
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
    let compare =
        insn(3, Some((3, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![total, im(0, 4)])), vec![], vec![2]);
    let preserve =
        insn(4, Some((4, 4)), Some(sem(Operation::Move, "mov", vec![saved], vec![temporary])), vec![3], vec![1]);
    let input = body(
        "live-scale",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(shift), Arc::new(add), Arc::new(compare)], vec![1]).into(),
            block(1, vec![Arc::new(preserve)], vec![]).into(),
        ],
    );

    let result = addresses(&input, "386").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["mov", "shl", "add", "cmp"]);
}

#[test]
fn test_affine_address_folds_the_longest_prefix_with_dead_flags() {
    // HOTLPX's `mov bx,cx; shl bx,2; add bx,cx; shl bx,1` keeps its last shift:
    // only a prefix whose flags nothing reads becomes one LEA; a zero-count
    // shift keeps the flags; `mov`+`shl bx,2` alone would need a longer
    // base-less form; ESP cannot be an index.
    for (guard, expected, scale) in [
        ("none", vec!["lea", "shl"], 4),
        ("dword", vec!["lea", "shl"], 4),
        ("carry", vec!["mov", "shl", "add", "adc"], 0),
        ("zero_shift", vec!["mov", "shl", "add", "shl"], 0),
        ("other_source", vec!["lea", "shl"], 4),
        ("same", vec!["mov", "shl", "add", "shl"], 0),
        ("stack", vec!["mov", "shl", "add", "shl"], 0),
    ] {
        let width = if guard == "dword" { 4 } else { 2 };
        let dest = rl(if width == 4 { RegId::EBX } else { RegId::BX }, width);
        let mut source = rl(if width == 4 { RegId::ECX } else { RegId::CX }, width);
        if guard == "same" {
            source = dest.clone();
        }
        if guard == "stack" {
            source = rl(RegId::SP, width);
        }
        let make = |at: i64, kind: Operation, name: &str, sources: Vec<Loc>| {
            Arc::new(insn(at, Some((at, at)), Some(sem(kind, name, vec![dest.clone()], sources)), vec![], vec![]))
        };
        let mut added = source.clone();
        if guard == "other_source" {
            added = rl(RegId::DX, 2);
        }
        let (last, count) = match guard {
            "carry" => ("adc", 1),
            "zero_shift" => ("shl", 0),
            _ => ("shl", 1),
        };
        let insns = vec![
            make(0, Operation::Move, "mov", vec![source.clone()]),
            make(1, Operation::Binary, "shl", vec![dest.clone(), im(2, 1)]),
            make(2, Operation::Binary, "add", vec![dest.clone(), added]),
            make(3, Operation::Binary, last, vec![dest.clone(), im(count, 1)]),
        ];
        let input = body("affine", 0, vec![block(0, insns, vec![]).into()]);

        let result = addresses(&input, "386").unwrap();

        assert_eq!(names(&result.blocks[0].insns), expected, "{guard}");
        if let Some(Loc::Address(address)) = result.blocks[0].insns[0].what.as_ref().unwrap().sources.first() {
            assert_eq!(address.scale, scale, "{guard}");
            assert_eq!(result.blocks[0].insns[0].what.as_ref().unwrap().dests, [dest], "{guard}");
        }
    }
}

#[test]
fn test_affine_address_folds_a_copy_offset_and_shift() {
    // deedlines' PLASMA kept `mov bx,dx; sub bx,-640; shl bx,1` per pixel once
    // regthrash moved the copy ahead of the SUB: bx = dx*2 + 1280.
    let (bx, dx) = (rl(RegId::BX, 2), rl(RegId::DX, 2));
    let make = |at: i64, kind: Operation, name: &str, sources: Vec<Loc>| {
        Arc::new(insn(at, Some((at, at)), Some(sem(kind, name, vec![bx.clone()], sources)), vec![], vec![]))
    };
    let insns = vec![
        make(0, Operation::Move, "mov", vec![dx.clone()]),
        make(1, Operation::Binary, "sub", vec![bx.clone(), im(-640, 2)]),
        make(2, Operation::Binary, "shl", vec![bx.clone(), im(1, 1)]),
        Arc::new(insn(
            3,
            Some((3, 3)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])),
            vec![],
            vec![],
        )),
    ];
    let input = body("plasma", 0, vec![block(0, insns, vec![]).into()]);

    let result = addresses(&input, "386").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["lea", "cmp"]);
    let Some(Loc::Address(address)) = result.blocks[0].insns[0].what.as_ref().unwrap().sources.first() else {
        panic!("not an address")
    };
    assert_eq!(
        (address.through, address.index_through, address.scale, address.offset),
        (RegId::EDX, RegId::EDX, 1, 1280)
    );
}

#[test]
fn test_affine_address_ignores_an_identity_read_only_before_it() {
    // Coalescing gave PLASMA's `sub` the identity dx held before it; counting
    // that earlier read as a later one refused the fold.
    let (bx, dx) = (rl(RegId::BX, 2), rl(RegId::DX, 2));
    let insns = vec![
        Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![dx.clone()], vec![im(7, 2)])),
            vec![5],
            vec![],
        )),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![dx.clone(), im(0, 2)])),
            vec![],
            vec![5],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![dx])),
            vec![8],
            vec![5],
        )),
        Arc::new(insn(
            3,
            Some((3, 3)),
            Some(sem(Operation::Binary, "sub", vec![bx.clone()], vec![bx.clone(), im(-640, 2)])),
            vec![5],
            vec![5],
        )),
        Arc::new(insn(
            4,
            Some((4, 4)),
            Some(sem(Operation::Binary, "shl", vec![bx.clone()], vec![bx.clone(), im(1, 1)])),
            vec![8],
            vec![8],
        )),
        Arc::new(insn(
            5,
            Some((5, 5)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![bx, im(0, 2)])),
            vec![],
            vec![8],
        )),
    ];
    let input = body("reach", 0, vec![block(0, insns, vec![]).into()]);

    let result = addresses(&input, "386").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["mov", "cmp", "lea", "cmp"]);
}

#[test]
fn test_affine_address_sees_flags_dead_past_its_block() {
    // Flags were taken as live at every block end, so a chain ending a block
    // stayed `mov`+`shl` though its successor overwrote them.
    let (bx, dx) = (rl(RegId::BX, 2), rl(RegId::DX, 2));
    let insns = vec![
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![dx])), vec![], vec![])),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "shl", vec![bx.clone()], vec![bx.clone(), im(1, 1)])),
            vec![],
            vec![],
        )),
    ];
    let next = vec![Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![bx, im(0, 2)])),
        vec![],
        vec![],
    ))];
    let input = body("edge", 0, vec![block(0, insns, vec![1]).into(), block(1, next, vec![]).into()]);

    let result = addresses(&input, "386").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["lea"]);
}

/// `mov ax,di; sub ax,4` stayed two instructions (5 bytes) at 486: the only LEA
/// the pass built was the 67h dword form, priced at its prefix. The word form
/// `lea ax,[di-4]` is 3 bytes and no prefix: QCport -Os had some 700 such
/// pairs.
#[test]
fn test_affine_address_is_a_word_lea_where_the_sum_is_a_real_mode_address() {
    for (label, source, then, expected, through, index, offset) in [
        ("base and offset", RegId::DI, ("sub", im(4, 2)), vec!["lea", "call"], RegId::DI, RegId::None, -4),
        ("base and index", RegId::BX, ("add", rl(RegId::SI, 2)), vec!["lea", "call"], RegId::BX, RegId::SI, 0),
        // CX names no word address: the dword form `lea ax,[ecx-4]`, which the
        // 486 prices at its two clocks and a prefix, as the pair.
        ("not an address register", RegId::CX, ("sub", im(4, 2)), vec!["lea", "call"], RegId::ECX, RegId::None, -4),
        // Two index registers are no word address either: the dword form, `lea
        // ax,[esi+edi]`, in the pair's bytes and clocks.
        ("two indexes", RegId::SI, ("add", rl(RegId::DI, 2)), vec!["lea", "call"], RegId::ESI, RegId::EDI, 0),
    ] {
        let ax = rl(RegId::AX, 2);
        let insns = vec![
            Arc::new(insn(
                0,
                Some((0, 0)),
                Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![rl(source, 2)])),
                vec![],
                vec![],
            )),
            Arc::new(insn(
                1,
                Some((1, 1)),
                Some(sem(Operation::Binary, then.0, vec![ax.clone()], vec![ax.clone(), then.1])),
                vec![],
                vec![],
            )),
            Arc::new(Insn {
                clobbers: BTreeSet::from([RegId::AX, RegId::CX, RegId::DX]),
                ..insn(2, Some((2, 2)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![])
            }),
        ];
        let input = body("word-lea", 0, vec![block(0, insns, vec![]).into()]);

        let result = addresses(&input, "486").unwrap();

        assert_eq!(names(&result.blocks[0].insns), expected, "{label}");
        if let Some(Loc::Address(address)) = result.blocks[0].insns[0].what.as_ref().unwrap().sources.first() {
            assert_eq!((address.through, address.index_through, address.offset), (through, index, offset), "{label}");
        }
    }
}

/// The word LEA keeps no flags: `mov ax,di; sub ax,4; adc ...` reads the
/// borrow, so stays.
#[test]
fn test_affine_address_word_lea_keeps_a_flag_still_read() {
    let (ax, di) = (rl(RegId::AX, 2), rl(RegId::DI, 2));
    let insns = vec![
        Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![di])), vec![], vec![])),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "sub", vec![ax.clone()], vec![ax.clone(), im(4, 2)])),
            vec![],
            vec![],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, "adc", vec![ax.clone()], vec![ax, im(0, 2)])),
            vec![],
            vec![],
        )),
    ];
    let input = body("word-lea-flags", 0, vec![block(0, insns, vec![]).into()]);

    let result = addresses(&input, "486").unwrap();

    assert_eq!(names(&result.blocks[0].insns), ["mov", "sub", "adc"]);
}

/// What the epilogue saves: the convention's preserved registers.
fn saved() -> Vec<RegId> {
    llrm_x86_m16::PRESERVED.iter().map(|(whole, _)| *whole).collect()
}

/// A store of a literal into a cell: `mov dword ptr [bp-N],0` is 8 bytes; `mov
/// [bp-N],eax` is 4.
fn literal_store(
    at: i64,
    width: u32,
    disp: i64,
    value: i64,
) -> Arc<Insn> {
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(disp), width) });
    Arc::new(insn(
        at,
        Some((at, at + 1)),
        Some(sem(Operation::Move, "mov", vec![cell], vec![im(value, width)])),
        vec![],
        vec![],
    ))
}

/// The function's remaining block, which reads `reads` and writes the flags
/// (`flags`) or reads them.
fn after_stores(
    reads_flags: bool,
    reads: &[RegId],
) -> Vec<Arc<Insn>> {
    let first = if reads_flags {
        sem(Operation::Binary, "adc", vec![rl(RegId::SI, 2)], vec![rl(RegId::SI, 2), im(0, 2)])
    } else {
        sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::SI, 2), im(0, 2)])
    };
    let mut insns = vec![Arc::new(insn(10, Some((10, 11)), Some(first), vec![], vec![]))];
    // Everything after the block is live (it has no successor), so what it does
    // not write first is read: the four scratch registers are written last,
    // except where `reads` first reads them.
    for (at, register) in reads.iter().enumerate() {
        let what = sem(Operation::Compare, "cmp", vec![], vec![rl(*register, 4), im(0, 4)]);
        insns.push(Arc::new(insn(11 + at as i64, Some((11 + at as i64, 12 + at as i64)), Some(what), vec![], vec![])));
    }
    for (at, register) in [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX].into_iter().enumerate() {
        let what = sem(Operation::Move, "mov", vec![rl(register, 4)], vec![im(1, 4)]);
        insns.push(Arc::new(insn(20 + at as i64, Some((20 + at as i64, 21 + at as i64)), Some(what), vec![], vec![])));
    }
    insns
}

/// QCport -Os kept `mov dword ptr [bp-8],0` twice (16 bytes) where a zeroed
/// register stores them in 3+4+4: some 270 runs. The literal's register must be
/// dead, and the zero is `xor` only where the flags are, else `mov`; one that
/// saves no byte (a lone word store) stays.
#[test]
fn test_stores_of_one_literal_share_a_dead_register_when_that_is_fewer_bytes() {
    let size = crate::backend::cpu::tuned("486", true).unwrap();
    let all = [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX];
    for (label, stores, flags_read, live, expected) in [
        ("two dwords, flags dead", vec![(4, -4), (4, -8)], false, vec![], vec!["xor", "mov", "mov"]),
        ("two dwords, flags read", vec![(4, -4), (4, -8)], true, vec![], vec!["mov", "mov", "mov"]),
        ("a dword and a word", vec![(4, -4), (2, -8)], false, vec![], vec!["xor", "mov", "mov"]),
        ("one dword", vec![(4, -4)], false, vec![], vec!["xor", "mov"]),
        ("one word saves nothing", vec![(2, -2)], false, vec![], vec!["mov"]),
        ("every scratch register read after", vec![(4, -4), (4, -8)], false, all.to_vec(), vec!["mov", "mov"]),
    ] {
        let mut insns: Vec<Arc<Insn>> =
            stores.iter().enumerate().map(|(at, (width, disp))| literal_store(at as i64, *width, *disp, 0)).collect();
        let next = after_stores(flags_read, &live);
        let input = body(
            "stores",
            0,
            vec![block(0, std::mem::take(&mut insns), vec![1]).into(), block(1, next, vec![]).into()],
        );

        let result = sharedstores::shared(&input, size, &saved(), &crate::backend::classes::RegisterClasses::m16());

        let got = names(&result.blocks[0].insns);
        assert_eq!(got, expected, "{label}");
    }
}

/// Which scratch register a run may take is the convention's fact
/// (`Target::callee_saved`): where it preserves BX, a run takes BX only if the
/// body already names it (so its epilogue saves it), else it would push and pop
/// a register to save a byte.
#[test]
fn test_a_shared_store_takes_no_register_the_convention_preserves_unless_the_body_names_it() {
    let size = crate::backend::cpu::tuned("486", true).unwrap();
    let input = |named: bool| {
        // BX is dead after the stores (the call clobbers it, nothing reads it);
        // the other scratch registers are read first. The body names BX
        // only where it writes it first.
        let mut insns = vec![literal_store(1, 4, -4, 0), literal_store(2, 4, -8, 0)];
        if named {
            let write = sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![im(5, 4)]);
            insns.insert(0, Arc::new(insn(0, Some((0, 1)), Some(write), vec![], vec![])));
        }
        let mut next = after_stores(false, &[RegId::EAX, RegId::ECX, RegId::EDX]);
        next.truncate(4);
        next.push(Arc::new(Insn {
            clobbers: BTreeSet::from([RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX]),
            ..insn(40, Some((40, 41)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![])
        }));
        body("stores", 0, vec![block(0, insns, vec![1]).into(), block(1, next, vec![]).into()])
    };
    let preserving_bx = [RegId::ESI, RegId::EDI, RegId::EBX];

    assert_eq!(
        names(
            &sharedstores::shared(
                &input(false),
                size,
                &preserving_bx,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .blocks[0]
                .insns
        ),
        ["mov", "mov"]
    );
    assert_eq!(
        names(
            &sharedstores::shared(&input(false), size, &saved(), &crate::backend::classes::RegisterClasses::m16())
                .blocks[0]
                .insns
        ),
        ["xor", "mov", "mov"]
    );
    assert_eq!(
        names(
            &sharedstores::shared(&input(true), size, &preserving_bx, &crate::backend::classes::RegisterClasses::m16())
                .blocks[0]
                .insns
        ),
        ["mov", "xor", "mov", "mov"]
    );
}

/// Two pushes of a clock are not a size choice: -O2 keeps the stores.
#[test]
fn test_stores_of_one_literal_are_not_shared_at_o2() {
    let o2 = crate::backend::cpu::tuned("486", false).unwrap();
    let input = body(
        "stores",
        0,
        vec![
            block(0, vec![literal_store(0, 4, -4, 0), literal_store(1, 4, -8, 0)], vec![1]).into(),
            block(1, after_stores(false, &[]), vec![]).into(),
        ],
    );

    assert_eq!(sharedstores::shared(&input, o2, &saved(), &crate::backend::classes::RegisterClasses::m16()), input);
}

#[test]
fn test_lea_of_a_frame_cell_is_decoded() {
    let regs = crate::backend::registerinfo::test_regs();
    // `lea ax,[bp-18]` read as touching every lane kept all upper halves live
    // across the B$ERAS calls after a loop.
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-18), 2) });
    let lea =
        insn(0, Some((0, 0)), Some(sem(Operation::Address, "lea", vec![rl(RegId::AX, 2)], vec![cell])), vec![], vec![]);

    let (reads, writes) = _register_effects(regs, 16, &lea, false, true).expect("decoded");

    assert!(reads.is_subset(&_lanes(regs, RegId::EBP)));
    assert!(_lanes(regs, RegId::AX).is_subset(&writes) && writes.and(&_lanes(regs, RegId::EBX)).is_empty());
}

#[test]
fn test_zeroing_requires_flags_overwritten_before_observation() {
    // HARR-style zeroing is safe before CMP, but not before a carry consumer.
    for (following, zeroed) in
        [("cmp", true), ("add", true), ("adc", false), ("inc", false), ("shl", false), ("call", false), ("je", false)]
    {
        let dest = rl(RegId::AX, 2);
        let first = insn(
            0,
            Some((0, 3)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![im(0, 2)])),
            vec![],
            vec![],
        );
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
        let result = transform(body("zero", 0, vec![block(0, vec![Arc::new(first), Arc::new(last)], vec![]).into()]));
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
            1 => RegId::AL,
            2 => RegId::AX,
            _ => RegId::EAX,
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
            middle.clobbers = [RegId::EAX].into();
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
        let result = zeroes(&body("zero", 0, crate::model::lir::shared(blocks))).insns()[0].clone();
        // "boundary": the next block's cmp overwrites every flag before
        // anything reads one.
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
    // `mov bx,0; jmp` stayed three bytes: every block end was taken to have its
    // flags read.
    for (successor, zeroed) in [(Some("cmp"), true), (Some("jb"), false), (None, false)] {
        let dest = rl(RegId::BX, 2);
        let zero = insn(
            0,
            Some((0, 3)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![im(0, 2)])),
            vec![],
            vec![],
        );
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
        let result = zeroes(&body("zero", 0, crate::model::lir::shared(blocks))).insns()[0].clone();
        assert_eq!(result.what.as_ref().unwrap().name.as_deref(), Some(if zeroed { "xor" } else { "mov" }));
    }
}

/// `mov ecx, 0; mov edx, 64; jmp _chop` (a sibling call) stayed five bytes: the
/// jump that returns was taken to read the flags, which the pin's `TAILJMPd`
/// does not (m32 -O2 recchop, +3 B over the pin's `xor`).
#[test]
fn test_a_zero_before_a_sibling_call_is_xor() {
    let (ecx, edx) = (rl(RegId::ECX, 4), rl(RegId::EDX, 4));
    let insns = vec![
        Arc::new(insn(
            0,
            Some((0, 5)),
            Some(sem(Operation::Move, "mov", vec![ecx.clone()], vec![im(0, 4)])),
            vec![],
            vec![],
        )),
        Arc::new(insn(5, Some((5, 10)), Some(sem(Operation::Move, "mov", vec![edx], vec![im(64, 4)])), vec![], vec![])),
        Arc::new(insn(
            10,
            Some((10, 15)),
            Some(semt(Operation::Return, "jmp", vec![], vec![], Some(99))),
            vec![],
            vec![],
        )),
    ];
    let result =
        zeroes(&body("sibling", 0, crate::model::lir::shared(vec![block(0, insns, vec![])]))).insns()[0].clone();
    assert_eq!(result.what.as_ref().unwrap().name.as_deref(), Some("xor"));
}

#[test]
fn test_zero_compare_before_its_branch_is_test() {
    // `cmp ax,0; jl` is three bytes where `test ax,ax; jl` is two, and writes
    // no register as `or ax,ax` did. Only AF differs. The branch need not end
    // its block: `-ftree-ch` left `cmp ecx, 0; je` before a jump.
    for (variant, rewritten) in [
        ("plain", true),
        ("jumped", true),
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
        let ax = rl(RegId::AX, 2);
        let tested = if variant == "memory" { Loc::Mem(mem(frame(-2), 2, RegId::BP, 0, 2)) } else { ax.clone() };
        let compare =
            insn(0, Some((0, 3)), Some(sem(Operation::Compare, "cmp", vec![], vec![tested, im(0, 2)])), vec![], vec![]);
        let moved = sem(Operation::Move, "mov", vec![rl(RegId::BX, 2)], vec![rl(RegId::CX, 2)]);
        let between = insn(3, Some((3, 5)), if variant == "between" { None } else { Some(moved) }, vec![], vec![]);
        let branch =
            insn(6, Some((6, 8)), Some(semt(Operation::Branch, "jl", vec![], vec![], Some(9))), vec![], vec![]);
        let after = sem(Operation::Compare, "cmp", vec![], vec![ax.clone(), im(1, 2)]);
        let symbol =
            Loc::Imm(Imm { value: 0, width: 2, address: Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }) });
        let last = match variant {
            "adjust" => None,
            "call" => Some(sem(Operation::Call, "call", vec![], vec![])),
            "return" => Some(sem(Operation::Return, "", vec![], vec![])),
            "relocated" => Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![symbol.clone()])),
            "x87" => Some(sem(Operation::Compare, "fcomp", vec![], vec![Loc::Mem(mem(frame(-4), 4, RegId::BP, 0, 2))])),
            "pushf" => Some(sem(Operation::Nothing, "pushf", vec![], vec![symbol])),
            _ => Some(after.clone()),
        };
        let compare = Arc::new(compare);
        let jump = insn(7, Some((7, 8)), Some(semt(Operation::Jump, "jmp", vec![], vec![], Some(9))), vec![], vec![]);
        let head = if ["moves", "between"].contains(&variant) {
            vec![Arc::clone(&compare), Arc::new(between), Arc::new(branch)]
        } else if variant == "jumped" {
            vec![Arc::clone(&compare), Arc::new(branch), Arc::new(jump)]
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
        let result = zero_compares(rules(), &body("zero", 0, crate::model::lir::shared(blocks))).insns()[0].clone();
        let what = result.what.as_ref().unwrap();
        if rewritten {
            assert_eq!(
                (what.op, what.name.as_deref(), &what.dests, &what.sources),
                (Operation::Compare, Some("test"), &Vec::new(), &vec![ax.clone(), ax]),
                "{variant}"
            );
        } else {
            assert_eq!(result.what, compare.what, "{variant}");
        }
    }
}

/// A loop counted up to zero kept `add bx,2; cmp bx,0; jne` where its
/// branch was followed by a jump to the exit: the add's flags were the test.
#[test]
fn test_a_zero_test_before_a_branch_and_a_jump_reads_the_step() {
    let bx = rl(RegId::BX, 2);
    let step = insn(
        0,
        Some((0, 3)),
        Some(sem(Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), im(2, 2)])),
        vec![],
        vec![],
    );
    let compare =
        insn(3, Some((3, 6)), Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![]);
    let branch = insn(6, Some((6, 8)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(0))), vec![], vec![]);
    let jump = insn(8, Some((8, 10)), Some(semt(Operation::Jump, "jmp", vec![], vec![], Some(10))), vec![], vec![]);
    let ax = rl(RegId::AX, 2);
    let flags = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "xor", vec![ax.clone()], vec![ax.clone(), ax])),
        vec![],
        vec![],
    );
    let exit = insn(12, Some((12, 13)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]);
    let blocks = vec![
        block(0, vec![Arc::new(step), Arc::new(compare), Arc::new(branch), Arc::new(jump)], vec![0, 10]),
        block(10, vec![Arc::new(flags), Arc::new(exit)], vec![]),
    ];
    let names = tested(rules(), &body("zero", 0, crate::model::lir::shared(blocks))).blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().and_then(|what| what.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(names, ["add", "", "jne", "jmp"]);
}

/// queens -O3 kept `sub eax,[esp+20]; cmp eax,0; je` where the spiller had
/// folded the subtrahend into the sub: 5,676 executed `test eax,eax`, 5,676 of
/// the row's 13,766 extra clocks. The sub's flags are the test whatever its
/// source, where no instruction stands between.
#[test]
fn test_a_zero_test_after_a_step_with_a_memory_source_reads_the_step() {
    let bx = rl(RegId::BX, 2);
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-10), 2) });
    let step = insn(
        0,
        Some((0, 3)),
        Some(sem(Operation::Binary, "sub", vec![bx.clone()], vec![bx.clone(), cell])),
        vec![],
        vec![],
    );
    let compare =
        insn(3, Some((3, 6)), Some(sem(Operation::Compare, "cmp", vec![], vec![bx, im(0, 2)])), vec![], vec![]);
    let branch = insn(6, Some((6, 8)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(0))), vec![], vec![]);
    let ax = rl(RegId::AX, 2);
    let flags = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "xor", vec![ax.clone()], vec![ax.clone(), ax])),
        vec![],
        vec![],
    );
    let exit = insn(12, Some((12, 13)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]);
    let blocks = vec![
        block(0, vec![Arc::new(step), Arc::new(compare), Arc::new(branch)], vec![0, 10]),
        block(10, vec![Arc::new(flags), Arc::new(exit)], vec![]),
    ];
    let names = tested(rules(), &body("zero", 0, crate::model::lir::shared(blocks))).blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().and_then(|what| what.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(names, ["sub", "", "je"]);
}

/// rcflip's RAMP kept `add si,2; cmp si,0; jne`: an anchor carrying a
/// phi's value sat between the test and its branch.
#[test]
fn test_a_zero_test_before_an_anchor_and_its_branch_reads_the_step() {
    let bx = rl(RegId::BX, 2);
    let step = insn(
        0,
        Some((0, 3)),
        Some(sem(Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), im(2, 2)])),
        vec![],
        vec![],
    );
    let compare =
        insn(3, Some((3, 6)), Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![]);
    let anchor = insn(6, None, Some(sem(Operation::Nothing, "", vec![], vec![])), vec![7], vec![5]);
    let branch = insn(6, Some((6, 8)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(0))), vec![], vec![]);
    let ax = rl(RegId::AX, 2);
    let flags = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "xor", vec![ax.clone()], vec![ax.clone(), ax])),
        vec![],
        vec![],
    );
    let exit = insn(12, Some((12, 13)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]);
    let blocks = vec![
        block(0, vec![Arc::new(step), Arc::new(compare), Arc::new(anchor), Arc::new(branch)], vec![0, 10]),
        block(10, vec![Arc::new(flags), Arc::new(exit)], vec![]),
    ];
    let names = tested(rules(), &body("zero", 0, crate::model::lir::shared(blocks))).blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().and_then(|what| what.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(names, ["add", "", "", "jne"]);
}

#[test]
fn test_register_round_trip_through_memory_is_one_instruction() {
    // `mov bx,[bp-4]; add bx,1; mov [bp-4],bx` where bcc writes `add word ptr
    // [bp-4],1`. Python printed these through `masm`; the reprs are of the
    // semantics it printed. "unsigned byte" is not ported: it fails against
    // the Python implementation too.
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
            Some(vec![printed(
                binary,
                "add",
                format!("({word},)"),
                format!("({word}, Imm(value=1, width=2, address=None))"),
            )]),
        ),
        (
            "register",
            Some(vec![printed(binary, "add", format!("({word},)"), format!("({word}, Reg(register=22, width=2))"))]),
        ),
        ("unary", Some(vec![printed("<Operation.UNARY: 'unary'>", "neg", format!("({word},)"), format!("({word},)"))])),
        (
            "compare",
            Some(vec![
                printed(cmp, "cmp", "()".to_owned(), format!("({word}, Imm(value=5, width=2, address=None))")),
                jl.to_owned(),
            ]),
        ),
        (
            "zero",
            Some(vec![
                printed(cmp, "cmp", "()".to_owned(), format!("({word}, Imm(value=0, width=2, address=None))")),
                jl.to_owned(),
            ]),
        ),
        (
            "signed byte",
            Some(vec![
                printed(cmp, "cmp", "()".to_owned(), format!("({byte}, Imm(value=0, width=1, address=None))")),
                jl.to_owned(),
            ]),
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
        let (bx, cx) = (rl(RegId::BX, 2), rl(RegId::CX, 2));
        let mut cell = mem(frame(-4), if extension.is_some() { 1 } else { 2 }, RegId::BP, 0, 2);
        if variant == "addressed" {
            cell = mem(None, 2, RegId::BX, 2, 1);
        }
        let cell = Loc::Mem(cell);
        let spans = if variant == "bytes" { Some((0, 3)) } else { None };
        let make =
            |at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, target: Option<i64>, covers| {
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
        let reread = make(8, Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![bx.clone()], None, None);
        let rewrite = make(8, Operation::Move, "mov", vec![bx], vec![cx], None, None);
        // Back to the top rather than a return: liveness reads a return as
        // reading every register.
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
        let result = fused(rules(), &body("fused", 0, crate::model::lir::shared(blocks))).blocks[0].clone();
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
    // ls_animate emitted MOV AX,[bp-16]; OR AX,AX although BCC compares the
    // slot.
    let (bx, cx) = (rl(RegId::BX, 2), rl(RegId::CX, 2));
    let cell = Loc::Mem(mem(frame(-4), 2, RegId::BP, 0, 2));
    let load = Insn {
        spill_reload: true,
        ..insn(1, None, Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![cell.clone()])), vec![2], vec![])
    };
    let anchor = insn(2, None, Some(sem(Operation::Nothing, "", vec![], vec![])), vec![3], vec![2]);
    let compare =
        insn(3, None, Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![3]);
    let branch = insn(4, None, Some(semt(Operation::Branch, "je", vec![], vec![], Some(2))), vec![], vec![]);
    let overwrite = Arc::new(insn(5, None, Some(sem(Operation::Move, "mov", vec![bx], vec![cx])), vec![], vec![]));
    let input = body(
        "anchored-fusion",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(anchor), Arc::new(compare), Arc::new(branch)], vec![1, 2]).into(),
            block(1, vec![Arc::clone(&overwrite)], vec![]).into(),
            block(2, vec![overwrite], vec![]).into(),
        ],
    );

    let done = fused(rules(), &input);

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
    // qcport's mdl_ai lost value#47 after fusion removed the function pointer
    // load.
    let (bx, cx) = (rl(RegId::BX, 2), rl(RegId::CX, 2));
    let cell = Loc::Mem(mem(frame(-4), 2, RegId::BP, 0, 2));
    let load = insn(1, None, Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![cell])), vec![47], vec![]);
    let compare =
        insn(2, None, Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(0, 2)])), vec![], vec![47]);
    let branch = insn(3, None, Some(semt(Operation::Branch, "je", vec![], vec![], Some(2))), vec![], vec![]);
    let call = Insn {
        clobbers: [RegId::EBX].into(),
        ..insn(4, None, Some(sem(Operation::Call, "call", vec![], vec![bx.clone()])), vec![], vec![47])
    };
    let overwrite = insn(5, None, Some(sem(Operation::Move, "mov", vec![bx], vec![cx])), vec![], vec![]);
    let input = body(
        "indirect-call-liveness",
        0,
        vec![
            block(0, vec![Arc::new(load), Arc::new(compare), Arc::new(branch)], vec![1, 2]).into(),
            block(1, vec![Arc::new(call)], vec![]).into(),
            block(2, vec![Arc::new(overwrite)], vec![]).into(),
        ],
    );

    let done = fused(rules(), &input);

    assert!(verify::verify(&done, false).is_empty());
}

#[test]
fn test_far_pointer_loaded_in_one_instruction() {
    // `mov bx,[bp-8]; mov es,[bp-6]` where bcc writes `les bx,[bp-8]`.
    // Python printed these through `masm`; the reprs are of the semantics it
    // printed.
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
        let bx = rl(RegId::BX, 2);
        let segment = rl(if variant == "fs" { RegId::FS } else { RegId::ES }, 2);
        let mut low = mem(frame(-8), 2, RegId::BP, 0, 2);
        let mut high = mem(frame(if variant == "another cell" { -4 } else { -6 }), 2, RegId::BP, 0, 2);
        let mut base = RegId::BP;
        if variant.starts_with("through the offset") {
            (low, high) = (mem(None, 2, RegId::BX, 0, 1), mem(None, 2, RegId::BX, 2, 1));
            base = RegId::BX;
        }
        if variant.starts_with("override") {
            // As lowered and allocated: the offset value placed in si.
            let far = |disp: i64| Mem {
                base: Some(Held { value: 1, width: 2 }),
                ..mem(Some(Addr { segment: (RegId::ES).iced(), ..Addr::new(Space::Far, disp) }), 2, RegId::SI, 0, 2)
            };
            (low, high) = (far(16), far(18));
            base = RegId::SI;
        }
        let moved = |at: i64, dest: Loc, cell: Mem| {
            Arc::new(insn(
                at,
                None,
                Some(sem(Operation::Move, "mov", vec![dest], vec![Loc::Mem(cell)])),
                vec![],
                vec![],
            ))
        };
        let mut pair = vec![moved(0, bx, low), moved(1, segment, high)];
        if variant.ends_with("segment first") {
            pair.reverse();
        }
        let result =
            far_loads(rules(), &body("far", 0, vec![block(0, pair.clone(), vec![]).into()])).blocks[0].insns.clone();
        let Some(printed) = printed else {
            assert_eq!(result, pair, "{variant}");
            continue;
        };
        let found: Vec<String> = result.iter().map(|one| one.what.as_ref().unwrap().repr()).collect();
        assert_eq!(found, [printed], "{variant}");
        let made = code(result[0].what.as_ref().unwrap());
        let decoded = Decoder::new(16, &made, DecoderOptions::NONE).decode();
        let expected = if variant == "fs" { Mnemonic::Lfs } else { Mnemonic::Les };
        assert_eq!(
            (decoded.mnemonic(), decoded.op0_register(), decoded.memory_base()),
            (expected, iced_x86::Register::BX, base.iced())
        );
    }
}

#[test]
fn test_repeated_copy_requires_unchanged_source_and_destination() {
    // LNGMXX copied ECX into EAX twice around CDQ; partial writes must prevent
    // reuse.
    for change in [None, Some(RegId::CH), Some(RegId::AH)] {
        let moved = insn(
            0,
            Some((0, 1)),
            Some(sem(Operation::Move, "mov", vec![rl(RegId::EAX, 4)], vec![rl(RegId::ECX, 4)])),
            vec![],
            vec![],
        );
        let mut extend = insn(
            1,
            Some((1, 2)),
            Some(sem(Operation::Extend, "cdq", vec![rl(RegId::EDX, 4)], vec![rl(RegId::EAX, 4)])),
            vec![],
            vec![],
        );
        if let Some(change) = change {
            extend.clobbers = [change].into();
        }
        let last = Insn { at: 2, covers: Some((2, 3)), ..moved.clone() };
        let input = body(
            "copies",
            0,
            vec![block(0, vec![Arc::new(moved.clone()), Arc::new(extend), Arc::new(last)], vec![]).into()],
        );
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
    let copy = |at: i64, dest: RegId, source: Loc| {
        Arc::new(insn(
            at,
            Some((at, at + 1)),
            Some(sem(Operation::Move, "mov", vec![rl(dest, 4)], vec![source])),
            vec![],
            vec![],
        ))
    };
    let insns = vec![
        copy(0, RegId::EAX, rl(RegId::ECX, 4)),
        copy(1, RegId::EDX, rl(RegId::ECX, 4)),
        copy(2, RegId::ECX, im(7, 4)),
        copy(3, RegId::EAX, rl(RegId::EDX, 4)),
        copy(4, RegId::EAX, rl(RegId::ECX, 4)),
    ];
    let result = constants(&body("snapshot", 0, vec![block(0, insns.clone(), vec![]).into()]));
    let expected: Vec<Semantics> =
        insns.iter().filter(|one| one.at != 3).map(|one| one.what.clone().unwrap()).collect();
    assert_eq!(whats(&result.insns()), expected);
}

/// `mov ax, 1; movzx eax, ax` is `mov eax, 1`: the extension of a register
/// holding a constant is the extended constant (dos32 BASIC passed every
/// INTEGER constant to a runtime call so).
#[test]
fn test_the_extension_of_a_register_holding_a_constant_is_the_extended_constant() {
    let moved = |at: i64, dest: Loc, source: Loc| {
        Arc::new(insn(
            at,
            Some((at, at + 1)),
            Some(sem(Operation::Move, "mov", vec![dest], vec![source])),
            vec![],
            vec![],
        ))
    };
    let extended = |at: i64, name: &str, dest: Loc, source: Loc| {
        Arc::new(insn(
            at,
            Some((at, at + 1)),
            Some(sem(Operation::Extend, name, vec![dest], vec![source])),
            vec![],
            vec![],
        ))
    };
    let after =
        |input: Vec<Arc<Insn>>| whats(&constants(&body("extension", 0, vec![block(0, input, vec![]).into()])).insns());
    let mov32 = |register: RegId, value: i64| sem(Operation::Move, "mov", vec![rl(register, 4)], vec![im(value, 4)]);
    // Zero extension of a word, in place and into another register.
    let narrow = moved(0, rl(RegId::AX, 2), im(1, 2));
    assert_eq!(
        after(vec![narrow.clone(), extended(1, "movzx", rl(RegId::EAX, 4), rl(RegId::AX, 2))])[1],
        mov32(RegId::EAX, 1)
    );
    assert_eq!(
        after(vec![narrow.clone(), extended(1, "movzx", rl(RegId::ECX, 4), rl(RegId::AX, 2))])[1],
        mov32(RegId::ECX, 1)
    );
    // A byte's sign: -3 is 0xfffffffd, 200 as a byte is 200 whether or not it
    // is negative to `movsx`.
    let byte = moved(0, rl(RegId::AL, 1), im(-3, 1));
    assert_eq!(
        after(vec![byte.clone(), extended(1, "movsx", rl(RegId::EAX, 4), rl(RegId::AL, 1))])[1],
        mov32(RegId::EAX, -3)
    );
    assert_eq!(after(vec![byte, extended(1, "movzx", rl(RegId::EAX, 4), rl(RegId::AL, 1))])[1], mov32(RegId::EAX, 253));
    // The low word of a whole register, and nothing once the source is written
    // between.
    let whole = moved(0, rl(RegId::EAX, 4), im(0x1_0005, 4));
    assert_eq!(after(vec![whole, extended(1, "movzx", rl(RegId::ECX, 4), rl(RegId::AX, 2))])[1], mov32(RegId::ECX, 5));
    let unknown = moved(1, rl(RegId::AX, 2), rl(RegId::DX, 2));
    let kept = after(vec![narrow, unknown, extended(2, "movzx", rl(RegId::EAX, 4), rl(RegId::AX, 2))]);
    assert_eq!(kept[2].name.as_deref(), Some("movzx"));
}

#[test]
fn test_partial_write_invalidates_constant() {
    let moved = |at: i64, dest: Loc, source: Loc| {
        Arc::new(insn(
            at,
            Some((at, at + 1)),
            Some(sem(Operation::Move, "mov", vec![dest], vec![source])),
            vec![],
            vec![],
        ))
    };
    let first = moved(0, rl(RegId::EAX, 4), im(512, 4));
    let change = moved(1, rl(RegId::AH, 1), im(0, 1));
    let again = moved(2, rl(RegId::EAX, 4), im(512, 4));
    let input = body("partial", 0, vec![block(0, vec![first, change, again], vec![]).into()]);
    assert_eq!(constants(&input).blocks[0].insns.len(), 3);
}

#[test]
fn test_empty_ownership_marker_preserves_register_knowledge() {
    // Expanded FPDEEP emitted MOV AX,0 twice, separated only by a removed
    // instruction's marker.
    for clobbers in [vec![], vec![RegId::AX]] {
        let what = sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![im(0, 2)]);
        let first = insn(0, Some((0, 3)), Some(what.clone()), vec![], vec![]);
        let marker = Insn {
            clobbers: clobbers.iter().copied().collect(),
            ..insn(3, Some((3, 5)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])
        };
        let last = Insn { at: 5, covers: Some((5, 8)), ..first.clone() };
        let input =
            body("marker", 0, vec![block(0, vec![Arc::new(first), Arc::new(marker), Arc::new(last)], vec![]).into()]);
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
    let moved = sem(Operation::Move, "mov", vec![rl(RegId::EAX, 4)], vec![im(512, 4)]);
    let first = insn(0, Some((0, 0)), Some(moved.clone()), vec![1], vec![]);
    let extend = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Extend, "cdq", vec![rl(RegId::EDX, 4)], vec![rl(RegId::EAX, 4)])),
        vec![2],
        vec![1],
    );
    let marker = insn(2, Some((2, 2)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![3], vec![1]);
    let again = insn(3, Some((3, 3)), Some(moved.clone()), vec![4], vec![]);
    let input = body(
        "nbody-dividend",
        0,
        vec![block(0, vec![Arc::new(first), Arc::new(extend), Arc::new(marker), Arc::new(again)], vec![]).into()],
    );

    let result = constants(&input);

    assert_eq!(result.insns().iter().filter(|one| one.what.as_ref() == Some(&moved)).count(), 1);
    assert!(verify::verify(&result, false).is_empty());
}

#[test]
fn test_qlight_folds_batched_parameter_loads_into_their_extensions() {
    // The QB frontend loaded both parameters before widening either one.
    let (ax, cx, eax, ebx) = (rl(RegId::AX, 2), rl(RegId::CX, 2), rl(RegId::EAX, 4), rl(RegId::EBX, 4));
    let left = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(6), 2) });
    let right = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(8), 2) });
    let insns = vec![
        Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![left.clone()])),
            vec![1],
            vec![],
        )),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![cx.clone()], vec![right.clone()])),
            vec![2],
            vec![],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Extend, "movsx", vec![ebx.clone()], vec![ax])),
            vec![3],
            vec![1],
        )),
        Arc::new(insn(
            3,
            Some((3, 3)),
            Some(sem(Operation::Extend, "movsx", vec![eax.clone()], vec![cx])),
            vec![4],
            vec![2],
        )),
    ];
    let input = body("qlight-parameters", 0, vec![block(0, insns, vec![]).into()]);

    let real = whats(&extensions(rules(), &input).insns());

    assert_eq!(
        real,
        [
            sem(Operation::Extend, "movsx", vec![ebx], vec![left]),
            sem(Operation::Extend, "movsx", vec![eax], vec![right]),
        ]
    );
}

#[test]
fn test_transitive_extension_preserves_nonlocal_machine_state() {
    for guard in ["signedness", "register", "shared", "clobber"] {
        let cell = Loc::Mem(Mem { through: RegId::BX, ..Mem::new(None, 1) });
        let mut narrow = insn(
            0,
            Some((0, 3)),
            Some(sem(Operation::Extend, "movzx", vec![rl(RegId::DX, 2)], vec![cell])),
            vec![1],
            vec![],
        );
        let mut wide = insn(
            3,
            Some((3, 6)),
            Some(sem(Operation::Extend, "movzx", vec![rl(RegId::EDX, 4)], vec![rl(RegId::DX, 2)])),
            vec![2],
            vec![1],
        );
        let mut tail = vec![];
        match guard {
            "signedness" => wide.what.as_mut().unwrap().name = Some("movsx".into()),
            "register" => wide.what.as_mut().unwrap().dests = vec![rl(RegId::EAX, 4)],
            "shared" => {
                tail.push(Arc::new(insn(
                    6,
                    Some((6, 6)),
                    Some(sem(Operation::Push, "push", vec![], vec![rl(RegId::DX, 2)])),
                    vec![],
                    vec![1],
                )));
            }
            _ => narrow.clobbers = [RegId::AX].into(),
        }
        let input =
            body("guarded", 0, vec![block(0, [vec![Arc::new(narrow), Arc::new(wide)], tail].concat(), vec![]).into()]);
        assert_eq!(extensions(rules(), &input), input, "{guard}");
    }
}

/// Unrolling marked every clone symbolic; matmul then kept seven of its eight
/// `mov bx,[m]; movsx ebx,bx` pairs unfolded in the hot loop.
#[test]
fn test_register_only_extension_clone_does_not_claim_a_relocation() {
    let cell = Loc::Mem(Mem { through: RegId::BX, ..Mem::new(None, 1) });
    let mut narrow = insn(
        0,
        Some((0, 3)),
        Some(sem(Operation::Extend, "movzx", vec![rl(RegId::DX, 2)], vec![cell.clone()])),
        vec![1],
        vec![],
    );
    narrow.symbol = Some(true);
    let mut wide = insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Extend, "movzx", vec![rl(RegId::EDX, 4)], vec![rl(RegId::DX, 2)])),
        vec![2],
        vec![1],
    );
    wide.symbol = Some(true);
    let input = body("cloned-extension", 0, vec![block(0, vec![Arc::new(narrow), Arc::new(wide)], vec![]).into()]);

    let result = extensions(rules(), &input).insns();

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
            source = Loc::Imm(Imm {
                value: 512,
                width: 4,
                address: Some(Addr { index: 5, ..Addr::new(Space::Segment, 0) }),
            });
        }
        let what = sem(Operation::Move, "mov", vec![rl(RegId::EAX, 4)], vec![source]);
        let first = insn(0, Some((0, 1)), Some(what.clone()), vec![], vec![]);
        let last = Insn { at: 2, covers: Some((2, 3)), ..first.clone() };
        let mut middle = insn(
            1,
            Some((1, 2)),
            Some(sem(Operation::Move, "mov", vec![rl(RegId::BX, 2)], vec![im(7, 2)])),
            vec![],
            vec![],
        );
        if interruption == "call" {
            middle.what = Some(sem(Operation::Call, "call", vec![], vec![]));
        }
        if ["extend", "extend_clobber"].contains(&interruption) {
            middle.what = Some(sem(Operation::Extend, "cdq", vec![rl(RegId::EDX, 4)], vec![rl(RegId::EAX, 4)]));
            if interruption == "extend_clobber" {
                middle.clobbers = [RegId::AH].into();
            }
        }
        if interruption == "extend_write" {
            middle.what = Some(sem(Operation::Extend, "movsx", vec![rl(RegId::EAX, 4)], vec![rl(RegId::AX, 2)]));
        }
        if interruption == "clobber" {
            middle.clobbers = [RegId::EAX].into();
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
        let result = constants(&body("constants", 0, crate::model::lir::shared(blocks)));
        assert_eq!(
            result.insns().iter().filter(|one| one.what.as_ref() == Some(&what)).count(),
            // A movsx of the held constant is that constant again, so the later
            // copy is redundant.
            if ["none", "extend", "extend_write"].contains(&interruption) { 1 } else { 2 },
            "{interruption}"
        );
    }
}

#[test]
fn test_a_string_fill_reading_the_direction_flag_leaves_zero_as_xor() {
    // `rep stosb` reads DF; xor leaves DF as it is.
    let (ax, bx) = (rl(RegId::AX, 2), rl(RegId::BX, 2));
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
    let result = zeroes(&body("zero", 0, crate::model::lir::shared(blocks))).blocks[0].insns[0].clone();
    let what = result.what.as_ref().unwrap();
    assert_eq!((what.op, what.name.as_deref()), (Operation::Binary, Some("xor")));
}

// ----------------------------------------------------------
// test_machine_copyprop

fn moved(
    at: i64,
    dest: Loc,
    source: Loc,
) -> Insn {
    insn(at, Some((at, at)), Some(sem(Operation::Move, "mov", vec![dest], vec![source])), vec![], vec![])
}

#[test]
fn test_copy_source_is_forwarded_to_an_explicit_use() {
    // ls_face_key spent `mov bx,ax` only to compare BX; the loop paid one
    // instruction each trip.
    let (ax, bx) = (rl(RegId::AX, 2), rl(RegId::BX, 2));
    let copied = moved(0, bx.clone(), ax.clone());
    let compared = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), im(116, 2)])),
        vec![],
        vec![],
    );
    let overwritten = moved(2, bx, im(7, 2));
    let result = transform(body(
        "ls_face_key",
        0,
        vec![
            block(0, vec![Arc::new(copied.clone()), Arc::new(compared.clone()), Arc::new(overwritten)], vec![]).into(),
        ],
    ));

    let insns = result.insns();
    assert!(!insns.iter().any(|one| one.what == copied.what));
    assert!(insns.iter().any(|one| one.at == compared.at && one.what.as_ref().unwrap().sources[0] == ax));
}

#[test]
fn test_copy_forwarding_does_not_rename_a_fixed_register_use() {
    // GCC/LLVM both recheck constraints: CWD still reads AX even when AX is a
    // copy of BX.
    let (ax, bx, dx) = (rl(RegId::AX, 2), rl(RegId::BX, 2), rl(RegId::DX, 2));
    let copied = moved(0, ax.clone(), bx);
    let fixed = insn(1, Some((1, 1)), Some(sem(Operation::Extend, "cwd", vec![dx], vec![ax.clone()])), vec![], vec![]);
    let result =
        transform(body("fixed", 0, vec![block(0, vec![Arc::new(copied), Arc::new(fixed.clone())], vec![]).into()]));
    let found = result.insns().iter().find(|one| one.at == fixed.at).unwrap().what.as_ref().unwrap().sources[0].clone();
    assert_eq!(found, ax);
}

// ------------------------------------------------------------
// test_memory_folding

fn plain(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    target: Option<i64>,
) -> Arc<Insn> {
    Arc::new(insn(at, None, Some(semt(op, name, dests, sources, target)), vec![], vec![]))
}

fn delayed(
    head: Vec<Arc<Insn>>,
    eax: &Loc,
    ecx: &Loc,
) -> LirBody {
    let overwrite = plain(4, Operation::Move, "mov", vec![eax.clone()], vec![ecx.clone()], None);
    let jump = plain(5, Operation::Jump, "jmp", vec![], vec![], Some(0));
    body(
        "delayed-memory-read",
        0,
        vec![block(0, head, vec![1]).into(), block(1, vec![overwrite, jump], vec![0]).into()],
    )
}

#[test]
fn test_memory_round_trip_folds_across_an_independent_operand_load() {
    // C nbody used four instructions where BCC writes `mov vel; add [pos],reg`.
    // Python printed these through `masm`; the reprs are of the semantics it
    // printed.
    let (eax, ecx) = (rl(RegId::EAX, 4), rl(RegId::ECX, 4));
    let position = Loc::Mem(mem(frame(-4), 4, RegId::BP, 0, 2));
    let velocity = Loc::Mem(mem(frame(-8), 4, RegId::BP, 0, 2));
    let head = vec![
        plain(0, Operation::Move, "mov", vec![eax.clone()], vec![position.clone()], None),
        plain(1, Operation::Move, "mov", vec![ecx.clone()], vec![velocity], None),
        plain(2, Operation::Binary, "add", vec![eax.clone()], vec![eax.clone(), ecx.clone()], None),
        plain(3, Operation::Move, "mov", vec![position], vec![eax.clone()], None),
    ];

    let result = fused(rules(), &delayed(head, &eax, &ecx)).blocks[0].clone();
    let printed: Vec<String> = whats(&result.insns).iter().map(Repr::repr).collect();

    let pos = "Mem(addr=[bp-0x4], width=4, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
               selector=None, index=None, scale=1, index_through=0)";
    let vel = "Mem(addr=[bp-0x8], width=4, through=26, offset=0, disp_width=2, base=None, stack_argument=False, \
               selector=None, index=None, scale=1, index_through=0)";
    assert_eq!(
        printed,
        [
            format!(
                "Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=(Reg(register=38, width=4),), \
             sources=({vel},), target=None, indirect=False)"
            ),
            format!(
                "Semantics(op=<Operation.BINARY: 'binary'>, name='add', dests=({pos},), \
             sources=({pos}, Reg(register=38, width=4)), target=None, indirect=False)"
            ),
        ]
    );
}

#[test]
fn test_single_use_loaded_addend_folds_into_the_arithmetic_operand() {
    // Frontend-parity MEMORY emitted C's `mov cx,[si]; add ax,cx`.
    let (ax, cx) = (rl(RegId::AX, 2), rl(RegId::CX, 2));
    let delta =
        Loc::Mem(mem(Some(Addr { base: (RegId::SI).iced(), ..Addr::new(Space::Segment, 0) }), 2, RegId::SI, 0, 0));
    let load = plain(0, Operation::Move, "mov", vec![cx.clone()], vec![delta.clone()], None);
    let addition = plain(1, Operation::Binary, "add", vec![ax.clone()], vec![ax.clone(), cx], None);
    let finish = Arc::new(Insn {
        reads_complete: true,
        ..insn(2, None, Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![])
    });
    let input = body("loaded-addend", 0, vec![block(0, vec![load, addition, Arc::clone(&finish)], vec![]).into()]);

    let physical = whats(&fused(rules(), &input).insns());

    assert_eq!(
        physical,
        [sem(Operation::Binary, "add", vec![ax.clone()], vec![ax, delta]), finish.what.clone().unwrap()]
    );
}

/// lru bas `BENCHLRU&`: an argument loaded at the top and pushed forty
/// instructions later, across stores to globals, stayed `mov ax,[bp+6]` ...
/// `push ax`; where the allocator spilled it, the reload sat beside the push
/// and folded (+1 instruction, +5 B). A load whose cell no instruction in
/// between writes is the push's memory operand wherever it sits.
#[test]
fn test_a_load_folds_into_its_push_across_stores_to_other_cells() {
    let ax = rl(RegId::AX, 2);
    let argument = Loc::Mem(mem(frame(6), 2, RegId::BP, 0, 1));
    let global = Loc::Mem(mem(Some(Addr::new(Space::Segment, 0)), 2, RegId::SI, 0, 0));
    let head = vec![
        plain(0, Operation::Move, "mov", vec![ax.clone()], vec![argument.clone()], None),
        plain(1, Operation::Move, "mov", vec![global], vec![im(7, 2)], None),
        plain(2, Operation::Push, "push", vec![], vec![ax.clone()], None),
    ];
    let end = plain(3, Operation::Move, "mov", vec![ax], vec![im(0, 2)], None);
    let input = body("push-fold", 0, vec![block(0, head, vec![1]).into(), block(1, vec![end], vec![0]).into()]);
    let done = fused(rules(), &input).blocks[0].clone();
    let pushed: Vec<_> = whats(&done.insns).into_iter().filter(|one| one.op == Operation::Push).collect();
    assert_eq!(pushed.len(), 1);
    assert_eq!(pushed[0].sources, [argument], "the push reads the cell: {:?}", whats(&done.insns));
    assert!(
        !done.insns.iter().filter_map(|one| one.what.as_ref()).any(|what| what.name.as_deref() == Some("mov")
            && matches!(what.dests.as_slice(), [Loc::Reg(_)])
            && matches!(what.sources.as_slice(), [Loc::Mem(_)])),
        "the load is gone"
    );
}

/// The same fold stops at a store that may change the cell.
#[test]
fn test_a_load_does_not_fold_into_its_push_across_a_store_to_its_cell() {
    let ax = rl(RegId::AX, 2);
    let argument = mem(frame(6), 2, RegId::BP, 0, 1);
    let head = vec![
        plain(0, Operation::Move, "mov", vec![ax.clone()], vec![Loc::Mem(argument.clone())], None),
        plain(1, Operation::Move, "mov", vec![Loc::Mem(argument)], vec![im(7, 2)], None),
        plain(2, Operation::Push, "push", vec![], vec![ax.clone()], None),
    ];
    let end = plain(3, Operation::Move, "mov", vec![ax.clone()], vec![im(0, 2)], None);
    let input = body("push-nofold", 0, vec![block(0, head, vec![1]).into(), block(1, vec![end], vec![0]).into()]);
    let done = fused(rules(), &input).blocks[0].clone();
    let pushed: Vec<_> = whats(&done.insns).into_iter().filter(|one| one.op == Operation::Push).collect();
    assert_eq!(pushed[0].sources, [ax]);
}

fn narrow_load_parts() -> (Mem, Arc<Insn>, Arc<Insn>) {
    let cell = mem(frame(-8), 2, RegId::BP, 0, 2);
    let narrow = Arc::new(insn(
        0,
        None,
        Some(sem(Operation::Move, "mov", vec![rl(RegId::CX, 2)], vec![Loc::Mem(cell.clone())])),
        vec![1],
        vec![],
    ));
    let wide = Arc::new(insn(
        1,
        None,
        Some(sem(Operation::Extend, "movsx", vec![rl(RegId::EDX, 4)], vec![rl(RegId::CX, 2)])),
        vec![2],
        vec![1],
    ));
    (cell, narrow, wide)
}

#[test]
fn test_narrow_load_folds_into_its_only_widening_use() {
    // C matmul emitted `mov cx,[array]; movsx edx,cx` eight times.
    let (cell, narrow, wide) = narrow_load_parts();
    let used =
        plain(2, Operation::Binary, "add", vec![rl(RegId::EAX, 4)], vec![rl(RegId::EAX, 4), rl(RegId::EDX, 4)], None);
    let input = body("load-extend", 0, vec![block(0, vec![narrow, wide, used], vec![]).into()]);

    let result = extensions(rules(), &input);
    let emitted: Vec<Arc<Insn>> =
        result.insns().into_iter().filter(|one| one.what.as_ref().unwrap().op != Operation::Nothing).collect();

    assert_eq!(emitted[0].what, Some(sem(Operation::Extend, "movsx", vec![rl(RegId::EDX, 4)], vec![Loc::Mem(cell)])));
    assert_eq!(emitted[0].defines, [2]);
    assert_eq!(emitted.len(), 2);
    assert!(verify::verify(&result, false).is_empty());
}

#[test]
fn test_a_far_load_whose_selector_is_required_in_es_still_folds_into_its_widening_use() {
    // bench/huge -O2: `mov cx,es:[di]; movsx ecx,cx` ran 40,200 times beside
    // `movsx ecx,es:[di]`, because the load's requirement of ES refused the
    // fold.
    let (cell, narrow, wide) = narrow_load_parts();
    let mut far = (*narrow).clone();
    far.requires = vec![(Held { value: 9, width: 2 }, RegId::ES)];
    let used =
        plain(2, Operation::Binary, "add", vec![rl(RegId::EAX, 4)], vec![rl(RegId::EAX, 4), rl(RegId::EDX, 4)], None);
    let input = body("far-load-extend", 0, vec![block(0, vec![Arc::new(far), wide, used], vec![]).into()]);

    let result = extensions(rules(), &input);
    let emitted: Vec<Arc<Insn>> =
        result.insns().into_iter().filter(|one| one.what.as_ref().unwrap().op != Operation::Nothing).collect();

    assert_eq!(emitted[0].what, Some(sem(Operation::Extend, "movsx", vec![rl(RegId::EDX, 4)], vec![Loc::Mem(cell)])));
    assert_eq!(emitted[0].requires, [(Held { value: 9, width: 2 }, RegId::ES)], "the selector stays required");
    assert_eq!(emitted.len(), 2);
}

#[test]
fn test_shared_narrow_load_is_not_folded_into_one_widening_use() {
    // The load must remain when another instruction still reads its narrow
    // value.
    let (_cell, narrow, wide) = narrow_load_parts();
    let other =
        Arc::new(insn(2, None, Some(sem(Operation::Push, "push", vec![], vec![rl(RegId::CX, 2)])), vec![], vec![1]));
    let input = body("shared-load", 0, vec![block(0, vec![narrow, wide, other], vec![]).into()]);
    assert_eq!(extensions(rules(), &input), input);
}

#[test]
fn test_one_use_compare_folds_before_a_complete_return() {
    // indexed.lru_use loaded a sign test into DI before immediately returning.
    let di = rl(RegId::DI, 2);
    let cell = Loc::Mem(mem(frame(-4), 2, RegId::BP, 0, 2));
    let load = Arc::new(insn(
        1,
        None,
        Some(sem(Operation::Move, "mov", vec![di.clone()], vec![cell.clone()])),
        vec![1],
        vec![],
    ));
    let compare =
        Arc::new(insn(2, None, Some(sem(Operation::Compare, "cmp", vec![], vec![di, im(0, 2)])), vec![], vec![1]));
    let branch = plain(3, Operation::Branch, "jge", vec![], vec![], Some(2));
    let ret = Arc::new(Insn {
        reads_complete: true,
        ..insn(4, None, Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![])
    });
    let input = body(
        "complete-return-fold",
        0,
        vec![
            block(0, vec![load, compare, Arc::clone(&branch)], vec![1, 2]).into(),
            block(1, vec![Arc::clone(&ret)], vec![]).into(),
            block(2, vec![ret], vec![]).into(),
        ],
    );

    let result = fused(rules(), &input);

    assert_eq!(
        whats(&result.blocks[0].insns),
        [sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)]), branch.what.clone().unwrap(),]
    );
}

#[test]
fn test_dead_compare_load_may_overwrite_its_own_address_register() {
    // lru_use's bnext test used DI for both the pointer and loaded value.
    let di = rl(RegId::DI, 2);
    let cell =
        Loc::Mem(mem(Some(Addr { base: (RegId::DI).iced(), ..Addr::new(Space::Segment, 0) }), 2, RegId::DI, 0, 2));
    let load = plain(1, Operation::Move, "mov", vec![di.clone()], vec![cell.clone()], None);
    let compare = plain(2, Operation::Compare, "cmp", vec![], vec![di.clone(), im(0, 2)], None);
    let overwrite = plain(3, Operation::Move, "mov", vec![di], vec![rl(RegId::AX, 2)], None);
    let input = body(
        "self-addressed-compare",
        0,
        vec![
            block(
                0,
                vec![load, compare, overwrite, plain(4, Operation::Jump, "jmp", vec![], vec![], Some(0))],
                vec![0],
            )
            .into(),
        ],
    );

    let physical = whats(&fused(rules(), &input).insns());

    assert_eq!(physical[0], sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)]));
}

#[test]
fn test_memory_round_trip_does_not_cross_a_dependent_or_writing_instruction() {
    // Delayed memory folding must not change which cell or value an add reads.
    for hazard in ["uses loaded value", "changes address", "writes memory"] {
        let (eax, ebx, ecx) = (rl(RegId::EAX, 4), rl(RegId::EBX, 4), rl(RegId::ECX, 4));
        let through = if hazard == "changes address" { RegId::BX } else { RegId::BP };
        let position = Loc::Mem(mem(
            Some(if through == RegId::BX {
                Addr { base: through.iced(), ..Addr::new(Space::Segment, 0) }
            } else {
                Addr::new(Space::Frame, -4)
            }),
            4,
            through,
            0,
            2,
        ));
        let velocity = Loc::Mem(mem(frame(-8), 4, RegId::BP, 0, 2));
        let mut other = ecx.clone();
        let between = match hazard {
            "uses loaded value" => plain(
                1,
                Operation::Move,
                "mov",
                vec![ecx.clone()],
                vec![Loc::Mem(mem(None, 4, RegId::EAX, 0, 4))],
                None,
            ),
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
        assert_eq!(fused(rules(), &delayed(head.clone(), &eax, &ecx)).blocks[0].insns, head, "{hazard}");
    }
}

// ---------------------------------------------------------------
// test_addressforms

#[test]
fn test_mandel_sum_uses_67h_lea_before_preserving_a_copy() {
    // Mandelbrot emitted `mov sum,xx; add sum,yy` before its escape test.
    for width in [2u32, 4] {
        let registers =
            if width == 2 { [RegId::DI, RegId::DX, RegId::SI] } else { [RegId::EDI, RegId::EDX, RegId::ESI] };
        let [dest, left, right] = registers.map(|register| rl(register, width));
        let copy = Insn {
            widths: vec![(1, width), (3, width)],
            ..insn(
                47,
                Some((47, 47)),
                Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![left])),
                vec![3],
                vec![1],
            )
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
            ..insn(
                48,
                Some((48, 48)),
                Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(1024, width)])),
                vec![],
                vec![4],
            )
        };
        let input = body(
            "mandel",
            47,
            vec![block(47, vec![Arc::new(copy), Arc::new(addition), Arc::new(compare)], vec![]).into()],
        );

        let result = addresses(&input, "386").unwrap().insns();

        assert_eq!(names(&result), ["lea", "cmp"]);
        assert_eq!(result[0].defines, [4]);
        assert_eq!(result[0].uses, [1, 2]);
        assert_eq!(result[0].widths, [(1, width), (2, width), (4, width)]);
        let Loc::Address(address) = &result[0].what.as_ref().unwrap().sources[0] else { panic!("not an address") };
        let mut pair = [address.through, address.index_through];
        pair.sort();
        assert_eq!(pair, [RegId::EDX, RegId::ESI]);
        let encoded = code(result[0].what.as_ref().unwrap());
        assert!(encoded[..2].contains(&0x67));
    }
}

#[test]
fn test_sum_lea_preserves_observed_add_flags() {
    // A conditional branch after the sum still reads ADD's flags.
    let [dest, left, right] = [RegId::EDI, RegId::EDX, RegId::ESI].map(|register| rl(register, 4));
    let copy =
        insn(47, Some((47, 47)), Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![left])), vec![3], vec![1]);
    let addition = insn(
        47,
        Some((47, 48)),
        Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest, right])),
        vec![4],
        vec![3, 2],
    );
    let branch =
        insn(48, Some((48, 48)), Some(semt(Operation::Branch, "je", vec![], vec![], Some(60))), vec![], vec![]);
    let input = body(
        "flagged",
        47,
        vec![block(47, vec![Arc::new(copy), Arc::new(addition), Arc::new(branch)], vec![60]).into()],
    );

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "add", "je"]);
}

#[test]
fn test_sum_lea_preserves_a_copy_result_read_after_the_add() {
    // Nib nbody lost value 333 and stopped in the LIR verifier.
    let (destination, left, right, saved) =
        (rl(RegId::EAX, 4), rl(RegId::EBP, 4), rl(RegId::EDI, 4), rl(RegId::EDX, 4));
    let copy = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![destination.clone()], vec![left])),
        vec![3],
        vec![1],
    );
    let addition = insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Binary, "add", vec![destination.clone()], vec![destination.clone(), right])),
        vec![1],
        vec![1, 2],
    );
    let preserve = insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Move, "mov", vec![saved], vec![destination.clone()])),
        vec![4],
        vec![3],
    );
    let compare = insn(
        4,
        Some((4, 4)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![destination, im(0, 4)])),
        vec![],
        vec![1],
    );
    let input = LirBody {
        inputs: [1, 2].into(),
        ..body(
            "live-copy-sum",
            1,
            vec![
                block(1, vec![Arc::new(copy), Arc::new(addition), Arc::new(preserve), Arc::new(compare)], vec![])
                    .into(),
            ],
        )
    };

    let transformed = addresses(&input, "386").unwrap();

    assert_eq!(names(&transformed.insns()), ["mov", "add", "mov", "cmp"]);
    assert!(verify::verify(&transformed, false).is_empty());
}

fn _constant_sum_body(
    amount: i64,
    symbolic_add: bool,
) -> LirBody {
    let (dest, source) = (rl(RegId::BX, 2), rl(RegId::AX, 2));
    let copy = Insn {
        widths: vec![(1, 2), (3, 2)],
        ..insn(
            10,
            Some((10, 10)),
            Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])),
            vec![3],
            vec![1],
        )
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
                vec![Loc::Mem(Mem { through: RegId::BP, offset: -4, ..Mem::new(frame(-4), 2) })],
                vec![dest],
            )),
            vec![],
            vec![4],
        )
    };
    let compare = Insn {
        widths: vec![(1, 2)],
        ..insn(
            12,
            Some((12, 12)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![source, im(0, 2)])),
            vec![],
            vec![1],
        )
    };
    body(
        "constant_sum",
        10,
        vec![block(10, vec![Arc::new(copy), Arc::new(addition), Arc::new(store), Arc::new(compare)], vec![]).into()],
    )
}

#[test]
fn test_constant_sum_uses_67h_lea_without_code_growth() {
    // Matmul initialized each local element with `mov bx,ax; add bx,1`;
    // `increments` makes that `mov; inc`, three bytes, and the LEA is four:
    // a unit sum stays. A sum of two is the LEA's four bytes against `mov; add`
    // at five.
    for (amount, displacement) in [(2, 2), (0xFFFE, -2)] {
        let result = addresses(&_constant_sum_body(amount, false), "386").unwrap().insns();

        assert_eq!(names(&result), ["lea", "mov", "cmp"]);
        assert_eq!(
            result[0].what.as_ref().unwrap().sources,
            [Loc::Address(AddressRef { through: RegId::EAX, offset: displacement, ..AddressRef::new(None) })]
        );
        let Loc::Address(address) = &result[0].what.as_ref().unwrap().sources[0] else { unreachable!() };
        // `Address` equality compares only `addr`; Python's does too.
        assert_eq!((address.through, address.offset), (RegId::EAX, displacement));
        let encoded = code(result[0].what.as_ref().unwrap());
        assert!(encoded[..2].contains(&0x67));
    }
}

#[test]
fn test_constant_sum_of_one_stays_the_move_and_the_inc() {
    // The unit sum is `mov; inc` once `increments` has run, a byte under the
    // LEA, in both directions.
    for amount in [1, 0xFFFF] {
        let result = addresses(&_constant_sum_body(amount, false), "386").unwrap().insns();
        assert_eq!(names(&result), ["mov", "add", "mov", "cmp"], "{amount}");
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
    let result = addresses(&_constant_sum_body(2, true), "386").unwrap().insns();
    assert_eq!(names(&result), ["lea", "mov", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
}

#[test]
fn test_constant_sum_does_not_drop_a_predecessor_source_anchor() {
    // A fold cannot discard source ownership held by the removed copy.
    let input = _constant_sum_body(1, false);
    let mut insns = input.blocks[0].insns.to_vec();
    insns[0] = Arc::new(Insn { symbol: Some(true), ..(*insns[0]).clone() });
    let input = LirBody {
        blocks: vec![LirBlock { insns: insns.into(), ..input.blocks[0].clone().into_inner() }.into()],
        ..input
    };

    let result = addresses(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "add", "mov", "cmp"]);
    assert_eq!(result[0].symbol, Some(true));
}

// -----------------------------------------------------
// test_dead_address_arithmetic

#[test]
fn test_folded_culling_address_drops_only_unobserved_arithmetic() {
    // r_walk's shared +8 address was folded into its loads but left MOV AX,SI /
    // ADD AX,8.
    for flag_user in [Some("add"), Some("adc"), None] {
        let (ax, si, di) = (rl(RegId::AX, 2), rl(RegId::SI, 2), rl(RegId::DI, 2));
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
        let result = overwritten(&body("cull", 0, vec![block(0, operations, vec![]).into()])).insns();
        assert_eq!(!result.contains(&address), flag_user == Some("add"), "{flag_user:?}");
        assert_eq!(!result.contains(&copy), flag_user == Some("add"), "{flag_user:?}");
        assert!(result.contains(&overwrite));
    }
}

// ------------------------------------------------------------------
// test_parcopy

fn one_block(insns: Vec<Arc<Insn>>) -> LirBody {
    body("one", 0, vec![block(0, insns, vec![]).into()])
}

// ------------------------------------------------------------
// test_postallocation

#[test]
fn test_dword_constant_is_narrowed_when_the_abi_reads_only_its_low_word() {
    // C SCALAR emitted `mov eax,1789` where BASIC needed only AX.
    let value: u32 = 1;
    let source = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EAX, 4)], vec![im(1789, 4)])),
        vec![value],
        vec![],
    );
    let finish = Insn {
        requires: vec![(Held { value: value, width: 2 }, RegId::AX)],
        reads_complete: true,
        ..insn(2, Some((2, 2)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![value])
    };
    // Source/symbol ownership anchors from the unrolled frontend body must be
    // transparent to physical liveness even though they constrain layout.
    let anchor = Insn {
        symbol: Some(true),
        ..insn(2, Some((2, 2)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])
    };
    let input = body(
        "return-low",
        1,
        vec![block(1, vec![Arc::new(source), Arc::new(anchor), Arc::new(finish)], vec![]).into()],
    );

    let result = narrowed_moves(rules(), &input);

    assert_eq!(result.insns()[0].what, Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![im(1789, 2)])));
}

#[test]
fn test_dword_constant_stays_wide_when_any_upper_lane_is_live() {
    let wide = rl(RegId::EAX, 4);
    let source = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![wide.clone()], vec![im(1789, 4)])),
        vec![1],
        vec![],
    );
    let used =
        insn(2, Some((2, 2)), Some(sem(Operation::Compare, "cmp", vec![], vec![wide, im(0, 4)])), vec![], vec![1]);
    let input = body("return-wide", 1, vec![block(1, vec![Arc::new(source), Arc::new(used)], vec![]).into()]);

    assert_eq!(narrowed_moves(rules(), &input), input);
}

#[test]
fn test_dword_fixed_register_argument_keeps_all_value_lanes_live() {
    // Native nbody left its last Y velocity undamped: `mov bx,8192` with stale
    // upper EBX bits.
    let source = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![im(8192, 4)])),
        vec![1],
        vec![],
    );
    let call = Insn {
        clobbers: [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX].into(),
        requires: vec![(Held { value: 1, width: 4 }, RegId::BX)],
        ..insn(2, Some((2, 2)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![1])
    };
    let input = body("wide-fixed-argument", 1, vec![block(1, vec![Arc::new(source), Arc::new(call)], vec![]).into()]);

    assert_eq!(narrowed_moves(rules(), &input), input);
}

#[test]
fn test_dword_copy_read_after_a_call_that_may_clobber_its_upper_half_stays_wide() {
    // regparm3_matrix_4 at -O2: `mov edi,eax` became `mov di,ax` because the
    // call between it and `mov eax,edi; shrd eax,ebx,20` may clobber EDI's
    // upper half, and the shift then read a stale one.
    let (eax, ebx, edi) = (rl(RegId::EAX, 4), rl(RegId::EBX, 4), rl(RegId::EDI, 4));
    let copy =
        insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![edi.clone()], vec![eax.clone()])), vec![1], vec![]);
    let call = Insn {
        clobbers: [RegId::EAX, RegId::ECX, RegId::EDX, RegId::EBX].into(),
        clobbers_high: [RegId::ESI, RegId::EDI].into(),
        ..insn(5, Some((5, 5)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![])
    };
    let read = insn(2, Some((2, 2)), Some(sem(Operation::Move, "mov", vec![eax.clone()], vec![edi])), vec![2], vec![1]);
    let shift = insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Funnel, "shrd", vec![eax.clone()], vec![eax.clone(), ebx, im(20, 1)])),
        vec![3],
        vec![2, 4],
    );
    let finish = Insn {
        requires: vec![(Held { value: 3, width: 4 }, RegId::EAX)],
        reads_complete: true,
        ..insn(4, Some((4, 4)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![3])
    };
    let input = body(
        "wide-copy-shrd",
        1,
        vec![
            block(1, vec![Arc::new(copy), Arc::new(call), Arc::new(read), Arc::new(shift), Arc::new(finish)], vec![])
                .into(),
        ],
    );

    assert_eq!(narrowed_moves(rules(), &input), input);

    // Premise: with nothing reading EDI after it, the same copy is narrowed.
    let ret = Insn {
        reads_complete: true,
        ..insn(6, Some((6, 6)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![])
    };
    let unread = body("narrow-copy", 1, vec![block(1, vec![input.insns()[0].clone(), Arc::new(ret)], vec![]).into()]);
    assert_ne!(narrowed_moves(rules(), &unread), unread);
}

fn extract_parts(
    source: Loc,
    high: Loc,
    discarded: Loc,
    last: Insn,
) -> Vec<Arc<Insn>> {
    vec![
        Arc::new(insn(10, Some((10, 10)), Some(sem(Operation::Push, "push", vec![], vec![source])), vec![], vec![1])),
        Arc::new(insn(10, Some((10, 10)), Some(sem(Operation::Pop, "pop", vec![discarded], vec![])), vec![2], vec![])),
        Arc::new(insn(
            10,
            Some((10, 10)),
            Some(sem(Operation::Pop, "pop", vec![high.clone()], vec![])),
            vec![3],
            vec![],
        )),
        Arc::new(insn(
            11,
            Some((11, 11)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![high, im(0, 2)])),
            vec![],
            vec![3],
        )),
        Arc::new(last),
    ]
}

#[test]
fn test_register_high_extract_uses_one_double_shift_for_dx_ax_return() {
    // Frontend-parity ALGEBRA returned its high word with push/pop/pop.
    let source = rl(RegId::EBX, 4);
    let last = insn(
        12,
        Some((12, 12)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EDX, 4)], vec![im(0, 4)])),
        vec![4],
        vec![],
    );
    let parts = extract_parts(source.clone(), rl(RegId::DX, 2), rl(RegId::BX, 2), last);
    let input = body("return-high", 10, vec![block(10, parts, vec![]).into()]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shld", "cmp", "mov"]);
    assert_eq!(
        result[0].what,
        Some(sem(Operation::Funnel, "shld", vec![rl(RegId::EDX, 4)], vec![rl(RegId::EDX, 4), source, im(16, 1)]))
    );
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_selected_move_shift_high_extract_uses_the_same_double_shift() {
    // Frontend-parity ALGEBRA's C path retained MOV EDX,ECX; SHR EDX,16.
    let (source, high) = (rl(RegId::ECX, 4), rl(RegId::EDX, 4));
    let parts = vec![
        Arc::new(insn(
            10,
            Some((10, 10)),
            Some(sem(Operation::Move, "mov", vec![high.clone()], vec![source.clone()])),
            vec![3],
            vec![1],
        )),
        Arc::new(insn(
            11,
            Some((11, 11)),
            Some(sem(Operation::Binary, "shr", vec![high.clone()], vec![high.clone(), im(16, 1)])),
            vec![3],
            vec![3],
        )),
        Arc::new(Insn {
            reads_complete: true,
            requires: vec![(Held { value: 4, width: 2 }, RegId::AX), (Held { value: 3, width: 2 }, RegId::DX)],
            ..insn(
                12,
                Some((12, 12)),
                Some(sem(Operation::Return, "retf", vec![], vec![rl(RegId::AX, 2), rl(RegId::DX, 2)])),
                vec![],
                vec![4, 3],
            )
        }),
    ];
    let input = body("c-return-high", 10, vec![block(10, parts, vec![]).into()]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shld", "retf"]);
    assert_eq!(result[0].what, Some(sem(Operation::Funnel, "shld", vec![high.clone()], vec![high, source, im(16, 1)])));
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_a_move_shift_high_extract_is_found_past_an_unrelated_instruction() {
    // regparm3_matrix_4 (m16 -Os): a recolored allocation put the spill store
    // of the return value between `mov esi, eax` and `shr esi, 16`, and the
    // shift stayed (`mov` + `shr` for one `shld`, +8 bytes).
    let (source, high) = (rl(RegId::ECX, 4), rl(RegId::EDX, 4));
    let parts = vec![
        Arc::new(insn(
            10,
            Some((10, 10)),
            Some(sem(Operation::Move, "mov", vec![high.clone()], vec![source.clone()])),
            vec![3],
            vec![1],
        )),
        Arc::new(insn(
            11,
            Some((11, 11)),
            Some(sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![source.clone()])),
            vec![5],
            vec![1],
        )),
        Arc::new(insn(
            12,
            Some((12, 12)),
            Some(sem(Operation::Binary, "shr", vec![high.clone()], vec![high.clone(), im(16, 1)])),
            vec![3],
            vec![3],
        )),
        Arc::new(Insn {
            reads_complete: true,
            requires: vec![(Held { value: 4, width: 2 }, RegId::AX), (Held { value: 3, width: 2 }, RegId::DX)],
            ..insn(
                13,
                Some((13, 13)),
                Some(sem(
                    Operation::Return,
                    "retf",
                    vec![],
                    vec![rl(RegId::AX, 2), rl(RegId::DX, 2), rl(RegId::BX, 2)],
                )),
                vec![],
                vec![4, 3, 5],
            )
        }),
    ];
    let input = body("c-return-high-apart", 10, vec![block(10, parts, vec![]).into()]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["mov", "shld", "retf"]);
}

#[test]
fn test_a_move_shift_high_extract_is_found_through_a_coalescing_anchor() {
    // regparm3_matrix_4 (m16 -Os): `mov esi, eax` defined a split piece that an
    // anchor (no operation) handed to the value the shift is tied to, and
    // the pair stayed.
    let (source, high) = (rl(RegId::ECX, 4), rl(RegId::EDX, 4));
    let parts = vec![
        Arc::new(insn(
            10,
            Some((10, 10)),
            Some(sem(Operation::Move, "mov", vec![high.clone()], vec![source.clone()])),
            vec![7],
            vec![1],
        )),
        Arc::new(insn(11, Some((11, 11)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![3], vec![7])),
        Arc::new(insn(
            12,
            Some((12, 12)),
            Some(sem(Operation::Binary, "shr", vec![high.clone()], vec![high.clone(), im(16, 1)])),
            vec![3],
            vec![3],
        )),
        Arc::new(Insn {
            reads_complete: true,
            requires: vec![(Held { value: 4, width: 2 }, RegId::AX), (Held { value: 3, width: 2 }, RegId::DX)],
            ..insn(
                13,
                Some((13, 13)),
                Some(sem(Operation::Return, "retf", vec![], vec![rl(RegId::AX, 2), rl(RegId::DX, 2)])),
                vec![],
                vec![4, 3],
            )
        }),
    ];
    let input = body("c-return-high-anchored", 10, vec![block(10, parts, vec![]).into()]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert!(
        result.iter().any(|one| one.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("shld"))),
        "{result:?}"
    );
    assert!(!result.iter().any(|one| one.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("shr"))));
    // The anchor's use still has a definition.
    assert!(result.iter().any(|one| one.defines.contains(&7)));
}

#[test]
fn test_register_high_extract_shifts_a_dying_return_root_in_place() {
    // Frontend-parity LOOP returned EDX through push/pop/pop.
    let source = rl(RegId::EDX, 4);
    let last = insn(
        12,
        Some((12, 12)),
        Some(sem(Operation::Move, "mov", vec![source.clone()], vec![im(0, 4)])),
        vec![4],
        vec![],
    );
    let parts = extract_parts(source.clone(), rl(RegId::DX, 2), rl(RegId::BX, 2), last);
    let input = body("return-high-in-place", 10, vec![block(10, parts, vec![]).into()]);

    let result = high_extracts(&input, "386").unwrap().insns();

    assert_eq!(names(&result), ["shr", "cmp", "mov"]);
    assert_eq!(result[0].what, Some(sem(Operation::Binary, "shr", vec![source.clone()], vec![source, im(16, 1)])));
    assert_eq!(result[0].defines, [3]);
    assert_eq!(result[0].uses, [1]);
}

#[test]
fn test_dead_flags_crossing_increment_do_not_block_zero_idiom() {
    // PARITYCONTROL kept `mov eax,0` because a later DEC preserved dead CF.
    let (eax, bx, cx) = (rl(RegId::EAX, 4), rl(RegId::BX, 2), rl(RegId::CX, 2));
    let insns = vec![
        Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![eax.clone()], vec![im(0, 4)])),
            vec![1],
            vec![],
        )),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Unary, "dec", vec![bx.clone()], vec![bx])),
            vec![2],
            vec![2],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, "and", vec![cx.clone()], vec![cx.clone(), cx])),
            vec![3],
            vec![3],
        )),
        Arc::new(insn(3, Some((3, 3)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(1))), vec![], vec![])),
    ];
    let input = body("zero-before-dec", 0, vec![block(0, insns, vec![1]).into()]);

    let result = zeroes(&input);

    assert_eq!(result.insns()[0].what, Some(sem(Operation::Binary, "xor", vec![eax.clone()], vec![eax.clone(), eax])));
}

#[test]
fn test_return_high_extraction_drops_redundant_low_word_shuttle() {
    // PARITYMEMORY returned EAX as AX:DX through two needless BX moves.
    let (eax, ax, bx, edx, dx) =
        (rl(RegId::EAX, 4), rl(RegId::AX, 2), rl(RegId::BX, 2), rl(RegId::EDX, 4), rl(RegId::DX, 2));
    let parts = vec![
        Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![ax.clone()])),
            vec![2],
            vec![1],
        )),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![edx.clone()], vec![eax])),
            vec![3],
            vec![1],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, "shr", vec![edx.clone()], vec![edx, im(16, 1)])),
            vec![4],
            vec![3],
        )),
        Arc::new(insn(
            3,
            Some((3, 3)),
            Some(sem(Operation::Move, "mov", vec![ax.clone()], vec![bx])),
            vec![5],
            vec![2],
        )),
        Arc::new(Insn {
            clobbers: [RegId::EBX].into(),
            ..insn(4, Some((4, 4)), Some(sem(Operation::Nothing, "", vec![], vec![])), vec![], vec![])
        }),
        Arc::new(Insn {
            requires: vec![(Held { value: 5, width: 2 }, RegId::AX), (Held { value: 4, width: 2 }, RegId::DX)],
            ..insn(5, Some((5, 5)), Some(sem(Operation::Return, "retf", vec![], vec![ax, dx])), vec![], vec![5, 4])
        }),
    ];
    let input = body("return-pair", 0, vec![block(0, parts.clone(), vec![]).into()]);

    let result = transform(input);

    assert_eq!(
        whats(&result.insns()),
        [parts[1].what.clone().unwrap(), parts[2].what.clone().unwrap(), parts[5].what.clone().unwrap(),]
    );
}

#[test]
fn test_unit_add_selects_inc_only_when_carry_is_dead() {
    // Frontend-parity LOOP used C's ADD 1 but BASIC's equivalent INC.
    let counter = rl(RegId::CX, 2);
    let add = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "add", vec![counter.clone()], vec![counter.clone(), im(1, 2)])),
        vec![1],
        vec![1],
    ));
    let compare = Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![counter, im(8, 2)])),
        vec![],
        vec![1],
    ));
    let dead = body("counter", 1, vec![block(1, vec![Arc::clone(&add), compare], vec![]).into()]);
    let branch =
        Arc::new(insn(2, Some((2, 2)), Some(semt(Operation::Branch, "jb", vec![], vec![], Some(3))), vec![], vec![]));
    let live = body("carry", 1, vec![block(1, vec![add, branch], vec![3]).into(), block(3, vec![], vec![]).into()]);

    assert_eq!(names(&increments(rules(), &dead).insns())[0], "inc");
    assert_eq!(names(&increments(rules(), &live).insns())[0], "add");
}

/// A body of one `lea eax, address` and, where `flags_read`, a `jb` after it.
fn lea_body(
    address: AddressRef,
    flags_read: bool,
) -> LirBody {
    let eax = rl(RegId::EAX, 4);
    let lea = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Address, "lea", vec![eax], vec![Loc::Address(address)])),
        vec![1],
        vec![1],
    ));
    if !flags_read {
        // A compare writes every flag before anything reads one.
        let compare = Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::EAX, 4), im(8, 4)])),
            vec![],
            vec![1],
        ));
        return body("lea", 1, vec![block(1, vec![lea, compare], vec![]).into()]);
    }
    let branch =
        Arc::new(insn(2, Some((2, 2)), Some(semt(Operation::Branch, "jb", vec![], vec![], Some(3))), vec![], vec![]));
    body("lea", 1, vec![block(1, vec![lea, branch], vec![3]).into(), block(3, vec![], vec![]).into()])
}

fn sum(
    through: RegId,
    index_through: RegId,
    scale: i64,
    offset: i64,
) -> AddressRef {
    AddressRef { through, index_through, scale, offset, ..AddressRef::new(None) }
}

/// X86FixupLEAs `optTwoAddrLEA`: `lea eax,[eax+ecx]` is `add eax,ecx`, in
/// either operand order, where no flag is read after it. Without the pass the
/// LEA stayed an LEA: 3 bytes for `add`, 3 for `lea`, but the pin's output
/// differs in 37 IRs.
#[test]
fn test_two_address_lea_sum_is_an_add_where_flags_are_dead() {
    for (through, index) in [(RegId::EAX, RegId::ECX), (RegId::ECX, RegId::EAX)] {
        let done = fixed_leas(&lea_body(sum(through, index, 1, 0), false));
        let what = done.insns()[0].what.clone().unwrap();
        assert_eq!(what.name.as_deref(), Some("add"));
        assert_eq!(what.sources, [rl(RegId::EAX, 4), rl(RegId::ECX, 4)]);
        let kept = fixed_leas(&lea_body(sum(through, index, 1, 0), true));
        assert_eq!(names(&kept.insns())[0], "lea", "a jb reads the flags the add would set");
    }
}

/// `lea eax,[eax+8]` is `add eax,8`; a step of one is `inc`, of minus one
/// `dec`.
#[test]
fn test_two_address_lea_step_is_an_add_where_flags_are_dead() {
    let done = fixed_leas(&lea_body(sum(RegId::EAX, RegId::None, 1, 8), false));
    let what = done.insns()[0].what.clone().unwrap();
    assert_eq!(what.name.as_deref(), Some("add"));
    assert_eq!(what.sources, [rl(RegId::EAX, 4), im(8, 4)]);
    let one = fixed_leas(&lea_body(sum(RegId::EAX, RegId::None, 1, 1), false));
    assert_eq!(names(&one.insns())[0], "inc");
    let minus = fixed_leas(&lea_body(sum(RegId::EAX, RegId::None, 1, -1), false));
    assert_eq!(names(&minus.insns())[0], "dec");
}

/// The two-address pass spells an `lea` as a memory operand. The pin's case 1
/// does not care how the address is spelled or what follows: `lea
/// eax,[eax+ecx]` is `add eax,ecx` alone. Only one followed by an add was
/// (`lea_term_then_alu`), so a bare one stayed a 3-byte `lea`.
#[test]
fn test_a_two_address_lea_spelled_as_a_memory_operand_is_an_add_too() {
    let mut lea = lea_body(sum(RegId::EAX, RegId::ECX, 1, 0), false);
    let cell = Loc::Mem(Mem { through: RegId::EAX, index_through: RegId::ECX, scale: 1, ..Mem::new(None, 4) });
    let first = lea.blocks[0].insns.iter().next().cloned().expect("the lea");
    let mut moved = (*first).clone();
    moved.what.as_mut().unwrap().sources = vec![cell];
    let block = lea.blocks[0]
        .with_insns(std::iter::once(Arc::new(moved)).chain(lea.blocks[0].insns.iter().skip(1).cloned()).collect());
    lea.blocks[0] = block;
    let done = fixed_leas(&lea);
    assert_eq!(names(&done.insns())[0], "add", "{:?}", steps(&done));
}

/// The pin leaves a three-address or scaled LEA alone: only the two-address
/// scale-1 forms are the pass's.
#[test]
fn test_lea_that_is_not_two_address_stays() {
    for address in [
        sum(RegId::ECX, RegId::EDX, 1, 0),
        sum(RegId::EAX, RegId::ECX, 2, 0),
        sum(RegId::EAX, RegId::ECX, 1, 4),
        sum(RegId::ECX, RegId::None, 1, 8),
    ] {
        let done = fixed_leas(&lea_body(address, false));
        assert_eq!(names(&done.insns())[0], "lea");
    }
}

/// `lea ecx,[eax+edx]`, the `between` instructions, `op d, d, ecx`, then a
/// compare that writes every flag (or a `jb` that reads them, where
/// `flags_read`).
fn lea_alu_body(
    op: &str,
    d: RegId,
    between: Vec<Arc<Insn>>,
    flags_read: bool,
) -> LirBody {
    lea_alu_body_at(sum(RegId::EAX, RegId::EDX, 1, 0), op, d, between, flags_read)
}

fn lea_alu_body_at(
    address: AddressRef,
    op: &str,
    d: RegId,
    between: Vec<Arc<Insn>>,
    flags_read: bool,
) -> LirBody {
    let ecx = rl(RegId::ECX, 4);
    let mut insns = vec![Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Address, "lea", vec![ecx.clone()], vec![Loc::Address(address)])),
        vec![11],
        vec![20, 21],
    ))];
    insns.extend(between);
    insns.push(Arc::new(insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Binary, op, vec![rl(d, 4)], vec![rl(d, 4), ecx])),
        vec![12],
        vec![13, 11],
    )));
    if flags_read {
        insns.push(Arc::new(insn(
            4,
            Some((4, 4)),
            Some(semt(Operation::Branch, "jb", vec![], vec![], Some(3))),
            vec![],
            vec![],
        )));
        return body("leaalu", 1, vec![block(1, insns, vec![3]).into(), block(3, vec![], vec![]).into()]);
    }
    insns.push(Arc::new(insn(
        4,
        Some((4, 4)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::EBX, 4), im(8, 4)])),
        vec![],
        vec![],
    )));
    // The return reads only eax: everything else is dead after it.
    insns.push(Arc::new(Insn {
        requires: vec![(Held { value: 7, width: 4 }, RegId::EAX)],
        reads_complete: true,
        ..insn(5, Some((5, 5)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![7])
    }));
    body("leaalu", 1, vec![block(1, insns, vec![]).into()])
}

/// The block through `FixupLEAs`.
fn fixed_leas(body: &LirBody) -> LirBody {
    crate::backend::fixupleas::FixupLeas::default().transform(body.clone()).expect("the pass cannot fail")
}

fn steps(done: &LirBody) -> Vec<(String, Vec<Loc>)> {
    done.insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .map(|what| (what.name.as_deref().unwrap_or_default().to_owned(), what.sources.clone()))
        .collect()
}

/// X86FixupLEAs `optLEAALU`: `lea ecx,[eax+edx]; add ebx,ecx` is `add ebx,eax;
/// add ebx,edx`, and a subtract the same two steps. Without the case the
/// six-term sums of two llrm-c listings kept the `lea`/`mov` pair and ended
/// `mov eax, edx`.
#[test]
fn test_an_lea_read_once_by_an_add_or_sub_is_two_steps_on_its_destination() {
    for op in ["add", "sub"] {
        let done = steps(&fixed_leas(&lea_alu_body(op, RegId::EBX, vec![], false)));
        assert_eq!(done[0], (op.to_owned(), vec![rl(RegId::EBX, 4), rl(RegId::EAX, 4)]), "{done:?}");
        assert_eq!(done[1], (op.to_owned(), vec![rl(RegId::EBX, 4), rl(RegId::EDX, 4)]), "{done:?}");
        assert_eq!(done.len(), 4, "the lea is gone: two steps, the compare and the return: {done:?}");
        // The lea's value (11) is gone with it: a step reading it was "read but
        // never defined" in call_keeps.c.
        let again = fixed_leas(&lea_alu_body(op, RegId::EBX, vec![], false));
        assert!(again.insns().iter().all(|one| !one.uses.contains(&11)), "{op}: a step still reads the lea's value");
        assert!(
            again.insns().iter().any(|one| one.uses.contains(&20) && one.uses.contains(&21)),
            "{op}: the steps read the address's values"
        );
    }
}

/// `lea cx,[eax+edx]; add bx,cx` (a word `lea` of dword address registers, as
/// the real-mode target makes one) is no two word steps on `eax` and `edx`,
/// which are no word operands: QCport's `con_draw` ended `add ax, ecx; add ax,
/// edi`, which no assembler takes, and its object was not made. The pin splits
/// only the `lea`s whose terms are as wide as the add.
#[test]
fn test_a_word_lea_of_dword_registers_is_not_split_into_word_adds() {
    let cx = rl(RegId::CX, 2);
    let lea = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Address, "lea", vec![cx.clone()], vec![Loc::Address(sum(RegId::EAX, RegId::EDX, 1, 0))])),
        vec![11],
        vec![20, 21],
    ));
    let add = Arc::new(insn(
        3,
        Some((3, 3)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::BX, 2)], vec![rl(RegId::BX, 2), cx])),
        vec![12],
        vec![13, 11],
    ));
    let cmp = Arc::new(insn(
        4,
        Some((4, 4)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::SI, 2), im(8, 2)])),
        vec![],
        vec![],
    ));
    // The return reads only eax: everything else is dead after it, as in
    // `lea_alu_body_at`.
    let ret = Arc::new(Insn {
        requires: vec![(Held { value: 7, width: 4 }, RegId::EAX)],
        reads_complete: true,
        ..insn(5, Some((5, 5)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![7])
    });
    let done = fixed_leas(&body("wordlea", 1, vec![block(1, vec![lea, add, cmp, ret], vec![]).into()]));
    assert_eq!(names(&done.insns())[0], "lea", "{:?}", steps(&done));
}

/// Where the address register `d` is the index, `d` is added first: `d + (x+d)`
/// is `(d+d)+x`.
#[test]
fn test_the_destination_that_is_an_address_register_is_added_first() {
    let done = steps(&fixed_leas(&lea_alu_body("add", RegId::EDX, vec![], false)));
    assert_eq!(done[0].1, vec![rl(RegId::EDX, 4), rl(RegId::EDX, 4)], "{done:?}");
    assert_eq!(done[1].1, vec![rl(RegId::EDX, 4), rl(RegId::EAX, 4)], "{done:?}");
}

/// `lea ecx,[eax+ecx]; add ebx,ecx` (the LEA's register is one of its terms, as
/// six_ allocated it): `add ecx,eax`, and the add stays (`optTwoAddrLEA` case
/// 1, in the shape the two-address pass makes). It stayed an `lea` whose result
/// the add read, and six_ listed one instruction more than the pin's chain of
/// adds.
#[test]
fn test_an_lea_that_sums_into_its_own_register_before_an_add_is_an_add() {
    let done =
        steps(&fixed_leas(&lea_alu_body_at(sum(RegId::EAX, RegId::ECX, 1, 0), "add", RegId::EBX, vec![], false)));
    assert_eq!(done[0], ("add".to_owned(), vec![rl(RegId::ECX, 4), rl(RegId::EAX, 4)]), "{done:?}");
    assert_eq!(done[1], ("add".to_owned(), vec![rl(RegId::EBX, 4), rl(RegId::ECX, 4)]), "{done:?}");
}

/// The search ends where the ALU is not the next instruction (the pin looks
/// five ahead: DEVIATE LF-2), where the flags are read after the add, and where
/// the `lea`'s register is read again.
#[test]
fn test_an_lea_stays_where_the_add_is_not_next_or_its_flags_are_read() {
    let between = Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::ESI, 4)], vec![rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ));
    let read = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, vec![between], false)));
    assert_eq!(read[0].0, "lea", "{read:?}");
    let flags = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, vec![], true)));
    assert_eq!(flags[0].0, "lea", "{flags:?}");
}

/// An unrelated `mov r, imm` at `at`.
fn filler(
    at: i64,
    register: RegId,
) -> Arc<Insn> {
    Arc::new(insn(
        at,
        Some((at, at)),
        Some(sem(Operation::Move, "mov", vec![rl(register, 4)], vec![im(1, 4)])),
        vec![],
        vec![],
    ))
}

/// X86FixupLEAs `searchALUInst` looks five instructions ahead: the add three
/// after the `lea` is found. Only the next instruction was, and the gap kept
/// the `lea` and its `mov` (`optLEAALU` of the pin is the two adds).
#[test]
fn test_an_lea_is_split_where_its_add_is_within_five_instructions() {
    let between = vec![filler(2, RegId::ESI), filler(2, RegId::EDI), filler(2, RegId::EBP)];
    let done = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, between, false)));
    let names: Vec<&str> = done.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["mov", "mov", "mov", "add", "add", "cmp", "ret"], "{done:?}");
    assert_eq!(done[3].1, vec![rl(RegId::EBX, 4), rl(RegId::EAX, 4)]);
}

#[test]
fn test_an_lea_stays_where_its_add_is_more_than_five_instructions_away() {
    let between = (0..6)
        .map(|n| filler(2, [RegId::ESI, RegId::EDI, RegId::EBP, RegId::ESI, RegId::EDI, RegId::EBP][n]))
        .collect();
    let done = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, between, false)));
    assert_eq!(done[0].0, "lea", "{done:?}");
}

/// A base register written between the `lea` and the add: the two steps go
/// where the `lea` was (`InsertPos = I`).
#[test]
fn test_the_steps_go_before_a_redefinition_of_the_lea_s_base() {
    let done = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, vec![filler(2, RegId::EAX)], false)));
    let names: Vec<&str> = done.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["add", "add", "mov", "cmp", "ret"], "{done:?}");
}

/// ... unless the add's destination is named in between: then the pin leaves
/// the `lea`.
#[test]
fn test_an_lea_stays_where_its_base_is_written_and_the_add_s_destination_is_named() {
    let named = Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::ESI, 4)], vec![rl(RegId::EBX, 4)])),
        vec![],
        vec![],
    ));
    let done = steps(&fixed_leas(&lea_alu_body("add", RegId::EBX, vec![filler(2, RegId::EAX), named], false)));
    assert_eq!(done[0].0, "lea", "{done:?}");
}

/// The combiner's canonical `x - 1` is `add x, -1`: INC and DEC were chosen
/// only for `sub x, 1`, so recmany and recfloat grew (`dec ecx` became `add
/// edx, -1`, 3 bytes against 1). X86's patterns select DEC for `add x, -1` as
/// well.
#[test]
fn test_add_of_minus_one_selects_dec_only_when_carry_is_dead() {
    for width in [2, 4] {
        let counter = rl(RegId::CX, width);
        let add = Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Binary, "add", vec![counter.clone()], vec![counter.clone(), im(-1, width)])),
            vec![1],
            vec![1],
        ));
        let compare = Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![counter, im(8, width)])),
            vec![],
            vec![1],
        ));
        let dead = body("counter", 1, vec![block(1, vec![Arc::clone(&add), compare], vec![]).into()]);
        let branch = Arc::new(insn(
            2,
            Some((2, 2)),
            Some(semt(Operation::Branch, "jb", vec![], vec![], Some(3))),
            vec![],
            vec![],
        ));
        let live = body("carry", 1, vec![block(1, vec![add, branch], vec![3]).into(), block(3, vec![], vec![]).into()]);

        assert_eq!(names(&increments(rules(), &dead).insns())[0], "dec", "width {width}");
        assert_eq!(names(&increments(rules(), &live).insns())[0], "add", "width {width}");
    }
}

fn _pair(
    operation: Operation,
    name: &str,
    tail: Vec<Arc<Insn>>,
) -> (LirBody, Insn) {
    let (left, right) = (rl(RegId::EBX, 4), rl(RegId::ECX, 4));
    let combined = insn(
        1,
        Some((1, 3)),
        Some(sem(operation, name, vec![left.clone()], vec![left.clone(), right.clone()])),
        vec![10],
        vec![1, 2],
    );
    let copied = insn(3, Some((3, 3)), Some(sem(Operation::Move, "mov", vec![right], vec![left])), vec![11], vec![10]);
    let insns = [vec![Arc::new(combined), Arc::new(copied.clone())], tail].concat();
    (body("pair", 0, vec![block(0, insns, vec![]).into()]), copied)
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
        let (left, right) = (rl(RegId::EBX, 4), rl(RegId::ECX, 4));
        let shift = Arc::new(insn(
            3,
            Some((3, 5)),
            Some(sem(Operation::Binary, "sar", vec![right.clone()], vec![right.clone(), im(7, 1)])),
            vec![12],
            vec![11],
        ));
        let overwrite = Arc::new(insn(
            5,
            Some((5, 7)),
            Some(sem(Operation::Move, "mov", vec![left.clone()], vec![im(0, 4)])),
            vec![13],
            vec![],
        ));
        let (input, copied) = _pair(operation, name, vec![Arc::clone(&shift), Arc::clone(&overwrite)]);

        let result = transferred(rules(), &input).insns();

        assert_eq!(result[0].what, Some(sem(operation, name, vec![right.clone()], vec![right, left])));
        assert_eq!(result[1].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(result[1].defines, copied.defines);
        assert_eq!(result[2..], [shift, overwrite]);
    }
}

#[test]
fn test_commutative_result_copy_keeps_a_still_live_first_operand() {
    // Changing which multiply input is destroyed is legal only when the old
    // destination dies.
    let used = Arc::new(insn(
        3,
        Some((3, 5)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::EBX, 4), im(0, 4)])),
        vec![],
        vec![10],
    ));
    let (input, _copied) = _pair(Operation::Multiply, "imul", vec![used]);

    assert_eq!(transferred(rules(), &input), input);
}

#[test]
fn test_commutative_result_copy_keeps_source_owned_copy_bytes() {
    // A real input instruction is not the synthetic transfer this rewrite may
    // erase.
    let overwrite = Arc::new(insn(
        5,
        Some((5, 7)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![im(0, 4)])),
        vec![13],
        vec![],
    ));
    let (input, copied) = _pair(Operation::Multiply, "imul", vec![Arc::clone(&overwrite)]);
    let owned = Arc::new(Insn { covers: Some((3, 5)), ..copied });
    let input = LirBody {
        blocks: vec![
            LirBlock {
                insns: vec![Arc::clone(&input.insns()[0]), owned, overwrite].into(),
                ..input.blocks[0].clone().into_inner()
            }
            .into(),
        ],
        ..input
    };

    assert_eq!(transferred(rules(), &input), input);
}

fn _high_extract_body(tail: Vec<Arc<Insn>>) -> LirBody {
    let wide = rl(RegId::EDX, 4);
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-4), 4) });
    // A reload carries the call memory of what it stands beside: here a call
    // that touches none.
    let beside = crate::model::lir::CallMemory {
        effects: llrm_mir::memory::Effects::NONE,
        private: vec![],
        disturbs: Default::default(),
    };
    let load = Insn {
        symbol: Some(false),
        call: Some(Arc::new(beside)),
        ..insn(1, Some((1, 1)), Some(sem(Operation::Move, "mov", vec![wide.clone()], vec![cell])), vec![1], vec![])
    };
    let shift = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "shr", vec![wide.clone()], vec![wide, im(16, 1)])),
        vec![1],
        vec![1],
    );
    body("extract", 0, vec![block(0, [vec![Arc::new(load), Arc::new(shift)], tail].concat(), vec![]).into()])
}

fn _return_high() -> Arc<Insn> {
    Arc::new(Insn {
        reads_complete: true,
        requires: vec![(Held { value: 1, width: 2 }, RegId::DX)],
        ..insn(2, Some((2, 3)), Some(sem(Operation::Return, "retf", vec![], vec![])), vec![], vec![1])
    })
}

#[test]
fn test_synthetic_high_extract_loads_only_the_high_word() {
    // Mandel loaded a spilled dword and shifted it solely to return the high
    // word.
    let returned = _return_high();

    let result = high_extracts(&_high_extract_body(vec![Arc::clone(&returned)]), "386").unwrap().insns();

    let (load, anchor) = (&result[0], &result[1]);
    assert_eq!(
        load.what,
        Some(sem(
            Operation::Move,
            "mov",
            vec![rl(RegId::DX, 2)],
            vec![Loc::Mem(Mem { through: RegId::BP, offset: 2, ..Mem::new(frame(-2), 2) })],
        ))
    );
    assert_eq!(anchor.what.as_ref().unwrap().op, Operation::Nothing);
    assert_eq!(anchor.defines, [1]);
    assert_eq!(result[2..], [returned]);
}

#[test]
fn test_high_extract_keeps_observable_wide_load_or_shift_effects() {
    // Narrowing is legal only for a synthetic load with dead upper lanes and
    // flags.
    for hazard in ["full-result", "flags", "source-load"] {
        let tail = match hazard {
            "full-result" => vec![
                Arc::new(insn(
                    2,
                    Some((2, 4)),
                    Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::EDX, 4), im(0, 4)])),
                    vec![],
                    vec![1],
                )),
                _return_high(),
            ],
            "flags" => vec![Arc::new(insn(
                2,
                Some((2, 4)),
                Some(semt(Operation::Branch, "je", vec![], vec![], Some(9))),
                vec![],
                vec![],
            ))],
            _ => vec![_return_high()],
        };
        let mut input = _high_extract_body(tail);
        if hazard == "source-load" {
            let mut insns = input.blocks[0].insns.to_vec();
            insns[0] = Arc::new(Insn { covers: Some((1, 3)), symbol: None, ..(*insns[0]).clone() });
            input = LirBody {
                blocks: vec![LirBlock { insns: insns.into(), ..input.blocks[0].clone().into_inner() }.into()],
                ..input
            };
        }

        assert_eq!(high_extracts(&input, "386").unwrap(), input, "{hazard}");
    }
}

// -------------------------------------------------------------------
// test_prologue

fn procedure() -> LirBody {
    let instruction = |at: i64, what: Semantics| insn(at, Some((at, at + 1)), Some(what), vec![], vec![]);
    let held = Loc::Held(Held { value: 1, width: 2 });
    let insns = vec![
        Arc::new(instruction(0, sem(Operation::Move, "mov", vec![held.clone()], vec![im(6, 2)]))),
        Arc::new(Insn {
            requires: vec![(Held { value: 1, width: 2 }, RegId::CX)],
            ..instruction(1, sem(Operation::Call, "call", vec![], vec![held]))
        }),
        Arc::new(instruction(2, sem(Operation::Call, "call", vec![], vec![]))),
        Arc::new(instruction(3, sem(Operation::Return, "retf", vec![], vec![]))),
    ];
    body("procedure", 0, vec![block(0, insns, vec![]).into()])
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
                    vec![rl(RegId::AX, 2)],
                    vec![if used == "load" {
                        Loc::Mem(cell.clone())
                    } else {
                        Loc::Imm(Imm { value: 0, width: 2, address: cell.addr })
                    }],
                )
            });
            let one = Arc::new(insn(4, Some((4, 4)), what, vec![], vec![]));
            input.blocks[0].insns.edit(|insns| insns.push(one));
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

#[test]
fn test_a_repeated_address_copy_does_not_hide_a_read_modify_write() {
    // DRAWBOB's counter took a load, an add and a store once allocation
    // copied its pointer into SI again, as a new value, before the store.
    let (si, ax, dx) = (rl(RegId::SI, 2), rl(RegId::AX, 2), rl(RegId::DX, 2));
    let cell =
        |base: u32| Loc::Mem(Mem { base: Some(Held { value: base, width: 2 }), ..mem(None, 2, RegId::SI, 0, 0) });
    let copy = |at, into: u32| {
        Arc::new(insn(
            at,
            None,
            Some(sem(Operation::Move, "mov", vec![si.clone()], vec![ax.clone()])),
            vec![into],
            vec![1],
        ))
    };
    let insns = vec![
        copy(1, 10),
        Arc::new(insn(
            2,
            None,
            Some(sem(Operation::Move, "mov", vec![dx.clone()], vec![cell(10)])),
            vec![11],
            vec![10],
        )),
        Arc::new(insn(
            3,
            None,
            Some(sem(Operation::Binary, "add", vec![dx.clone()], vec![dx.clone(), im(1, 2)])),
            vec![12],
            vec![11],
        )),
        copy(4, 13),
        Arc::new(insn(
            5,
            None,
            Some(sem(Operation::Move, "mov", vec![cell(13)], vec![dx.clone()])),
            vec![],
            vec![13, 12],
        )),
        Arc::new(insn(6, None, Some(sem(Operation::Move, "mov", vec![dx], vec![im(0, 2)])), vec![14], vec![])),
    ];
    let result = transform(body("rmw", 0, vec![block(0, insns, vec![]).into()]));
    let found: Vec<String> = whats(&result.insns()).iter().map(Repr::repr).collect();
    let loads = whats(&result.insns())
        .iter()
        .filter(|what| what.op == Operation::Move && what.sources.iter().any(|one| matches!(one, Loc::Mem(_))))
        .count();
    assert_eq!(loads, 0, "{found:?}");
}

/// A load, an add and a store fuse only when the store writes the cell the
/// load read. Two cells through different registers compared equal, since
/// `Mem`'s equality leaves out the registers: deedlines' CREATEOBJECT added
/// z%(i) to the source row instead of storing it to the destination.
#[test]
fn test_a_store_through_another_register_is_not_fused() {
    let (dx, ax) = (rl(RegId::DX, 2), rl(RegId::AX, 2));
    let through = |value: u32, register: RegId| {
        Loc::Mem(Mem { base: Some(Held { value, width: 2 }), through: register, ..Mem::new(None, 2) })
    };
    let head: Vec<Arc<Insn>> = vec![
        Arc::new(insn(
            1,
            None,
            Some(sem(Operation::Move, "mov", vec![dx.clone()], vec![through(1, RegId::DI)])),
            vec![],
            vec![1],
        )),
        Arc::new(insn(
            2,
            None,
            Some(sem(Operation::Binary, "add", vec![dx.clone()], vec![dx.clone(), ax])),
            vec![],
            vec![],
        )),
        Arc::new(insn(
            3,
            None,
            Some(sem(Operation::Move, "mov", vec![through(2, RegId::SI)], vec![dx.clone()])),
            vec![],
            vec![2],
        )),
    ];
    let overwrite = insn(4, None, Some(sem(Operation::Move, "mov", vec![dx], vec![rl(RegId::CX, 2)])), vec![], vec![]);
    let input = body(
        "two-cells",
        0,
        vec![block(0, head.clone(), vec![1]).into(), block(1, vec![Arc::new(overwrite)], vec![]).into()],
    );

    assert_eq!(whats(&fused(rules(), &input).blocks[0].insns), whats(&head));
}

/// mandel's column counted to zero in memory kept `add [bp-10],1;
/// cmp [bp-10],0; jne` across an anchor: the add's flags were the test.
#[test]
fn test_a_zero_test_of_a_cell_after_its_step_reads_the_step() {
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(frame(-10), 2) });
    let step = insn(
        0,
        Some((0, 4)),
        Some(sem(Operation::Binary, "add", vec![cell.clone()], vec![cell.clone(), im(1, 2)])),
        vec![],
        vec![],
    );
    let compare =
        insn(4, Some((4, 8)), Some(sem(Operation::Compare, "cmp", vec![], vec![cell, im(0, 2)])), vec![], vec![]);
    let branch = insn(8, Some((8, 10)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(0))), vec![], vec![]);
    let ax = rl(RegId::AX, 2);
    let flags = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "xor", vec![ax.clone()], vec![ax.clone(), ax])),
        vec![],
        vec![],
    );
    let exit = insn(12, Some((12, 13)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]);
    let anchor = insn(4, None, Some(sem(Operation::Nothing, "", vec![], vec![])), vec![7], vec![5]);
    let blocks = vec![
        block(0, vec![Arc::new(step), Arc::new(anchor), Arc::new(compare), Arc::new(branch)], vec![0, 10]),
        block(10, vec![Arc::new(flags), Arc::new(exit)], vec![]),
    ];
    let names = tested(rules(), &body("zero", 0, crate::model::lir::shared(blocks))).blocks[0]
        .insns
        .iter()
        .map(|one| one.what.as_ref().and_then(|what| what.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(names, ["add", "", "", "jne"]);
}

#[test]
fn test_a_borrow_mask_needs_no_zero() {
    // A huge pointer's step spread its borrow as `mov si,0 ; sbb si,0`, one
    // instruction more than `sbb si,si` each element of bench/huge.
    let (dx, si) = (rl(RegId::DX, 2), rl(RegId::SI, 2));
    let insns = vec![
        Arc::new(insn(
            0,
            Some((0, 0)),
            Some(sem(Operation::Binary, "add", vec![dx.clone()], vec![dx.clone(), im(4, 2)])),
            vec![1],
            vec![1],
        )),
        Arc::new(insn(
            1,
            Some((1, 1)),
            Some(sem(Operation::Move, "mov", vec![si.clone()], vec![im(0, 2)])),
            vec![2],
            vec![],
        )),
        Arc::new(insn(
            2,
            Some((2, 2)),
            Some(sem(Operation::Binary, "sbb", vec![si.clone()], vec![si.clone(), im(0, 2)])),
            vec![2],
            vec![2],
        )),
        Arc::new(insn(
            3,
            Some((3, 3)),
            Some(sem(Operation::Binary, "and", vec![si.clone()], vec![si.clone(), im(4096, 2)])),
            vec![2],
            vec![2],
        )),
    ];
    let input = body("borrow", 0, vec![block(0, insns, vec![]).into()]);

    let result = borrows(rules(), &input);

    assert_eq!(
        names(
            &result
                .insns()
                .into_iter()
                .filter(|one| one.what.as_ref().is_some_and(|what| what.op != Operation::Nothing))
                .collect::<Vec<_>>()
        ),
        ["add", "sbb", "and"]
    );
    assert!(
        result
            .insns()
            .iter()
            .any(|one| one.what == Some(sem(Operation::Binary, "sbb", vec![si.clone()], vec![si.clone(), si.clone()])))
    );
}

/// A dword copy and add run under the operand-size prefix each, so on a CPU
/// that charges a prefix they cost two more than their prices: `mov edx,ecx;
/// add edx,1024` is four against the 67h lea's four, and one instruction.
/// Priced without it, 2 against 3 kept both (SPHEREMAPLASMA, #386; mandel
/// -3% instructions when it became lea).
#[test]
fn test_a_dword_copy_and_add_are_priced_with_their_operand_size_prefixes() {
    let (dest, source) = (rl(RegId::EDX, 4), rl(RegId::ECX, 4));
    let copy = insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Move, "mov", vec![dest.clone()], vec![source.clone()])),
        vec![2],
        vec![1],
    );
    let add = insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "add", vec![dest.clone()], vec![dest.clone(), im(1024, 4)])),
        vec![2],
        vec![2],
    );
    let compare =
        insn(2, Some((2, 2)), Some(sem(Operation::Compare, "cmp", vec![], vec![dest, im(0, 4)])), vec![], vec![2]);
    let input =
        body("dword-offset", 0, vec![block(0, vec![Arc::new(copy), Arc::new(add), Arc::new(compare)], vec![]).into()]);
    let result = addresses(&input, "486").unwrap().insns();
    assert_eq!(names(&result), ["lea", "cmp"]);
}

// ------------------------------------------------------------ flat zero
// extensions

fn flat() -> &'static crate::backend::peep::Rules {
    &crate::backend::targets::x86_m32::RULES
}

fn word_load(into: RegId) -> Arc<Insn> {
    let cell = Loc::Mem(mem(frame(8), 2, RegId::EBP, 0, 4));
    Arc::new(insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Extend, "movzx", vec![rl(into, 4)], vec![cell])),
        vec![],
        vec![],
    ))
}

fn partial_load(into: RegId) -> Arc<Insn> {
    let cell = Loc::Mem(mem(frame(8), 2, RegId::EBP, 0, 4));
    Arc::new(insn(0, Some((0, 0)), Some(sem(Operation::Move, "mov", vec![rl(into, 2)], vec![cell])), vec![], vec![]))
}

fn extension(
    into: RegId,
    from: RegId,
) -> Arc<Insn> {
    Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Extend, "movzx", vec![rl(into, 4)], vec![rl(from, 2)])),
        vec![],
        vec![],
    ))
}

fn flat_body(insns: Vec<Arc<Insn>>) -> LirBody {
    body("flat", 0, vec![block(0, insns, vec![]).into()])
}

/// A word whose register is zero above it, extended to another register: the
/// register's own copy. The extension was a three clock `movzx` where a one
/// clock `mov` does, and kept the copy from joining its source.
#[test]
fn test_flat_extension_of_a_zero_extended_word_is_a_copy() {
    let made = zero_extensions(flat(), &flat_body(vec![word_load(RegId::EAX), extension(RegId::ESI, RegId::AX)]));
    let kept: Vec<Semantics> = whats(&made.insns());
    assert_eq!(kept[1].name.as_deref(), Some("mov"), "{kept:?}");
    assert_eq!((kept[1].dests.clone(), kept[1].sources.clone()), (vec![rl(RegId::ESI, 4)], vec![rl(RegId::EAX, 4)]));
}

/// The same extension in place is nothing.
#[test]
fn test_flat_extension_in_place_of_a_zero_extended_word_is_dropped() {
    let made = zero_extensions(flat(), &flat_body(vec![word_load(RegId::EAX), extension(RegId::EAX, RegId::AX)]));
    assert_eq!(
        names(
            &whats(&made.insns())
                .into_iter()
                .map(|what| Arc::new(insn(0, None, Some(what), vec![], vec![])))
                .collect::<Vec<_>>()
        ),
        ["movzx"]
    );
}

/// Where the register's upper half is not known zero the extension stays: a
/// word load leaves it as it was. Taking it for a copy read the old upper half.
#[test]
fn test_flat_extension_of_a_partial_load_stays() {
    let made = zero_extensions(flat(), &flat_body(vec![partial_load(RegId::EAX), extension(RegId::ESI, RegId::AX)]));
    assert_eq!(whats(&made.insns())[1].name.as_deref(), Some("movzx"));
}

/// A word copy of a zero-extended register is the dword one when nothing reads
/// the upper half it was leaving.
#[test]
fn test_flat_word_copy_of_a_zero_extended_register_is_a_dword_copy() {
    let copy = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::CX, 2)], vec![rl(RegId::AX, 2)])),
        vec![],
        vec![],
    ));
    // Overwritten whole after, so nothing reads the upper half the word copy
    // was leaving.
    let cleared = Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Binary, "xor", vec![rl(RegId::ECX, 4)], vec![rl(RegId::ECX, 4), rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ));
    let made = widened_moves(flat(), &flat_body(vec![word_load(RegId::EAX), copy, cleared]));
    let kept = whats(&made.insns());
    assert_eq!((kept[1].dests.clone(), kept[1].sources.clone()), (vec![rl(RegId::ECX, 4)], vec![rl(RegId::EAX, 4)]));
}

fn compared() -> Arc<Insn> {
    Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::EAX, 4), rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ))
}

fn dword_sum() -> Arc<Insn> {
    Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::EAX, 4)], vec![rl(RegId::EAX, 4), rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ))
}

/// A dword sum and the extension of its low word, where the sum's register is
/// zero above: the word sum.
#[test]
fn test_flat_dword_sum_and_its_extension_is_the_word_sum() {
    // A compare after overwrites the flags the word sum would set differently.
    let made = narrowed_arithmetic(
        flat(),
        &flat_body(vec![word_load(RegId::EAX), dword_sum(), extension(RegId::EAX, RegId::AX), compared()]),
    );
    let kept = whats(&made.insns());
    assert_eq!(kept.len(), 3, "{kept:?}");
    assert_eq!(
        (kept[1].dests.clone(), kept[1].sources.clone()),
        (vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), rl(RegId::CX, 2)])
    );
}

/// Not where it is not: the word sum leaves the upper half as it was, which the
/// extension would have cleared. Taking the word sum there left the old upper
/// half in a register read as a dword.
#[test]
fn test_flat_dword_sum_stays_where_the_upper_half_may_not_be_zero() {
    let made = narrowed_arithmetic(
        flat(),
        &flat_body(vec![partial_load(RegId::EAX), dword_sum(), extension(RegId::EAX, RegId::AX), compared()]),
    );
    let kept = whats(&made.insns());
    assert_eq!((kept[1].name.as_deref(), kept[2].name.as_deref()), (Some("add"), Some("movzx")), "{kept:?}");
}

// ------------------------------------------------------- copies forwarded into
// addresses

/// `mov esi, ecx` then a store through `[ebp+esi-8]`: the store reads ECX. Only
/// the register operands were forwarded, so an address kept the copy's register
/// and the copy stayed (`Mem` equality leaves registers out, so a rewritten
/// cell also read as unchanged and was dropped).
#[test]
fn test_a_copy_is_forwarded_into_the_address_that_reads_it() {
    let copy = Arc::new(insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::ESI, 4)], vec![rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ));
    let cell = Mem {
        index_through: RegId::ESI,
        index: Some(Held { value: 32, width: 4 }),
        ..mem(Some(Addr { segment: (RegId::SS).iced(), ..Addr::new(Space::Literal, -1032) }), 1, RegId::EBP, 0, 2)
    };
    let store = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![im(1, 1)])),
        vec![],
        vec![],
    ));
    let made = machinecp::propagated(&flat_body(vec![copy, store]));
    let Some(Loc::Mem(cell)) = made.insns()[1].what.as_ref().map(|what| what.dests[0].clone()) else {
        panic!("a store")
    };
    assert_eq!(cell.index_through, RegId::ECX);
}

/// Not where the older register changes between the copy and the read.
#[test]
fn test_a_copy_is_not_forwarded_past_a_write_of_its_source() {
    let copy = Arc::new(insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::ESI, 4)], vec![rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ));
    let bump = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::ECX, 4)], vec![rl(RegId::ECX, 4), im(1, 4)])),
        vec![],
        vec![],
    ));
    let cell = Mem {
        index_through: RegId::ESI,
        index: Some(Held { value: 32, width: 4 }),
        ..mem(Some(Addr { segment: (RegId::SS).iced(), ..Addr::new(Space::Literal, -1032) }), 1, RegId::EBP, 0, 2)
    };
    let store = Arc::new(insn(
        2,
        Some((2, 2)),
        Some(sem(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![im(1, 1)])),
        vec![],
        vec![],
    ));
    let made = machinecp::propagated(&flat_body(vec![copy, bump, store]));
    let Some(Loc::Mem(cell)) = made.insns()[2].what.as_ref().map(|what| what.dests[0].clone()) else {
        panic!("a store")
    };
    assert_eq!(cell.index_through, RegId::ESI);
}

// ------------------------------------------------------------- the mode is the
// body's

/// `mov ebx, ecx` then a byte read through `[ebx]`. A 32-bit base encodes only
/// in 32-bit mode.
fn read_through_a_copy(bits: u32) -> LirBody {
    let copy = Arc::new(insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    ));
    let read = Arc::new(insn(
        1,
        Some((1, 1)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::AL, 1)], vec![Loc::Mem(mem(None, 1, RegId::EBX, 0, 0))])),
        vec![],
        vec![],
    ));
    LirBody { bits, ..flat_body(vec![copy, read]) }
}

fn through_of_the_read(body: &LirBody) -> RegId {
    match &body.insns()[1].what.as_ref().expect("a read").sources[0] {
        Loc::Mem(cell) => cell.through,
        other => panic!("{other:?}"),
    }
}

/// What a body encodes and touches depends on the mode it is for, which it
/// carries: a flat body forwards a copy into a 32-bit base, a real-mode one
/// does not (it has no such operand). The mode was a thread-local set around a
/// compile, so a pass run on a body outside one read real mode whatever the
/// body was for.
#[test]
fn test_a_body_is_encoded_in_its_own_mode() {
    assert_eq!(through_of_the_read(&machinecp::propagated(&read_through_a_copy(32))), RegId::ECX);
    assert_eq!(through_of_the_read(&machinecp::propagated(&read_through_a_copy(16))), RegId::EBX);
}

// ----------------------------------------------------------------- what a jump
// reads

/// The flags a conditional jump reads are the condition's: what m16's rows say
/// of each jump, iced states of the mnemonic, so a target whose jump rows are
/// its own needs no copy of them.
#[test]
fn test_what_a_conditional_jump_reads_is_the_conditions_for_every_condition() {
    let rows: Vec<_> = llrm_x86_m16::instructions::FORMS.iter().filter(|form| form.operation == "branch").collect();
    assert_eq!(rows.len(), 16);
    for form in rows {
        let branch = Semantics { name: Some(form.name.as_str().into()), ..Semantics::new(Operation::Branch) };
        let wanted = _flag_lanes(llrm_x86_m16::instructions::flags(form).expect("a jump reads flags").0);
        assert_eq!(_branch_reads(&branch), wanted, "{}", form.name);
    }
}

// ------------------------------------------------------------------ the count
// of a `rep`

fn repeated_fill(
    name: &str,
    width: u32,
    di: RegId,
    cx: RegId,
    ax: RegId,
) -> Insn {
    // Real mode's string store names its segment: the selector it stores
    // through.
    let selector = if di == RegId::DI { vec![rl(RegId::ES, 2)] } else { vec![] };
    let what = sem(
        Operation::Fill,
        name,
        vec![Loc::Mem(Mem::new(None, 0)), rl(di, width), rl(cx, width)],
        [vec![rl(ax, width), rl(cx, width), rl(di, width)], selector].concat(),
    );
    insn(0, Some((0, 0)), Some(what), vec![], vec![])
}

/// `rep stosw` in 16-bit code counts CX down and leaves the upper half of ECX
/// alone. `rep` was taken to write ECX whole, so a value live in ECX's upper
/// half across it read as overwritten.
#[test]
fn test_a_rep_in_real_mode_writes_cx_and_not_the_upper_half_of_ecx() {
    let regs = crate::backend::registerinfo::test_regs();
    let (_, writes) =
        _register_effects(regs, 16, &repeated_fill("stosw", 2, RegId::DI, RegId::CX, RegId::AX), true, false)
            .expect("encodes");
    assert!(writes.contains(&(RegId::ECX, 0)) && writes.contains(&(RegId::ECX, 1)), "{writes:?}");
    assert!(!writes.contains(&(RegId::ECX, 2)) && !writes.contains(&(RegId::ECX, 3)), "{writes:?}");
}

/// In 32-bit code it counts ECX.
#[test]
fn test_a_rep_in_flat_mode_writes_ecx_whole() {
    let regs = llrm_lir::registers::Regs(&llrm_x86_m32::REGISTER_INFO);
    let (_, writes) =
        _register_effects(regs, 32, &repeated_fill("stosd", 4, RegId::EDI, RegId::ECX, RegId::EAX), true, false)
            .expect("encodes");
    assert!((0..4).all(|lane| writes.contains(&(RegId::ECX, lane))), "{writes:?}");
}

/// Every effect was worked out by assembling the instruction and decoding the
/// bytes (8% of compiling matmul, #560). An instruction with a row in
/// `x86.instr` is answered from it, and one without is not known, not decoded.
#[test]
fn test_an_instruction_with_a_row_is_answered_from_the_table_and_one_without_is_unknown() {
    let regs = crate::backend::registerinfo::test_regs();
    let table = sem(Operation::Binary, "add", vec![rl(RegId::CX, 2)], vec![rl(RegId::CX, 2), rl(RegId::DX, 2)]);
    let (reads, writes) = _register_effects_of_what(regs, 16, &table, true, true).expect("the table answers");
    assert!(reads.contains(&(RegId::ECX, 0)) && reads.contains(&(RegId::EDX, 1)) && writes.contains(&(RegId::ECX, 1)));
    let none = sem(Operation::Unary, "nosuchop", vec![rl(RegId::EAX, 4)], vec![rl(RegId::EAX, 4)]);
    assert_eq!(_register_effects_of_what(regs, 32, &none, true, true), None);
}

/// A cache of an instruction's effects was given a memory operand equal, when
/// `Mem`'s equality left out its encoding fields, to one spelled through
/// another register (queens -Os: `mov es,[bx+2]` for `mov es,[si+2]`).
#[test]
fn test_two_cells_equal_but_spelled_through_other_registers_have_their_own_effects() {
    let regs = crate::backend::registerinfo::test_regs();
    let through = |register| {
        let cell = Mem { through: register, offset: 2, base: Some(Held { value: 1, width: 2 }), ..Mem::new(None, 2) };
        sem(Operation::Move, "mov", vec![rl(RegId::ES, 2)], vec![Loc::Mem(cell)])
    };
    // One place in memory, the second instruction put where the first was (as a
    // body's rewrite does).
    let mut place = through(RegId::BX);
    assert!(
        place.same_meaning(&through(RegId::SI)) && place != through(RegId::SI),
        "the same cell, spelled another way, is not equal to it"
    );
    let (first, _) = _register_effects_of_what(regs, 16, &place, true, false).expect("the table answers");
    place = through(RegId::SI);
    let (second, _) = _register_effects_of_what(regs, 16, &place, true, false).expect("the table answers");
    assert!(first.contains(&(RegId::EBX, 0)) && !first.contains(&(RegId::ESI, 0)));
    assert!(second.contains(&(RegId::ESI, 0)) && !second.contains(&(RegId::EBX, 0)), "[si+2] answered as [bx+2]");
}

/// d_faces -O2: 6,364 asks, 3,843 worked out, 12 M Ir of a 44 M peephole. The
/// effects read neither an immediate (but a shift count that is a multiple of
/// 32) nor a displacement, so instructions that differ in them were worked out
/// one by one.
#[test]
fn test_instructions_differing_in_an_immediate_or_a_displacement_are_worked_out_once() {
    let regs = crate::backend::registerinfo::test_regs();
    let add = |value| {
        insn(
            0,
            Some((0, 2)),
            Some(sem(Operation::Binary, "add", vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), im(value, 2)])),
            vec![],
            vec![],
        )
    };
    let load = |offset| {
        insn(
            0,
            Some((0, 2)),
            Some(sem(
                Operation::Move,
                "mov",
                vec![rl(RegId::AX, 2)],
                vec![Loc::Mem(mem(None, 2, RegId::BX, offset, 0))],
            )),
            vec![],
            vec![],
        )
    };
    let before = effects_computed();
    let (add_first, load_first) =
        (_register_effects(regs, 16, &add(1), false, true), _register_effects(regs, 16, &load(2), false, true));
    for value in 2..6 {
        assert_eq!(_register_effects(regs, 16, &add(value), false, true), add_first);
        assert_eq!(_register_effects(regs, 16, &load(value * 2), false, true), load_first);
    }
    assert_eq!(effects_computed() - before, 2, "worked out again for an immediate or a displacement");
    let shift = |count| {
        insn(
            0,
            Some((0, 2)),
            Some(sem(Operation::Binary, "shl", vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), im(count, 1)])),
            vec![],
            vec![],
        )
    };
    assert_ne!(
        _register_effects(regs, 16, &shift(32), false, true),
        _register_effects(regs, 16, &shift(1), false, true),
        "a shift by a multiple of 32 changes no flag"
    );
}

/// The passes asked `_register_effects` of the same instruction again and
/// again, each time lowering it to text and decoding it: 886,000 asks for
/// 16,000 distinct instructions in one QCport module. An instruction equal to
/// one already answered is answered from it, whichever `Insn` it is held in.
#[test]
fn test_an_instruction_equal_to_one_already_answered_is_not_worked_out_again() {
    let regs = crate::backend::registerinfo::test_regs();
    let add = |at| {
        insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Binary, "add", vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), rl(RegId::BX, 2)])),
            vec![],
            vec![],
        )
    };
    let (before, hashed) = (effects_computed(), effects_hashed());
    let first = _register_effects(regs, 16, &add(0), false, true);
    // A miss hashed the ask again to remember it: 1 M Ir of d_faces -O2.
    assert_eq!(effects_hashed() - hashed, 1, "an ask hashed more than once");
    for at in 1..6 {
        assert_eq!(_register_effects(regs, 16, &add(at), false, true), first);
    }
    assert_eq!(effects_computed() - before, 1, "worked out again for each ask");
    let other = insn(
        0,
        Some((0, 2)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), rl(RegId::CX, 2)])),
        vec![],
        vec![],
    );
    assert_ne!(
        _register_effects(regs, 16, &other, false, true),
        first,
        "another instruction was given this one's answer"
    );
}

/// `dead_at_exit` was worked out for the body ten passes in a row, though all
/// but one of them left it as it was (#924). A body that is the same is
/// answered from the last; one whose instruction changed is not.
#[test]
fn test_the_dead_lanes_of_a_body_no_pass_changed_are_worked_out_once() {
    let mov = |at, to, from| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Move, "mov", vec![rl(to, 2)], vec![rl(from, 2)])),
            vec![],
            vec![],
        ))
    };
    let input =
        body("f", 0, vec![block(0, vec![mov(0, RegId::AX, RegId::BX), mov(2, RegId::CX, RegId::AX)], vec![]).into()]);
    let before = liveness::exits_computed();
    let first = liveness::dead_at_exit(&input);
    for _ in 0..5 {
        assert_eq!(liveness::dead_at_exit(&input.clone()), first);
    }
    assert_eq!(liveness::exits_computed() - before, 1, "worked out again for each ask");
    let changed =
        body("f", 0, vec![block(0, vec![mov(0, RegId::AX, RegId::BX), mov(2, RegId::DX, RegId::AX)], vec![]).into()]);
    liveness::dead_at_exit(&changed);
    assert_eq!(liveness::exits_computed() - before, 2, "a changed body was given the last answer");
}

/// `high_extracts` solved the function's liveness and walked every block for
/// the dead lanes after each instruction, though none held a shift to extract
/// (3.0 M Ir of d_faces -O2). A body with nothing to extract is answered
/// unchanged without a solve.
#[test]
fn test_a_body_with_nothing_to_extract_is_not_solved_for_high_extracts() {
    let mov = Arc::new(insn(
        0,
        Some((0, 2)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 4)], vec![rl(RegId::BX, 4)])),
        vec![],
        vec![],
    ));
    let input = body("f", 0, vec![block(0, vec![mov], vec![]).into()]);
    let before = liveness::exits_computed();
    let done = high_extracts(&input, "386").unwrap();
    assert_eq!(done, input);
    assert_eq!(liveness::exits_computed() - before, 0, "solved for a body with nothing to extract");
}

/// `addresses` solved the flags live out of every block and the registers live
/// out of the function for each body, though no instruction in it starts a
/// chain to fold into an address (7.5 M Ir of d_faces -O2). A body with nothing
/// to fold is answered unchanged without either.
#[test]
fn test_a_body_with_nothing_to_fold_is_not_solved_for_addresses() {
    let load = Arc::new(insn(
        0,
        Some((0, 2)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 4)], vec![Loc::Mem(mem(frame(8), 4, RegId::BP, 8, 1))])),
        vec![],
        vec![],
    ));
    let input = body("f", 0, vec![block(0, vec![load], vec![]).into()]);
    let before = flags_computed();
    let done = addresses(&input, "386").unwrap();
    assert_eq!(done, input);
    assert_eq!(flags_computed() - before, 0, "flag liveness solved for a body with nothing to fold");
}

/// `popped_arguments` solved the function's liveness and walked every block
/// for the dead lanes after each instruction, though no instruction in it
/// cleans a call's arguments (1.69 M Ir of d_faces -O2). A body with no
/// `add sp, n` is answered unchanged without a solve.
#[test]
fn test_a_body_with_no_argument_cleanup_is_not_solved_for_popped_arguments() {
    let mov = Arc::new(insn(
        0,
        Some((0, 2)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![rl(RegId::BX, 2)])),
        vec![],
        vec![],
    ));
    let input = body("f", 0, vec![block(0, vec![mov], vec![]).into()]);
    let before = liveness::exits_computed();
    let done = popped_arguments(&input, &crate::backend::cpu::tuned("486", true).unwrap()).unwrap();
    assert_eq!(done, input);
    assert_eq!(liveness::exits_computed() - before, 0, "solved for a body with nothing to pop");
}

/// The sub-passes of the peephole ask for the lanes live out of each block in
/// turn, and each that changes the body made the next solve the whole function
/// again (39 solves for 11 functions of d_faces -O2). A body brought up to date
/// is solved from scratch once.
#[test]
fn test_the_peephole_solves_a_functions_lanes_from_scratch_once() {
    let mov = |at: i64, to: RegId, from: RegId| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Move, "mov", vec![rl(to, 2)], vec![rl(from, 2)])),
            vec![],
            vec![],
        ))
    };
    let add = |at: i64, to: RegId| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Binary, "add", vec![rl(to, 2)], vec![rl(to, 2), im(1, 2)])),
            vec![],
            vec![],
        ))
    };
    // A copy chain in the first block, which copy propagation and the dead-move
    // pass change, and a chain of blocks that nothing changes after it.
    let mut blocks: Vec<crate::model::lir::Block> = vec![
        LirBlock {
            succ: vec![10].into(),
            ..block(0, vec![mov(0, RegId::CX, RegId::BX), mov(2, RegId::AX, RegId::CX)], vec![])
        }
        .into(),
    ];
    for at in 10..16 {
        let more = if at == 15 { vec![0] } else { vec![at + 1, 99] };
        blocks.push(
            LirBlock {
                succ: more.into(),
                ..block(at, vec![add(at * 4, RegId::AX), add(at * 4 + 2, RegId::DX)], vec![])
            }
            .into(),
        );
    }
    blocks.push(block(99, vec![mov(400, RegId::BX, RegId::AX)], vec![]).into());
    let input = body("f", 0, blocks);
    let [whole, partly, _] = liveness::asks();
    let output = transform(input.clone());
    let [whole_now, partly_now, _] = liveness::asks();
    assert_eq!(whole_now - whole, 1, "solved from scratch more than once");
    assert!(output != input, "premise: the peephole changed the body");
    assert!(partly_now > partly, "premise: a changed body was asked about again");
}

/// A loop's header branched on the flags each trip, which kept them live round
/// the loop; the branch goes. Solved again from the answer that was held, the
/// flags stayed live round the loop on their own and a flag-setting `cmp` that
/// nothing reads was kept.
#[test]
fn test_a_flag_kept_live_round_a_loop_by_a_branch_that_is_gone_is_not_live() {
    let mov = |at: i64, to: RegId, from: RegId| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Move, "mov", vec![rl(to, 2)], vec![rl(from, 2)])),
            vec![],
            vec![],
        ))
    };
    let branch =
        Arc::new(insn(1, Some((1, 2)), Some(semt(Operation::Branch, "jne", vec![], vec![], Some(2))), vec![], vec![]));
    let compare = Arc::new(insn(
        5,
        Some((5, 7)),
        Some(sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::AX, 2), rl(RegId::BX, 2)])),
        vec![],
        vec![],
    ));
    let trip: crate::model::lir::Block = block(2, vec![mov(3, RegId::DX, RegId::BX)], vec![1]).into();
    let leave: crate::model::lir::Block = block(3, vec![compare], vec![]).into();
    let header = |insns| -> crate::model::lir::Block { block(1, insns, vec![2, 3]).into() };
    let body_of = |header| body("f", 1, vec![header, trip.clone(), leave.clone()]);
    let zero = _flag_lanes(RflagsBits::ZF);
    let before = _flags_live_out(&body_of(header(vec![branch])));
    assert!(!zero.is_disjoint(&before.entering[&2]), "premise: ZF is live round the loop");
    let changed = body_of(header(vec![mov(1, RegId::CX, RegId::BX)]));
    let after = _flags_live_out(&changed);
    assert!(zero.is_disjoint(&after.entering[&2]), "ZF is live out of the loop's body");
    assert!(after.entering == flags_from_scratch(&changed));
}

/// The flag liveness, upper-half zeroes and post-RA sink liveness each iterated
/// every block to a fixed point: a backward chain of 30 blocks took 30 rounds
/// of 30. All go through `dataflow::solve`, which works a block again only when
/// an input changed.
#[test]
fn test_flag_liveness_over_a_loop_is_not_worked_by_rounds() {
    let blocks = 30;
    let mut list = Vec::new();
    for at in 1..=blocks {
        let next = if at == blocks { vec![1, blocks + 1] } else { vec![at + 1] };
        list.push(block(at, vec![], next));
    }
    list.push(block(blocks + 1, vec![], vec![]));
    let input = body("loop", 1, crate::model::lir::shared(list));
    crate::analysis::dataflow::WORKED.with(|count| count.set(0));
    super::_flags_live_out(&input);
    let worked = crate::analysis::dataflow::WORKED.with(std::cell::Cell::get);
    assert!(worked <= 4 * blocks as usize, "{worked} blocks worked for a loop of {blocks}");
}

/// Copy propagation worked every block again each round until no exit changed:
/// a loop of 30 blocks took 30 rounds of 30 blocks. A block is worked again
/// when a parent's exit changed.
#[test]
fn test_copy_propagation_works_a_block_again_only_when_a_parent_changed() {
    let (ax, bx) = (rl(RegId::AX, 2), rl(RegId::BX, 2));
    let blocks = 30;
    let mut list = Vec::new();
    for at in 1..=blocks {
        let copy = insn(
            at,
            Some((at, at)),
            Some(sem(Operation::Move, "mov", vec![bx.clone()], vec![ax.clone()])),
            vec![],
            vec![],
        );
        let next = if at == blocks { vec![1, blocks + 1] } else { vec![at + 1] };
        list.push(block(at, if at == 1 { vec![Arc::new(copy)] } else { vec![] }, next));
    }
    list.push(block(blocks + 1, vec![], vec![]));
    let input = body("loop", 1, crate::model::lir::shared(list));
    crate::analysis::dataflow::WORKED.with(|count| count.set(0));
    crate::backend::machinecp::propagated(&input);
    let worked = crate::analysis::dataflow::WORKED.with(std::cell::Cell::get);
    assert!(worked <= 4 * blocks as usize, "{worked} blocks worked for a loop of {blocks}");
}

/// Every sub-pass walked every function's whole body, `machinecp` and
/// `regthrash` among them, found no copy in most and changed nothing: two
/// thirds of the peephole's time went to passes that found nothing. A body with
/// no copy is not given to copy propagation.
#[test]
fn test_a_function_without_a_copy_runs_copy_propagation_zero_times() {
    let add = |at, into, from| {
        Arc::new(insn(
            at,
            Some((at, at)),
            Some(sem(Operation::Binary, "add", vec![rl(into, 2)], vec![rl(into, 2), rl(from, 2)])),
            vec![],
            vec![],
        ))
    };
    let copy = |at, into, from| {
        Arc::new(insn(
            at,
            Some((at, at)),
            Some(sem(Operation::Move, "mov", vec![rl(into, 2)], vec![rl(from, 2)])),
            vec![],
            vec![],
        ))
    };
    let before = machinecp::runs();
    transform(one_block(vec![add(0, RegId::CX, RegId::DX), add(1, RegId::BX, RegId::CX)]));
    assert_eq!(machinecp::runs() - before, 0, "copy propagation ran on a body with no copy");
    let before = machinecp::runs();
    transform(one_block(vec![copy(0, RegId::AX, RegId::BX), add(1, RegId::CX, RegId::AX)]));
    assert!(machinecp::runs() - before >= 1, "copy propagation did not run on a body with a copy");
}

/// Every block was walked twice and its effects worked out for each of its
/// instructions, copies or none: 9.8 M instructions of the 77 M of `d_faces`'
/// peephole went to blocks that hold no copy. Only a block with a copy is
/// walked.
#[test]
fn test_copy_propagation_walks_only_the_blocks_that_hold_a_copy() {
    let add = |at| {
        Arc::new(insn(
            at,
            Some((at, at)),
            Some(sem(Operation::Binary, "add", vec![rl(RegId::CX, 2)], vec![rl(RegId::CX, 2), rl(RegId::DX, 2)])),
            vec![],
            vec![],
        ))
    };
    let copy = Arc::new(insn(
        0,
        Some((0, 0)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![rl(RegId::BX, 2)])),
        vec![],
        vec![],
    ));
    let blocks = (1..=5)
        .map(|at| {
            block(
                at,
                if at == 3 { vec![Arc::clone(&copy), add(at)] } else { vec![add(at)] },
                if at < 5 { vec![at + 1] } else { vec![] },
            )
        })
        .collect::<Vec<_>>();
    let input = body("copies", 1, crate::model::lir::shared(blocks));
    let before = machinecp::walks();
    machinecp::propagated(&input);
    assert_eq!(machinecp::walks() - before, 1, "five blocks, one with a copy");
}

/// The lanes of a register follow the description: a high byte is lane 1 of its
/// root, the stack pointer and what the file omits have none.
#[test]
fn the_lanes_of_a_register_are_the_descriptions() {
    let regs = crate::backend::registerinfo::test_regs();
    assert_eq!(_lanes(regs, RegId::AH), [(RegId::EAX, 1)].into_iter().collect());
    assert_eq!(_lanes(regs, RegId::BL), [(RegId::EBX, 0)].into_iter().collect());
    assert_eq!(_lanes(regs, RegId::DX), [(RegId::EDX, 0), (RegId::EDX, 1)].into_iter().collect());
    assert!(
        !_lanes(regs, RegId::EBP).is_empty()
            && _lanes(regs, RegId::ESP).is_empty()
            && _lanes(regs, RegId::XMM0).is_empty()
    );
}

/// `lea eax, [bx-52]` took a local's address after `mov ebp, ebx`: a frame
/// cell's base was renamed with the frame register's copy.
#[test]
fn a_frame_cells_base_is_not_renamed_with_a_copy_of_the_frame_register() {
    let regs = crate::backend::registerinfo::test_regs();
    let frame = Addr::new(Space::Frame, -52);
    let cell = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(Some(frame), 4) });
    assert_eq!(_register_operand(regs, &cell, RegId::BP, RegId::BX), cell);
    let address = Loc::Address(AddressRef {
        addr: Some(frame),
        through: RegId::BP,
        index_through: RegId::None,
        scale: 1,
        offset: 0,
        disp_width: 2,
    });
    assert_eq!(_register_operand(regs, &address, RegId::BP, RegId::BX), address);
    let pointer = Loc::Mem(Mem { through: RegId::BP, ..Mem::new(None, 4) });
    assert_eq!(_register_operand(regs, &pointer, RegId::BP, RegId::BX).address().map(|at| at.through), Some(RegId::BX));
}

/// A binary search tested `a[mid] < key` and then `a[mid] > key` in the else
/// arm with a second `cmp ecx,edi`: 2.1% of x_binsearch -O1's clocks, the
/// branch reading flags the first compare already set.
#[test]
fn test_a_compare_that_a_sole_predecessor_just_made_is_not_made_again() {
    let (cx, di) = (rl(RegId::CX, 4), rl(RegId::DI, 4));
    let compare = |at| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![cx.clone(), di.clone()])),
            vec![],
            vec![],
        ))
    };
    let branch = |at, name: &str, target| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(semt(Operation::Branch, name, vec![], vec![], Some(target))),
            vec![],
            vec![],
        ))
    };
    let ret = Arc::new(insn(20, Some((20, 21)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]));
    let blocks = vec![
        block(0, vec![compare(0), branch(2, "jl", 30)], vec![10, 30]),
        block(10, vec![compare(10), branch(12, "jle", 30)], vec![20, 30]),
        block(20, vec![ret.clone()], vec![]),
        block(30, vec![ret], vec![]),
    ];
    let names = |after: &LirBody, at: usize| {
        after.blocks[at]
            .insns
            .iter()
            .map(|one| one.what.as_ref().and_then(|what| what.name.as_deref().map(str::to_owned)).unwrap_or_default())
            .collect::<Vec<_>>()
    };
    let after = compares_again(&body("search", 0, crate::model::lir::shared(blocks)));
    assert_eq!(names(&after, 0), ["cmp", "jl"]);
    assert_eq!(names(&after, 1), ["", "jle"], "the second compare is made again");
}

#[test]
fn test_a_compare_of_other_operands_or_a_block_with_two_ways_in_is_made_again() {
    let (cx, di, si) = (rl(RegId::CX, 4), rl(RegId::DI, 4), rl(RegId::SI, 4));
    let compare = |at, left: &Loc, right: &Loc| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(Operation::Compare, "cmp", vec![], vec![left.clone(), right.clone()])),
            vec![],
            vec![],
        ))
    };
    let branch = |at, target| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(semt(Operation::Branch, "jl", vec![], vec![], Some(target))),
            vec![],
            vec![],
        ))
    };
    let ret = Arc::new(insn(20, Some((20, 21)), Some(sem(Operation::Return, "", vec![], vec![])), vec![], vec![]));
    let compares = |after: &LirBody, at: usize| {
        after.blocks[at]
            .insns
            .iter()
            .filter(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Compare))
            .count()
    };
    let other = vec![
        block(0, vec![compare(0, &cx, &di), branch(2, 30)], vec![10, 30]),
        block(10, vec![compare(10, &cx, &si), branch(12, 30)], vec![20, 30]),
        block(20, vec![ret.clone()], vec![]),
        block(30, vec![ret.clone()], vec![]),
    ];
    assert_eq!(compares(&compares_again(&body("other", 0, crate::model::lir::shared(other))), 1), 1);
    // Block 10 is also entered from 5: the flags are not the compare's.
    let joined = vec![
        block(0, vec![compare(0, &cx, &di), branch(2, 30)], vec![10, 5]),
        block(5, vec![compare(5, &cx, &si), branch(7, 30)], vec![10, 30]),
        block(10, vec![compare(10, &cx, &di), branch(12, 30)], vec![20, 30]),
        block(20, vec![ret.clone()], vec![]),
        block(30, vec![ret], vec![]),
    ];
    assert_eq!(compares(&compares_again(&body("joined", 0, crate::model::lir::shared(joined))), 2), 1);
}

/// A sub-pass that matched nothing in a block rebuilt it anyway: 17,255 of the
/// 24,456 block visits of an `-O1` module's peephole allocated and dropped a
/// block for 514 that changed. An untouched block is the same allocation.
#[test]
fn a_pass_that_changes_nothing_returns_every_block_as_it_was() {
    let nop = |at: i64| {
        Arc::new(insn(
            at,
            Some((at, at)),
            Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![rl(RegId::BX, 2)])),
            vec![],
            vec![],
        ))
    };
    let input = body("untouched", 1, vec![block(1, vec![nop(1), nop(2)], vec![]).into()]);

    for result in [commuted(rules(), &input), borrows(rules(), &input), shuttles(rules(), &input)] {
        assert!(result.blocks[0].same_as(&input.blocks[0]));
        assert!(result.blocks[0].insns.same_as(&input.blocks[0].insns));
    }
}

/// The same frame address made twice into `bx` with `bx` untouched between
/// the two: the pin has no MachineCSE after allocation, and ours had one (the
/// second `lea` became an anchor, 3.7 M Ir of d_faces for the finds of 8 of
/// 568 objects). Both are emitted.
#[test]
fn test_a_repeated_frame_address_after_allocation_is_made_again() {
    let address = AddressRef {
        through: RegId::BP,
        offset: -100,
        disp_width: 1,
        ..AddressRef::new(Some(Addr::new(Space::Frame, -100)))
    };
    let lea = |at: i64, value: u32| {
        Arc::new(insn(
            at,
            Some((at, at + 3)),
            Some(sem(Operation::Address, "lea", vec![rl(RegId::BX, 2)], vec![Loc::Address(address.clone())])),
            vec![value],
            vec![],
        ))
    };
    let store = |at: i64, slot: i64| {
        Arc::new(insn(
            at,
            Some((at, at + 2)),
            Some(sem(
                Operation::Move,
                "mov",
                vec![Loc::Mem(mem(frame(slot), 2, RegId::BP, slot, 1))],
                vec![rl(RegId::BX, 2)],
            )),
            vec![],
            vec![],
        ))
    };
    let ret = Arc::new(insn(20, Some((20, 20)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![]));
    let insns = vec![lea(0, 1), store(4, -2), lea(8, 2), store(12, -4), ret];
    let result = transform(body("cse", 0, vec![block(0, insns, vec![]).into()]));
    let leas = whats(&result.blocks[0].insns).iter().filter(|what| what.name.as_deref() == Some("lea")).count();
    assert_eq!(leas, 2, "{:?}", names(&result.blocks[0].insns));
}

/// The pin gives `bsf` a `rep` prefix unless the function is optimised for size
/// or the flags are read (`X86MCInstLower`). It was on every `bsf`, a byte at
/// -Os m16 that main did not have.
#[test]
fn test_bsf_gets_the_rep_prefix_only_where_the_pin_gives_it() {
    for (label, size, flags_read, expected) in [
        ("speed, flags dead", false, false, true),
        ("size, flags dead", true, false, false),
        ("speed, flags read", false, true, false),
    ] {
        let scan = sem(Operation::Binary, "bsf", vec![rl(RegId::EAX, 4)], vec![rl(RegId::EAX, 4), rl(RegId::ECX, 4)]);
        let first = Arc::new(insn(10, Some((10, 11)), Some(scan), vec![], vec![]));
        let next = if flags_read {
            sem(Operation::Binary, "adc", vec![rl(RegId::SI, 4)], vec![rl(RegId::SI, 4), im(0, 4)])
        } else {
            sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::SI, 4), im(0, 4)])
        };
        let second = Arc::new(insn(11, Some((11, 12)), Some(next), vec![], vec![]));
        let input = body("scan", 0, vec![block(0, vec![first], vec![1]).into(), block(1, vec![second], vec![]).into()]);
        let cpu = crate::backend::cpu::tuned("486", size).unwrap();
        let result = crate::backend::repbsf::marked(&input, &cpu);
        let got = result.blocks[0].insns[0].what.as_ref().unwrap().repeat;
        assert_eq!(got, expected, "{label}");
    }
}

/// PLASMA at m16: `mov [bp-14],di ; mov di,[bp-14]` -- a reload of what the
/// register still held -- stood in each inner loop (a spilled one-instruction
/// piece of a split range, then the copy that follows it).
fn store_then_reload(between: Vec<Insn>) -> usize {
    let cell = || Loc::Mem(mem(frame(-14), 2, RegId::BP, -14, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut insns = vec![Arc::new(mov(0, vec![cell()], vec![rl(RegId::DI, 2)]))];
    insns.extend(between.into_iter().map(Arc::new));
    insns.push(Arc::new(mov(100, vec![rl(RegId::DI, 2)], vec![cell()])));
    insns.push(Arc::new(insn(
        200,
        Some((200, 200)),
        Some(sem(Operation::Return, "ret", vec![], vec![])),
        vec![],
        vec![],
    )));
    let done = forwarded_reloads(rules(), &body("forwarded-reload", 0, vec![block(0, insns, vec![]).into()]));
    whats(&done.blocks[0].insns).iter().filter(|what| what.name.as_deref() == Some("mov")).count()
}

/// huge_copy.c at m16 -Os: the forwarding dropped the reload `mov bl, [bp-2]`
/// and left `mov [bp-2], bl` in the loop, a store to a slot nothing reads
/// (the loop ran a dead byte store a trip, 5.1M clocks of a 17M run).
fn spill_slot_program(reload: bool) -> LirBody {
    spill_slot_program_of(reload, true)
}

fn spill_slot_program_of(
    reload: bool,
    spill: bool,
) -> LirBody {
    let cell = |disp: i64| Loc::Mem(mem(Some(Addr::new(Space::Frame, disp).in_slot(disp)), 2, RegId::BP, disp, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: spill && dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut insns = vec![Arc::new(mov(0, vec![cell(-14)], vec![rl(RegId::DI, 2)]))];
    if reload {
        insns.push(Arc::new(mov(100, vec![rl(RegId::AX, 2)], vec![cell(-14)])));
    }
    insns.push(Arc::new(insn(
        200,
        Some((200, 200)),
        Some(sem(Operation::Return, "ret", vec![], vec![])),
        vec![],
        vec![],
    )));
    body("unread-store", 0, vec![block(0, insns, vec![]).into()])
}

#[test]
fn test_a_store_to_a_spill_slot_nothing_reads_is_dropped() {
    let done = unread_spill_stores(&spill_slot_program(false));
    // The store's bytes stay on an anchor, no `mov` is left.
    assert_eq!(names(&done.blocks[0].insns), ["", "ret"]);
}

#[test]
fn test_a_store_to_a_spill_slot_something_reads_stays() {
    let done = unread_spill_stores(&spill_slot_program(true));
    assert_eq!(names(&done.blocks[0].insns), ["mov", "mov", "ret"]);
}

/// An alloca's store nothing reads is the program's own: va_copy, null and
/// pattern-fill listings in `isel_tests` lost theirs to the spill-slot pass.
#[test]
fn test_a_store_to_an_alloca_nothing_reads_stays() {
    let done = unread_spill_stores(&spill_slot_program_of(false, false));
    assert_eq!(names(&done.blocks[0].insns), ["mov", "ret"]);
}

#[test]
fn test_a_reload_straight_after_its_store_is_dropped() {
    assert_eq!(store_then_reload(vec![]), 1);
}

#[test]
fn test_a_reload_after_instructions_that_leave_the_register_and_cell_is_dropped() {
    let other = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::AX, 2)], vec![rl(RegId::AX, 2), rl(RegId::DI, 2)])),
        vec![],
        vec![],
    );
    assert_eq!(store_then_reload(vec![other]), 1);
}

#[test]
fn test_a_reload_stays_where_the_register_is_written_between() {
    let write = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::DI, 2)], vec![rl(RegId::DI, 2), im(1, 2)])),
        vec![],
        vec![],
    );
    assert_eq!(store_then_reload(vec![write]), 2);
}

#[test]
fn test_a_reload_stays_where_the_cell_is_written_between() {
    let write = insn(
        10,
        Some((10, 12)),
        Some(sem(
            Operation::Move,
            "mov",
            vec![Loc::Mem(mem(frame(-14), 2, RegId::BP, -14, 1))],
            vec![rl(RegId::AX, 2)],
        )),
        vec![],
        vec![],
    );
    assert_eq!(store_then_reload(vec![write]), 3);
}

#[test]
fn test_a_reload_stays_across_a_call() {
    let call = Insn {
        clobbers: [RegId::AX, RegId::CX, RegId::DX].into_iter().collect(),
        ..insn(10, Some((10, 13)), Some(sem(Operation::Call, "call", vec![], vec![])), vec![], vec![])
    };
    assert_eq!(store_then_reload(vec![call]), 2);
}

#[test]
fn test_a_reload_stays_across_a_volatile_access() {
    let device = Insn {
        volatile: true,
        ..insn(
            10,
            Some((10, 12)),
            Some(sem(Operation::Move, "mov", vec![rl(RegId::AX, 2)], vec![Loc::Mem(mem(None, 2, RegId::BX, 0, 0))])),
            vec![],
            vec![],
        )
    };
    assert_eq!(store_then_reload(vec![device]), 3);
}

/// nib quicksort `_sort` at m16, the swap block: `mov [bp-14],dx ; mov [si],cx
/// ; mov dx,[bp-14]` kept a reload of what `dx` still held because `[si]` might
/// be the slot: 2 operands a swap, 9,286 over the run. Returns the movs left.
fn reload_across_a_pointer_store(spill: bool) -> usize {
    let cell = || Loc::Mem(mem(Some(Addr::new(Space::Frame, -14).in_slot(-14)), 2, RegId::BP, -14, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>, spill_store: bool| Insn {
        spill_store,
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let through_si = Loc::Mem(mem(None, 2, RegId::SI, 0, 0));
    let mut insns = vec![
        Arc::new(mov(0, vec![cell()], vec![rl(RegId::DX, 2)], spill)),
        Arc::new(mov(10, vec![through_si], vec![rl(RegId::CX, 2)], false)),
        Arc::new(mov(100, vec![rl(RegId::DX, 2)], vec![cell()], false)),
    ];
    insns.push(Arc::new(insn(
        200,
        Some((200, 200)),
        Some(sem(Operation::Return, "ret", vec![], vec![])),
        vec![],
        vec![],
    )));
    let done = forwarded_reloads(rules(), &body("forwarded-reload", 0, vec![block(0, insns, vec![]).into()]));
    whats(&done.blocks[0].insns).iter().filter(|what| what.name.as_deref() == Some("mov")).count()
}

#[test]
fn test_a_spill_slot_is_not_reached_by_a_store_through_a_pointer() {
    assert_eq!(reload_across_a_pointer_store(true), 2);
}

#[test]
fn test_a_cell_that_is_no_spill_slot_may_be_reached_by_a_store_through_a_pointer() {
    assert_eq!(reload_across_a_pointer_store(false), 3);
}

/// quicksort `_sort` at m16 -Os: `mov [bp-2],cx ; ... ; mov bx,[bp-2]` read a
/// cell back into another register while `cx` still held it. Returns the
/// reload's source register, or `None` when it still reads the cell.
fn store_then_reload_elsewhere(between: Vec<Insn>) -> Option<RegId> {
    let cell = || Loc::Mem(mem(frame(-2), 2, RegId::BP, -2, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut insns = vec![Arc::new(mov(0, vec![cell()], vec![rl(RegId::CX, 2)]))];
    insns.extend(between.into_iter().map(Arc::new));
    insns.push(Arc::new(mov(100, vec![rl(RegId::BX, 2)], vec![cell()])));
    insns.push(Arc::new(insn(
        200,
        Some((200, 200)),
        Some(sem(Operation::Return, "ret", vec![], vec![])),
        vec![],
        vec![],
    )));
    let done = forwarded_copies(rules(), &body("forwarded-copy", 0, vec![block(0, insns, vec![]).into()]));
    let reload = whats(&done.blocks[0].insns).into_iter().rev().nth(1)?.clone();
    match reload.sources.as_slice() {
        [Loc::Reg(held)] => Some(held.register),
        _ => None,
    }
}

#[test]
fn test_a_reload_into_another_register_copies_the_register_that_filled_the_cell() {
    assert_eq!(store_then_reload_elsewhere(vec![]), Some(RegId::CX));
}

#[test]
fn test_a_reload_into_another_register_stays_where_the_filling_register_is_written() {
    let write = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Binary, "add", vec![rl(RegId::CX, 2)], vec![rl(RegId::CX, 2), im(1, 2)])),
        vec![],
        vec![],
    );
    assert_eq!(store_then_reload_elsewhere(vec![write]), None);
}

/// x87 `lrint` at m16 -Os: `fistp [bp-4] ; mov eax,[bp-4] ; mov [bp-8],eax`
/// kept the value in a second cell across a call, 4 bytes of frame and one
/// instruction more than main. Returns the cells the reloads at the end read
/// and the stores to a frame cell that are left.
fn copied_cell(between: Vec<Insn>) -> Option<(Vec<i64>, usize)> {
    let cell = |at: i64| Loc::Mem(mem(frame(at), 4, RegId::BP, at, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut insns = vec![
        Arc::new(mov(0, vec![rl(RegId::EAX, 4)], vec![cell(-4)])),
        Arc::new(mov(2, vec![cell(-8)], vec![rl(RegId::EAX, 4)])),
    ];
    insns.extend(between.into_iter().map(Arc::new));
    insns.push(Arc::new(mov(100, vec![rl(RegId::EAX, 4)], vec![cell(-8)])));
    insns.push(Arc::new(insn(
        200,
        Some((200, 200)),
        Some(sem(Operation::Return, "ret", vec![], vec![])),
        vec![],
        vec![],
    )));
    let done = crate::backend::slotcopies::merged(&body("slot-copy", 0, vec![block(0, insns, vec![]).into()]))?;
    let all = whats(&done.blocks[0].insns);
    let reads = all
        .iter()
        .filter_map(|what| match what.sources.as_slice() {
            [Loc::Mem(cell)] => cell.addr.map(|addr| addr.disp),
            _ => None,
        })
        .collect();
    let stores = all.iter().filter(|what| matches!(what.dests.as_slice(), [Loc::Mem(_)])).count();
    Some((reads, stores))
}

#[test]
fn test_a_spill_copied_from_a_cell_reads_that_cell() {
    assert_eq!(copied_cell(vec![]), Some((vec![-4, -4], 0)));
}

#[test]
fn test_a_spill_copied_from_a_cell_stays_where_the_cell_is_written_before_its_read() {
    let rewrite = insn(
        10,
        Some((10, 12)),
        Some(sem(Operation::Move, "mov", vec![Loc::Mem(mem(frame(-4), 4, RegId::BP, -4, 1))], vec![rl(RegId::ECX, 4)])),
        vec![],
        vec![],
    );
    assert_eq!(copied_cell(vec![rewrite]), None);
}

// ------------------------------------------------------------ width and flag
// audit of the width- and flag-changing rules

fn jump_if(
    name: &str,
    target: i64,
) -> Arc<Insn> {
    Arc::new(insn(9, Some((9, 9)), Some(semt(Operation::Branch, name, vec![], vec![], Some(target))), vec![], vec![]))
}

fn names_of(made: &LirBody) -> Vec<String> {
    whats(&made.insns()).iter().map(|what| what.name.as_deref().unwrap_or_default().to_owned()).collect()
}

fn point(
    at: i64,
    what: Semantics,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    Arc::new(insn(at, Some((at, at)), Some(what), defines, uses))
}

/// `mov eax,<symbol>` narrowed to `mov ax,0`: the low word took no relocation
/// with it, so the address was lost.
#[test]
fn a_relocated_dword_immediate_is_not_narrowed_to_a_word_without_its_relocation() {
    let address = Addr { index: 1, ..Addr::new(Space::Segment, 0) };
    let symbol = Loc::Imm(Imm { value: 0, width: 4, address: Some(address) });
    let source = point(1, sem(Operation::Move, "mov", vec![rl(RegId::EAX, 4)], vec![symbol]), vec![1], vec![]);
    let finish = Insn {
        requires: vec![(Held { value: 1, width: 2 }, RegId::AX)],
        reads_complete: true,
        ..insn(2, Some((2, 2)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![1])
    };
    let input = body("symbol", 1, vec![block(1, vec![source, Arc::new(finish)], vec![]).into()]);

    assert_eq!(narrowed_moves(rules(), &input), input);
}

/// `lea cx,[bx+1]; cmp bx,5; mov bx,cx; jb` is `cmp; lea bx,[bx+1]; jb`: the
/// lea sets no flag, so the branch reads the compare's.
#[test]
fn a_step_after_its_compare_leaves_the_flags_the_branch_reads() {
    let (bx, cx) = (rl(RegId::BX, 2), rl(RegId::CX, 2));
    let cell = Loc::Mem(Mem { through: RegId::BX, offset: 1, disp_width: 1, ..Mem::new(None, 2) });
    for (other, rewritten) in [(im(5, 2), true), (cx.clone(), false)] {
        let step = point(1, sem(Operation::Address, "lea", vec![cx.clone()], vec![cell.clone()]), vec![2], vec![1]);
        let compare = point(2, sem(Operation::Compare, "cmp", vec![], vec![bx.clone(), other]), vec![], vec![1]);
        let copy = point(3, sem(Operation::Move, "mov", vec![bx.clone()], vec![cx.clone()]), vec![3], vec![2]);
        let insns = vec![step, compare, copy, jump_if("jb", 4)];
        let finish = Insn {
            reads_complete: true,
            ..insn(4, Some((4, 4)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![])
        };
        let input =
            body("step", 1, vec![block(1, insns, vec![4]).into(), block(4, vec![Arc::new(finish)], vec![]).into()]);

        let made = stepped_after_compare(rules(), &input);

        let expected = if rewritten { vec!["cmp", "lea", "jb", "ret"] } else { vec!["lea", "cmp", "mov", "jb", "ret"] };
        assert_eq!(names_of(&made), expected, "{rewritten}");
        if rewritten {
            assert_eq!(whats(&made.insns())[1].dests, vec![bx.clone()]);
        }
    }
}

/// A carry read in a successor block kept `add` as `inc` would not set it.
#[test]
fn a_unit_add_stays_where_a_successor_reads_the_carry() {
    for (register, width) in [(RegId::CL, 1), (RegId::CX, 2), (RegId::ECX, 4)] {
        let counter = rl(register, width);
        let add = point(
            1,
            sem(Operation::Binary, "add", vec![counter.clone()], vec![counter.clone(), im(1, width)]),
            vec![1],
            vec![1],
        );
        let bx = rl(RegId::BX, 2);
        let carry = point(4, sem(Operation::Binary, "adc", vec![bx.clone()], vec![bx, im(0, 2)]), vec![], vec![]);
        let input = body("carry", 1, vec![block(1, vec![add], vec![4]).into(), block(4, vec![carry], vec![]).into()]);

        assert_eq!(names_of(&increments(rules(), &input))[0], "add", "{width}");
    }
}

/// `mov cx,bx; shl cx,cl; mov bx,cx` read its count from BX's low byte; the
/// renamed `shl bx,cl` reads CX's.
#[test]
fn a_shuttle_does_not_rename_a_register_that_is_the_shift_count() {
    let (bx, cx, cl) = (rl(RegId::BX, 2), rl(RegId::CX, 2), rl(RegId::CL, 1));
    let insns = vec![
        point(1, sem(Operation::Move, "mov", vec![cx.clone()], vec![bx.clone()]), vec![2], vec![1]),
        point(2, sem(Operation::Binary, "shl", vec![cx.clone()], vec![cx.clone(), cl]), vec![3], vec![2]),
        point(3, sem(Operation::Move, "mov", vec![bx], vec![cx]), vec![4], vec![3]),
    ];
    let input = body("count", 1, vec![block(1, insns, vec![]).into()]);

    assert_eq!(shuttles(rules(), &input), input);
}

/// A block and its successor, which compares first: of the flags, only what
/// the first block's branch reads is live between them.
fn flat_blocks(insns: Vec<Arc<Insn>>) -> LirBody {
    body("flat", 0, vec![block(0, insns, vec![1]).into(), block(1, vec![compared()], vec![]).into()])
}

/// The word sum leaves the flags of 16 bits, so a branch after the extension
/// that reads any of them keeps the dword sum.
#[test]
fn a_flat_dword_sum_stays_where_a_flag_is_read_after_its_extension() {
    for reader in ["jb", "jo", "js", "je", "jp"] {
        let input =
            flat_blocks(vec![word_load(RegId::EAX), dword_sum(), extension(RegId::EAX, RegId::AX), jump_if(reader, 1)]);
        let kept = names_of(&narrowed_arithmetic(flat(), &input));
        assert_eq!(kept, ["movzx", "add", "movzx", reader, "cmp"], "{reader}");
    }
}

/// `mov cx,ax` as a dword copy where `ecx`'s upper half is read after it
/// copied AX's zero upper half over the one the program had.
#[test]
fn a_flat_word_copy_stays_where_the_upper_half_of_its_target_is_read() {
    let copy = point(1, sem(Operation::Move, "mov", vec![rl(RegId::CX, 2)], vec![rl(RegId::AX, 2)]), vec![], vec![]);
    let read = point(2, sem(Operation::Compare, "cmp", vec![], vec![rl(RegId::ECX, 4), im(5, 4)]), vec![], vec![]);
    let input = flat_body(vec![word_load(RegId::EAX), copy, read]);

    assert_eq!(widened_moves(flat(), &input), input);
}

/// `shl r,1` read as `add r,r` where a successor reads the carry or overflow
/// it sets: the add defines both as the shift does, at every width.
#[test]
fn a_doubling_add_keeps_the_carry_and_overflow_a_successor_reads() {
    let cpu = peephole().cpu;
    for (register, width) in [(RegId::CL, 1), (RegId::CX, 2), (RegId::ECX, 4)] {
        let counter = rl(register, width);
        let shift = point(
            1,
            sem(Operation::Binary, "shl", vec![counter.clone()], vec![counter.clone(), im(1, 1)]),
            vec![1],
            vec![1],
        );
        let input = body(
            "double",
            1,
            vec![block(1, vec![shift, jump_if("jb", 4)], vec![4]).into(), block(4, vec![], vec![]).into()],
        );

        let made = doubled(rules(), &input, &cpu).unwrap();

        assert_eq!(
            whats(&made.insns())[0],
            sem(Operation::Binary, "add", vec![counter.clone()], vec![counter.clone(), counter]),
            "{width}"
        );
    }
}

/// `add eax,[bp-4]; mov [bp-4],eax`, the spiller's fold of a reload into an
/// accumulating sum (m16 -Os qlight.c: 4 B and a second frame slot for 2
/// instructions), is the read-modify-write `add [bp-4],eax` when eax dies at
/// the store.
fn _accumulated(
    name: &str,
    dies: bool,
) -> (LirBody, Insn) {
    let cell = Loc::Mem(mem(frame(-4), 4, RegId::BP, -4, 1));
    let eax = rl(RegId::EAX, 4);
    let sum = insn(
        1,
        Some((1, 3)),
        Some(sem(Operation::Binary, name, vec![eax.clone()], vec![eax.clone(), cell.clone()])),
        vec![10],
        vec![1],
    );
    let stored =
        insn(3, Some((3, 3)), Some(sem(Operation::Move, "mov", vec![cell], vec![eax.clone()])), vec![], vec![10]);
    let after = insn(
        4,
        Some((4, 5)),
        Some(sem(Operation::Move, "mov", vec![rl(RegId::EBX, 4)], vec![if dies { im(0, 4) } else { eax.clone() }])),
        vec![12],
        vec![],
    );
    let clear =
        insn(5, Some((5, 6)), Some(sem(Operation::Move, "mov", vec![eax.clone()], vec![im(0, 4)])), vec![13], vec![]);
    let insns = if dies {
        vec![Arc::new(sum), Arc::new(stored.clone()), Arc::new(clear)]
    } else {
        vec![Arc::new(sum), Arc::new(stored.clone()), Arc::new(after)]
    };
    (body("sum", 0, vec![block(0, insns, vec![]).into()]), stored)
}

#[test]
fn test_a_sum_stored_back_to_the_cell_it_added_is_a_read_modify_write() {
    for name in ["add", "and", "or", "xor"] {
        let (input, stored) = _accumulated(name, true);
        let result = transferred(rules(), &input).insns();
        let cell = Loc::Mem(mem(frame(-4), 4, RegId::BP, -4, 1));
        assert_eq!(
            result[0].what,
            Some(sem(Operation::Binary, name, vec![cell.clone()], vec![cell, rl(RegId::EAX, 4)])),
            "{name}"
        );
        assert_eq!(result[1].what.as_ref().unwrap().op, Operation::Nothing, "{name}");
        assert_eq!(result[1].uses, stored.uses, "{name}");
    }
}

#[test]
fn test_a_sum_still_read_after_its_store_keeps_its_register_form() {
    let (input, _) = _accumulated("add", false);
    assert_eq!(transferred(rules(), &input).insns(), input.blocks[0].insns.to_vec());
    let (input, _) = _accumulated("sub", true);
    assert_eq!(transferred(rules(), &input).insns(), input.blocks[0].insns.to_vec());
}

/// nib quicksort `_sort` at m16: the swap block's `mov [bp-6],dx` filled a slot
/// the next block overwrites (`mov [bp-6],si`) before any read, yet
/// `unread_spill_stores` kept it because the slot is read elsewhere: one dead
/// operand a swap, 4,643 over the run. Returns the stores to the slot left in
/// the first block; `reads_first`: the next block reads the slot before
/// storing.
fn spill_store_before_an_overwrite(reads_first: bool) -> usize {
    let cell = || Loc::Mem(mem(Some(Addr::new(Space::Frame, -6).in_slot(-6)), 2, RegId::BP, -6, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let ret = insn(200, Some((200, 200)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![]);
    let store = |at: i64, from: RegId| mov(at, vec![cell()], vec![rl(from, 2)]);
    let load = |at: i64| mov(at, vec![rl(RegId::AX, 2)], vec![cell()]);
    let next =
        if reads_first { vec![load(100), store(110, RegId::SI)] } else { vec![store(100, RegId::SI), load(110)] };
    let first = block(0, vec![Arc::new(store(0, RegId::DX))], vec![100]);
    let second = block(100, next.into_iter().chain([ret]).map(Arc::new).collect(), vec![]);
    let done = crate::backend::deadspills::dead_spill_stores(&body("dead-spill", 0, vec![first.into(), second.into()]));
    whats(&done.blocks[0].insns).len()
}

#[test]
fn test_a_spill_store_the_next_block_overwrites_before_a_read_is_dropped() {
    assert_eq!(spill_store_before_an_overwrite(false), 0);
}

#[test]
fn test_a_spill_store_the_next_block_reads_stays() {
    assert_eq!(spill_store_before_an_overwrite(true), 1);
}

/// Instructions `dead_spill_stores` visits on a ladder of `blocks` blocks, each
/// storing the slot and a back edge from the last to the first.
fn visits(blocks: i64) -> usize {
    let spec: Vec<(i64, &str, Vec<i64>)> =
        (0..blocks).map(|at| (at * 100, "SD SS L", vec![if at + 1 < blocks { (at + 1) * 100 } else { 0 }])).collect();
    crate::backend::deadspills::VISITED.with(|visited| visited.set(0));
    stores_left(&spec);
    crate::backend::deadspills::VISITED.with(std::cell::Cell::get)
}

/// Doubling the function must not more than double the work.
#[test]
fn doubling_the_blocks_doubles_the_dead_store_work() {
    let (small, large) = (visits(100), visits(200));
    assert!(small > 300, "{small} visits: the pass looked at nothing");
    assert!(large * 10 <= small * 21, "{small} visits for 100 blocks, {large} for 200");
}

/// The spill stores `dead_spill_stores` leaves in block 0 of a function whose
/// blocks are `(at, instructions, successors)`; `S(from)` stores slot -6, `L`
/// loads it, `A` takes its address.
fn stores_left(blocks: &[(i64, &str, Vec<i64>)]) -> usize {
    let cell = || Loc::Mem(mem(Some(Addr::new(Space::Frame, -6).in_slot(-6)), 2, RegId::BP, -6, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>| Insn {
        spill_store: dests.iter().any(|d| matches!(d, Loc::Mem(_))),
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut at = 0;
    let built: Vec<Block> = blocks
        .iter()
        .map(|(start, text, succ)| {
            let insns = text
                .split_whitespace()
                .map(|word| {
                    at += 10;
                    Arc::new(match word {
                        "SD" => mov(at, vec![cell()], vec![rl(RegId::DX, 2)]),
                        "SS" => mov(at, vec![cell()], vec![rl(RegId::SI, 2)]),
                        "L" => mov(at, vec![rl(RegId::AX, 2)], vec![cell()]),
                        "A" => insn(
                            at,
                            Some((at, at)),
                            Some(sem(Operation::Address, "lea", vec![rl(RegId::BX, 2)], vec![Loc::Address(AddressRef::new(Some(Addr::new(Space::Frame, -6).in_slot(-6))))])),
                            vec![],
                            vec![],
                        ),
                        _ => insn(at, Some((at, at)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![]),
                    })
                })
                .collect::<Vec<_>>();
            Block::from(block(*start, insns, Vec::clone(succ)))
        })
        .collect();
    let done = crate::backend::deadspills::dead_spill_stores(&body("dead-spill", 0, built));
    done.blocks
        .iter()
        .flat_map(|block| whats(&block.insns))
        .filter(|what| what.name.as_deref() == Some("mov") && matches!(what.dests.as_slice(), [Loc::Mem(_)]))
        .count()
}

#[test]
fn test_a_spill_store_a_loop_reads_on_the_next_trip_stays() {
    // The store ends the loop body; the read is at its top, reached by the back edge.
    assert_eq!(stores_left(&[(0, "", vec![100]), (100, "L SD", vec![100, 200]), (200, "ret", vec![])]), 1);
}

#[test]
fn test_a_spill_store_overwritten_before_the_loop_reads_it_is_dropped() {
    assert_eq!(stores_left(&[(0, "SD", vec![100]), (100, "SS L", vec![100, 200]), (200, "ret", vec![])]), 1);
}

#[test]
fn test_two_stores_to_a_shared_slot_keep_the_one_a_reload_reads() {
    assert_eq!(stores_left(&[(0, "SD SS L ret", vec![])]), 1);
}

#[test]
fn test_a_slot_whose_address_escapes_keeps_every_store() {
    assert_eq!(stores_left(&[(0, "SD A SS L ret", vec![])]), 2);
}

#[test]
fn test_a_store_read_on_one_of_two_paths_stays() {
    // 0 -> {100 reads, 200 overwrites}: the read path needs it.
    assert_eq!(
        stores_left(&[(0, "SD", vec![100, 200]), (100, "L ret", vec![]), (200, "SS L ret", vec![])]),
        2
    );
}

/// The movs left of `mov [bp-14],dx ; <between> ; mov dx,[bp-14]` (a spill and
/// its reload) after forwarding, when `between` writes `place` and the slot is
/// named by an address when `taken`. 3: the reload stays.
fn reload_across(
    place: Loc,
    taken: bool,
) -> usize {
    let slot = || Addr::new(Space::Frame, -14).in_slot(-14);
    let cell = || Loc::Mem(mem(Some(slot()), 2, RegId::BP, -14, 1));
    let mov = |at: i64, dests: Vec<Loc>, sources: Vec<Loc>, spill_store: bool| Insn {
        spill_store,
        ..insn(at, Some((at, at + 2)), Some(sem(Operation::Move, "mov", dests, sources)), vec![], vec![])
    };
    let mut insns = vec![Arc::new(mov(0, vec![cell()], vec![rl(RegId::DX, 2)], true))];
    if taken {
        insns.push(Arc::new(insn(
            5,
            Some((5, 5)),
            Some(sem(Operation::Address, "lea", vec![rl(RegId::BX, 2)], vec![Loc::Address(AddressRef::new(Some(slot())))])),
            vec![],
            vec![],
        )));
    }
    insns.push(Arc::new(mov(10, vec![place], vec![rl(RegId::CX, 2)], false)));
    insns.push(Arc::new(mov(100, vec![rl(RegId::DX, 2)], vec![cell()], false)));
    insns.push(Arc::new(insn(200, Some((200, 200)), Some(sem(Operation::Return, "ret", vec![], vec![])), vec![], vec![])));
    let done = forwarded_reloads(rules(), &body("forwarded-reload", 0, vec![block(0, insns, vec![]).into()]));
    whats(&done.blocks[0].insns).iter().filter(|what| what.name.as_deref() == Some("mov")).count()
}

#[test]
fn test_a_store_to_the_spill_slot_stops_the_forwarding() {
    let slot = Addr::new(Space::Frame, -14).in_slot(-14);
    assert_eq!(reload_across(Loc::Mem(mem(Some(slot), 2, RegId::BP, -14, 1)), false), 3);
}

#[test]
fn test_a_byte_store_into_the_spill_slot_stops_the_forwarding() {
    let byte = Addr::new(Space::Frame, -13);
    assert_eq!(reload_across(Loc::Mem(mem(Some(byte), 1, RegId::BP, -13, 1)), false), 3);
}

#[test]
fn test_a_frame_array_store_indexed_by_a_register_does_not_reach_a_spill_slot() {
    let base = Addr::new(Space::Frame, -20);
    let indexed = Mem { index_through: RegId::SI, scale: 1, ..mem(Some(base), 2, RegId::BP, -20, 1) };
    assert_eq!(reload_across(Loc::Mem(indexed), false), 2);
}

#[test]
fn test_a_pointer_store_stops_the_forwarding_where_the_slot_address_is_taken() {
    assert_eq!(reload_across(Loc::Mem(mem(None, 2, RegId::DI, 0, 0)), true), 3);
}
