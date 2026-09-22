//! Port of `tests/test_loopsimplify.py`. `is` assertions compare by `==`.
//! Skipped, needing the corpus and `transform.applied`:
//! test_adjacent_angle_loops_reach_a_fixed_point,
//! test_real_timer_loop_has_one_backedge.

use std::rc::Rc;
use crate::model::mir::MirBody;
use std::collections::BTreeSet;

use super::{grouped, simplified};
use crate::analysis::loops;
use crate::model::mir::{Kind, Phi};
use crate::optimize::lcssa::tests::{held, incoming, loop_with_exit_use, operation, value};

#[test]
fn test_grouping_preserves_each_phi_edge_value() {
    let (body, carried, _) = loop_with_exit_use();
    let entry = body.blocks[0].clone();
    let seed = entry.ops[0].defines[0];
    let other_seed = value(20, 4, seed.variable, 20);
    // Two distinct entries must retain their own source value at the preheader.
    let mut other = entry.clone();
    other.at = 4;
    other.ops[0].at = 4;
    other.ops[0].defines = vec![other_seed];
    other.ops[0].results = vec![held(other_seed, 2)];
    let mut header = body.blocks[1].clone();
    let stepped = *header.phis[0].incoming.get(&2).unwrap();
    header.phis = vec![Phi { result: carried, incoming: incoming(&[(0, seed), (4, other_seed), (2, stepped)]) }];
    let mut blocks = vec![entry, header];
    blocks.extend(body.blocks[2..].iter().cloned());
    blocks.push(other);
    let body = crate::model::mir::MirBody { blocks, ..body };
    let result = grouped(&Rc::new(MirBody::clone(&body)), 1, &BTreeSet::from([0, 4]));
    let bridge = result.blocks.last().unwrap();
    assert_eq!(bridge.phis[0].incoming, incoming(&[(0, seed), (4, other_seed)]));
    let header = result.block(1).unwrap();
    let latch = result.block(2).unwrap();
    assert_eq!(*header.phis[0].incoming.get(&bridge.at).unwrap(), bridge.phis[0].result);
    assert_eq!(latch.succ, vec![1]);
}

#[test]
fn test_unsupported_group_is_atomic() {
    for hazard in ["entry", "missing-source", "opaque", "bad-phi"] {
        let (mut body, _, _) = loop_with_exit_use();
        let (mut target, mut sources) = (1, BTreeSet::from([0]));
        if hazard == "entry" {
            target = body.entry;
        } else if hazard == "missing-source" {
            sources = BTreeSet::from([999]);
        } else if hazard == "opaque" {
            body.blocks[0].ops[0].kind = Kind::Opaque;
        } else {
            body.blocks[1].phis[0].incoming = incoming(&[]);
        }
        assert_eq!(grouped(&Rc::new(MirBody::clone(&body)), target, &sources), Rc::new(body), "{hazard}");
    }
}

#[test]
fn test_conditional_entry_and_shared_exit_become_dedicated() {
    for unsupported_exit in [false, true] {
        let (body, _, _) = loop_with_exit_use();
        let (mut entry, mut header, latch, exit_block) =
            (body.blocks[0].clone(), body.blocks[1].clone(), body.blocks[2].clone(), body.blocks[3].clone());
        entry.succ = vec![header.at, exit_block.at];
        let mut branch = operation(10, Kind::Branch, &[], &[], vec![], vec![]);
        branch.target = Some(exit_block.at);
        entry.ops.push(branch);
        let mut test = operation(11, Kind::Branch, &[], &[], vec![], vec![]);
        test.target = Some(exit_block.at);
        header.ops = vec![test];
        if unsupported_exit {
            header.ops[0].kind = Kind::Opaque;
        }
        let header_at = header.at;
        let exit_at = exit_block.at;
        let body = crate::model::mir::MirBody { blocks: vec![entry, header, latch, exit_block], ..body };
        let result = simplified(&Rc::new(MirBody::clone(&body)));
        if unsupported_exit {
            assert_eq!(result, Rc::new(body));
            continue;
        }
        let found = loops::loops(&result.blocks, Some(result.entry));
        assert_eq!(found.len(), 1);
        let loop_ = &found[0];
        let predecessors = loops::predecessors(&result.blocks);
        let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
        assert_eq!(outside.len(), 1);
        let preheader = result.block(outside[0]).unwrap();
        assert_eq!(preheader.succ, vec![header_at]);
        let exits = result
            .blocks
            .iter()
            .filter(|block| loop_.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !loop_.body.contains(at))
            .collect::<BTreeSet<_>>();
        assert_ne!(exits, BTreeSet::from([exit_at]));
        assert!(exits.iter().all(|at| predecessors[at].is_subset(&loop_.body)));
        assert_eq!(simplified(&result), result);
    }
}
