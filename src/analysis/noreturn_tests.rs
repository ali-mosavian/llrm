//! Ports of the `noreturn` tests in `tests/test_noreturn.py` and
//! `tests/test_sccp.py`.

use std::rc::Rc;
use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::{after_terminal_calls, inferred};
use crate::model::ir::Operation;
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode};

fn op(at: i64, operation: Operation, name: &str, kind: Kind) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), name, vec![], vec![]);
    made.kind = kind;
    made
}

fn sealed(entry: i64, blocks: Vec<MirBlock>) -> MirBody {
    let mut body = MirBody::new(entry, blocks);
    body.sealed = true;
    body
}

#[test]
fn test_closed_local_terminal_scc_is_noreturn() {
    let call_b = op(2, Operation::Nothing, "", Kind::Call);
    let call_a = op(4, Operation::Nothing, "", Kind::Call);
    let first = sealed(0x30, vec![MirBlock::new(0x30, vec![], vec![call_b], vec![])]);
    let second = sealed(0x40, vec![MirBlock::new(0x40, vec![], vec![call_a], vec![])]);

    assert_eq!(
        inferred(
            &IndexMap::from_iter([(0x30, first), (0x40, second)]),
            &IndexMap::from_iter([(2, 0x40), (4, 0x30)]),
            &BTreeSet::new(),
        ),
        BTreeSet::from([0x30, 0x40])
    );
}

#[test]
fn test_terminal_call_inerts_newly_unreachable_successor() {
    let terminal = op(2, Operation::Nothing, "", Kind::Call);
    let store = op(10, Operation::Move, "mov", Kind::Store);
    let body = sealed(
        0,
        vec![
            MirBlock::new(0, vec![], vec![terminal], vec![10]),
            MirBlock::new(10, vec![], vec![store], vec![]),
        ],
    );

    let trimmed = after_terminal_calls(&Rc::new(MirBody::clone(&body)), &BTreeSet::from([2]));

    assert!(trimmed.block(0).unwrap().succ.is_empty());
    let orphan = trimmed.block(10).unwrap();
    assert!(orphan.succ.is_empty());
    assert!(orphan.ops.iter().all(|op| op.kind == Kind::Nothing && op.stores.is_empty()));
}

#[test]
fn test_terminal_call_inerts_its_same_block_source_tail() {
    let terminal = op(2, Operation::Nothing, "", Kind::Call);
    let mut dead_jump = op(3, Operation::Branch, "jmp", Kind::Branch);
    dead_jump.target = Some(20);
    dead_jump.absorbed = vec![3];
    let body = sealed(
        0,
        vec![
            MirBlock::new(0, vec![], vec![terminal, dead_jump], vec![20]),
            MirBlock::new(20, vec![], vec![], vec![]),
        ],
    );

    let trimmed = after_terminal_calls(&Rc::new(MirBody::clone(&body)), &BTreeSet::from([2]));

    let owner = trimmed.block(0).unwrap().ops.last().unwrap();
    assert_eq!(owner.at, 3);
    assert_eq!(owner.absorbed, vec![3]);
    assert_eq!(owner.kind, Kind::Nothing);
    assert_eq!(owner.target, None);
    assert!(trimmed.block(0).unwrap().succ.is_empty());
}

/// MAIN crashed reserving two spill bytes: HOST_SHUTDOWN ends via B$CEND,
/// but the frame pass recognized only direct runtime termination calls.
#[test]
fn test_qrender_main_spill_uses_shutdown_control_proof() {
    use crate::abi::runtime::{self, Control, Reg};
    use crate::backend::{allocate, frame, lower, prologue};
    use crate::model::ir::Loc;
    use crate::model::mir;
    use crate::objectfile::omf;
    use crate::support::testing;

    for terminal in [true, false] {
        let path = "fixtures/regressions/qrender-main-v-g3.obj";
        let found = testing::loaded(path).unwrap();
        // These two external BASIC procedures enter B$ENRA before reading flags.
        let inputs = BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di]);
        let external: IndexMap<String, runtime::Contract> = ["MOD_TEX_DUMP", "SB_DUMP"]
            .into_iter()
            .map(|name| (name.to_owned(), runtime::Contract { inputs: Some(inputs.clone()), ..runtime::worst(name) }))
            .collect();
        let mut contracts = runtime::for_module(&found, Some(&external)).unwrap();
        let raised = testing::raised_from(&found, &testing::partitioned(path), Some(&mut contracts));
        let bodies: IndexMap<i64, MirBody> =
            raised.values.iter().map(|(_, body)| (body.entry, MirBody::clone(body))).collect();
        let symbols: IndexMap<String, i64> =
            omf::pubdef_names(&found.records, found.seg).unwrap().into_iter().map(|(at, name)| (name, at)).collect();
        let local: IndexMap<i64, i64> =
            found.calls.iter().filter_map(|(at, name)| symbols.get(name).map(|target| (*at, *target))).collect();
        let exits: BTreeSet<i64> = contracts
            .iter()
            .filter(|(at, contract)| contract.established && contract.control == Control::Never && (terminal || **at != 0x17F7))
            .map(|(at, _)| *at)
            .collect();
        let proven = inferred(&bodies, &local, &exits);
        assert_eq!(proven.contains(&0x30), terminal);
        assert_eq!(proven.contains(&symbols["HOST_SHUTDOWN"]), terminal);
        // Has END arms and a returning arm.
        assert!(!proven.contains(&symbols["HOST_INIT"]));

        let handler = Rc::new(bodies[&symbols["HOST_SHUTDOWN"]].clone());
        let mut terminal_sites = exits.clone();
        terminal_sites.extend(local.iter().filter(|(_, target)| proven.contains(target)).map(|(at, _)| *at));
        // The object path used the summary only for its prologue: after B$CEND
        // at 0x17f7 it still lowered a dead call and return in this same block.
        let last = |body: &MirBody| -> Vec<Op> {
            let ops = &body.block(0x17BF).unwrap().ops;
            ops[ops.len() - 3..].to_vec()
        };
        assert_eq!(last(&handler).iter().map(|op| op.at).collect::<Vec<_>>(), [0x17F7, 0x17FC, 0x1801]);
        let trimmed = after_terminal_calls(&handler, &terminal_sites);
        if terminal {
            assert_eq!(last(&trimmed).iter().map(|op| op.at).collect::<Vec<_>>(), [0x17F7, 0x17FC, 0x1801]);
            assert_eq!(last(&trimmed).iter().map(|op| op.kind).collect::<Vec<_>>(), [Kind::Call, Kind::Nothing, Kind::Nothing]);
            assert!(trimmed.block(0x17BF).unwrap().succ.is_empty());
        } else {
            assert!(Rc::ptr_eq(&trimmed, &handler));
        }
        assert!(mir::verify(&trimmed).is_empty());

        let body = lower::lowered(
            "main",
            &bodies[&0x30],
            Some(&found.calls),
            found.absorbed.keys().copied().collect(),
            Some(&contracts),
            "386",
            lower::Lowered {
                noreturn: proven.contains(&0x30),
                nodes: raised.source.nodes.clone(),
                sites: found.absorbed.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let empty = crate::model::lir::LirBody { blocks: vec![], ..body.clone() };
        let assignment = allocate::Assignment {
            r#where: IndexMap::default(),
            spilled: BTreeSet::new(),
            cost: 0.0,
            optimal: true,
            why: String::new(),
        };
        assert_eq!(allocate::applied(&empty, &assignment).unwrap().noreturn, terminal);
        let mut slots = frame::Frame::new(0);
        slots.slot(23, 2).unwrap();
        let result = prologue::reserved(&body, &slots, Some(&found.calls));
        if !terminal {
            assert!(result.is_err());
            continue;
        }
        let result = result.unwrap();
        let insns = result.insns();
        let first = insns[0].what.as_ref().unwrap();
        assert_eq!(first.name.as_deref(), Some("sub"));
        let Some(Loc::Imm(size)) = first.sources.last() else { panic!("{first:?}") };
        assert_eq!(size.value, 2);
        assert_eq!(insns.iter().filter(|one| one.frame_adjust).count(), 1);
        assert!(insns.iter().any(|one| one.at == 0x10D));
    }
}
