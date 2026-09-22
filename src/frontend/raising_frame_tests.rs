//! Port of `tests/test_raising_frame.py`.
//!
//! Skipped, needing `mir.bodies`:
//! `test_pl_move_memory_push_reads_the_pointer_not_its_stack_destination`.
//! Skipped, needing `wholeseg`: `test_chain_constant_divisors_survive_argument_setup`.

use super::*;
use crate::analysis::regions::overlapping;
use crate::frontend::blocks::Ends;
use crate::frontend::declen;
use crate::model::ir::Operation;
use crate::model::mir::{MirBlock, MirBody};

fn load(path: &str) -> Module {
    module::load(path).unwrap().unwrap()
}

fn _block(raw: &str, at: usize, successors: &[usize]) -> Block {
    let digits: String = raw.split_whitespace().collect();
    let mut code = vec![0; at];
    code.extend((0..digits.len()).step_by(2).map(|one| u8::from_str_radix(&digits[one..one + 2], 16).unwrap()));
    let mut insns = Vec::new();
    let mut cursor = at;
    while cursor < code.len() {
        let decoded = declen::decode(&code, cursor).expect("decodes");
        cursor = decoded.end();
        insns.push(decoded);
    }
    Block { at, end: code.len(), insns, ends: Ends::FallsThrough, succ: successors.to_vec() }
}

#[test]
fn test_main_layout_uses_runtime_specific_fixed_prefix() {
    for (tag, floor) in [("q-O", -34), ("p-g2", -42), ("v-g3", -44)] {
        let found = load(&format!("fixtures/omf/chain-{tag}.obj").to_lowercase());
        assert_eq!(_layout(&found), Some((floor, 24)), "{tag}");
    }
}

#[test]
fn test_frame_pointer_values_are_not_private() {
    for raw in ["55", "8bc5", "8bc4", "8d46e6", "8bec"] {
        assert!(!_private(&[_block(raw, 0x30, &[])]), "{raw}");
    }
}

#[test]
fn test_direct_frame_load_does_not_escape_the_frame() {
    assert!(_private(&[_block("8b46e6", 0x30, &[])]));
}

#[test]
fn test_unmodeled_frame_stack_or_selector_change_loses_depth() {
    for raw in ["8bec", "83c402", "8ed0", "cd21", "5c"] {
        let block = _block(&format!("{raw} 50"), 0x30, &[]);
        let depths = _depths(&[block.clone()], block.at, -42, &IndexMap::default());
        assert_eq!(depths[&block.insns.last().unwrap().at], None, "{raw}");
    }
}

#[test]
fn test_known_cleanup_restores_depth_but_unknown_call_does_not() {
    let block = _block("50 9a00000000 50", 0x30, &[]);
    let call = block.insns[1].at as i64;
    let last = block.insns.last().unwrap().at;
    let contract = runtime::contract(Some("B$PSSD"));
    let depth = |routine: &Contract| {
        let contracts: IndexMap<i64, Contract> = [(call, routine.clone())].into_iter().collect();
        _depths(&[block.clone()], block.at, -42, &contracts)[&last]
    };
    assert_eq!(depth(&contract), Some(-42));
    let mut clobbering = contract.clone();
    clobbering.clobbers.insert(Reg::Bp);
    for changed in [
        runtime::worst("unknown"),
        Contract { cleanup: None, ..contract.clone() },
        Contract { enters_user_code: true, ..contract.clone() },
        clobbering,
    ] {
        assert_eq!(depth(&changed), None, "{changed:?}");
    }
}

#[test]
fn test_conflicting_stack_depths_do_not_produce_a_join_fact() {
    let entry = _block("90", 0x30, &[0x40, 0x50]);
    let left = _block("50", 0x40, &[0x60]);
    let right = _block("90", 0x50, &[0x60]);
    let join = _block("50", 0x60, &[]);
    let at = join.at;
    assert_eq!(_depths(&[entry, left, right, join], 0x30, -42, &IndexMap::default())[&at], None);
}

#[test]
fn test_unbalanced_loop_loses_depth_instead_of_iterating_forever() {
    let block = _block("50", 0x30, &[0x30]);
    assert_eq!(_depths(&[block.clone()], block.at, -42, &IndexMap::default())[&block.at], None);
}

#[test]
fn test_event_and_error_modules_keep_their_original_alias_facts() {
    for fixture in ["fixtures/omf/chain-p-evt.obj", "fixtures/omf/divmod-p-g2.obj"] {
        let found = load(fixture);
        let body = RaisedBody::new(MirBody::new(0x30, Vec::new()));
        let contracts = runtime::for_module(&found, None).unwrap();
        assert_eq!(annotated(body.clone(), &found, &[], &contracts), body, "{fixture}");
    }
}

/// PUSH [local] / POP [local] must not claim the explicit local access excludes itself.
#[test]
fn test_push_pop_frame_operand_is_not_its_implicit_stack_access() {
    let found = load("fixtures/omf/chain-p-g2.obj");
    for pushing in [true, false] {
        let block = _block(if pushing { "ff76e6" } else { "50 8f46e6" }, 0x30, &[]);
        let at = block.insns.last().unwrap().at as i64;
        let local = MemRef { space: Some(Space::Stack), ..MemRef::new(Some(Addr::new(Space::Frame, -26)), 2) };
        let stack = MemRef { space: Some(Space::Stack), ..MemRef::new(Some(Addr::new(Space::Stack, -2)), 2) };
        let (loads, stores) =
            if pushing { (vec![local.clone()], vec![stack.clone()]) } else { (vec![stack.clone()], vec![local.clone()]) };
        let mut op = Op::new(
            at,
            crate::model::mir::OpCode::Operation(if pushing { Operation::Push } else { Operation::Pop }),
            "",
            Vec::new(),
            Vec::new(),
        );
        op.kind = if pushing { Kind::Arg } else { Kind::Copy };
        op.args = vec![Arg::Cell(Cell { r#ref: loads[0].clone() })];
        op.results = vec![Arg::Cell(Cell { r#ref: stores[0].clone() })];
        op.loads = loads;
        op.stores = stores;
        let body = RaisedBody::new(MirBody::new(0x30, vec![MirBlock::new(0x30, Vec::new(), vec![op], Vec::new())]));
        let result = annotated(body, &found, &[block], &IndexMap::default()).blocks[0].ops[0].clone();
        let (explicit, implicit) =
            if pushing { (&result.loads[0], &result.stores[0]) } else { (&result.stores[0], &result.loads[0]) };
        assert!(explicit.excludes.is_empty(), "{pushing}");
        assert!(overlapping(explicit, &local, None, None, None).unwrap(), "{pushing}");
        assert!(!implicit.excludes.is_empty(), "{pushing}");
    }
}
