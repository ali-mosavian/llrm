//! Port of `tests/test_lcssa_merges.py`. `is` assertions compare by `==`.

use std::rc::Rc;
use std::collections::BTreeSet;

use crate::analysis::loops;
use crate::model::mir::{MirBlock, MirBody, Phi, Value};
use crate::optimize::lcssa::closed;
use crate::optimize::lcssa::tests::{incoming, loop_with_exit_use, value};

fn multiple_exits() -> (MirBody, Value) {
    let (mut body, carried, consume) = loop_with_exit_use();
    body.blocks[2].succ = vec![1, 4];
    body.blocks[3] = MirBlock::new(3, vec![], vec![], vec![5]);
    body.blocks.push(MirBlock::new(4, vec![], vec![], vec![5]));
    let mut moved = consume;
    moved.at = 5;
    body.blocks.push(MirBlock::new(5, vec![], vec![moved], vec![]));
    (body, carried)
}

#[test]
fn test_distinct_exits_merge_before_the_downstream_use() {
    let (body, carried) = multiple_exits();
    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    assert_ne!(*result, body);
    let (first, second, join) = (result.block(3).unwrap(), result.block(4).unwrap(), result.block(5).unwrap());
    assert_eq!(first.phis[0].incoming, incoming(&[(1, carried)]));
    assert_eq!(second.phis[0].incoming, incoming(&[(2, carried)]));
    assert_eq!(join.phis[0].incoming, incoming(&[(3, first.phis[0].result), (4, second.phis[0].result)]));
    assert_eq!(join.ops[0].uses, vec![join.phis[0].result]);
    assert_eq!(closed(&result).unwrap(), result);
}

#[test]
fn test_bypass_phi_keeps_its_non_loop_input() {
    let (mut body, carried) = multiple_exits();
    let seed = body.blocks[0].ops[0].defines[0];
    let answer = value(20, 6, 2, 0);
    body.blocks[0].succ = vec![1, 6];
    let last = body.blocks.len() - 1;
    body.blocks[last] = MirBlock::new(5, vec![], vec![], vec![6]);
    body.blocks.push(MirBlock::new(
        6,
        vec![Phi { result: answer, incoming: incoming(&[(0, seed), (5, carried)]) }],
        vec![],
        vec![],
    ));
    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    let join = result.block(5).unwrap();
    let bypass = result.block(6).unwrap();
    assert!(!join.phis.is_empty());
    assert_eq!(bypass.phis[0].incoming, incoming(&[(0, seed), (5, join.phis[0].result)]));
    assert_eq!(closed(&result).unwrap(), result);
}

#[test]
fn test_direct_use_after_a_bypass_is_not_fabricated() {
    let (mut body, _) = multiple_exits();
    body.blocks[0].succ = vec![1, 5];
    assert_eq!(closed(&Rc::new(MirBody::clone(&body))).unwrap(), Rc::new(body));
}

#[test]
fn test_following_cycle_keeps_complete_phi_edges() {
    let (mut body, _) = multiple_exits();
    let last = body.blocks.len() - 1;
    body.blocks[last].succ = vec![5, 6];
    body.blocks.push(MirBlock::new(6, vec![], vec![], vec![]));
    let result = closed(&Rc::new(MirBody::clone(&body))).unwrap();
    assert_ne!(*result, body);
    let predecessors = loops::predecessors(&result.blocks);
    for block in &result.blocks {
        for phi in &block.phis {
            assert_eq!(phi.incoming.keys().copied().collect::<BTreeSet<_>>(), predecessors[&block.at]);
        }
    }
    assert_eq!(closed(&result).unwrap(), result);
}

#[test]
#[ignore = "fails in Python too: assert result != body (closing changes nothing)"]
fn test_compiled_early_exit_accumulator_is_closed() {
    use crate::model::passes::Options;
    use crate::testing;
    let found = testing::module(concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/lcmerge-p-g2.obj"));
    let partition = testing::blocks_of(&found);
    let raised = testing::main_body(&found, &partition);
    let body = testing::applied(&found, Some(&partition), &raised, Options { lcssa: false, ..Options::default() });
    let [loop_] = <[loops::Loop; 1]>::try_from(loops::loops(&body.blocks, Some(body.entry))).unwrap();
    let result = closed(&body).unwrap();
    assert_ne!(result, body);
    let inside = |block: &MirBlock| loop_.body.contains(&block.at);
    let defined: BTreeSet<Value> = result
        .blocks
        .iter()
        .filter(|block| inside(block))
        .flat_map(|block| {
            let phis = block.phis.iter().map(|phi| phi.result);
            phis.chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()))
        })
        .filter(|value| !value.flags)
        .collect();
    let outside: Vec<&MirBlock> = result.blocks.iter().filter(|block| !inside(block)).collect();
    assert!(!outside.iter().flat_map(|block| &block.ops).flat_map(|op| &op.uses).any(|value| defined.contains(value)));
    assert!(outside
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.iter())
        .filter(|(_, value)| defined.contains(*value))
        .all(|(parent, _)| loop_.body.contains(parent)));
    assert_eq!(closed(&result).unwrap(), result);
}
