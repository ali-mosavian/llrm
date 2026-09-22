//! Ports of the `interprocedural` tests in `tests/test_sccp.py`.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::*;
use crate::abi::runtime;
use crate::model::ir::Operation;
use crate::model::mir::{Cell, MirBlock};
use crate::objectfile::module::Addr;

fn value(id: u32, at: i64, variable: u32) -> Value {
    Value { id, at, flags: false, variable, version: 1 }
}

fn op(at: i64, operation: Operation, kind: Kind) -> Op {
    let mut made = Op::new(at, OpCode::Operation(operation), "", vec![], vec![]);
    made.kind = kind;
    made
}

fn copy(at: i64, result: Value, number: i64, width: u32) -> Op {
    let mut made = op(at, Operation::Nothing, Kind::Copy);
    made.defines = vec![result];
    made.args = vec![Arg::Const(Const::new(number, width))];
    made.results = vec![Arg::Held(Held { value: result, width })];
    made
}

fn returning(at: i64, returned: Value) -> Op {
    let mut made = op(at, Operation::Nothing, Kind::Return);
    made.uses = vec![returned];
    made.args = vec![Arg::Held(Held { value: returned, width: 2 })];
    made
}

fn argument(at: i64, arg: Arg) -> Op {
    let mut made = op(at, Operation::Nothing, Kind::Arg);
    made.args = vec![arg];
    made
}

fn sealed(entry: i64, blocks: Vec<MirBlock>) -> MirBody {
    let mut body = MirBody::new(entry, blocks);
    body.sealed = true;
    body
}

fn frame_parameter() -> MemRef {
    let mut parameter = MemRef::new(Some(Addr { index: 1, ..Addr::new(Space::Frame, 0) }), 2);
    parameter.space = Some(Space::Frame);
    parameter
}

fn global() -> MemRef {
    let mut reference = MemRef::new(Some(Addr { index: 1, ..Addr::new(Space::Segment, 0) }), 2);
    reference.space = Some(Space::Segment);
    reference
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

fn calls(items: &[(i64, &str)]) -> IndexMap<i64, String> {
    items.iter().map(|(at, name)| (*at, (*name).to_owned())).collect()
}

fn _returned(number: i64, at: i64) -> MirBody {
    let returned = value(at as u32, at, at as u32);
    sealed(at, vec![MirBlock::new(at, vec![], vec![copy(at, returned, number, 2), returning(at + 1, returned)], vec![])])
}

#[test]
fn test_module_constant_returns_require_every_exit_to_agree() {
    let agrees = _returned(37, 1);
    let mut left = agrees.blocks[0].clone();
    left.succ = vec![10];
    let right = _returned(38, 10).blocks[0].clone();
    let mut disagrees = agrees.clone();
    disagrees.blocks = vec![left, right];
    assert_eq!(
        constant_returns(&IndexMap::from_iter([("yes".to_owned(), agrees), ("no".to_owned(), disagrees)])),
        IndexMap::from_iter([("yes".to_owned(), vec![Const::new(37, 2)])])
    );
}

#[test]
fn test_parameter_specialization_requires_every_call_to_agree() {
    let (seven, nine) = (Const::new(7, 2), Const::new(9, 2));
    let (a_calls, b_calls) = (calls(&[(1, "leaf")]), calls(&[(2, "leaf")]));
    let a_constants = IndexMap::from_iter([(1, vec![Some(seven.clone())])]);
    let b_constants = IndexMap::from_iter([(2, vec![Some(nine)])]);
    let procedures = IndexMap::from_iter([
        ("a".to_owned(), (&a_calls, &a_constants)),
        ("b".to_owned(), (&b_calls, &b_constants)),
    ]);
    assert_eq!(constant_parameters(&procedures, &names(&["leaf"])), Parameters::default());
    let b_constants = IndexMap::from_iter([(2, vec![Some(seven.clone())])]);
    let procedures = IndexMap::from_iter([
        ("a".to_owned(), (&a_calls, &a_constants)),
        ("b".to_owned(), (&b_calls, &b_constants)),
    ]);
    assert_eq!(
        constant_parameters(&procedures, &names(&["leaf"])),
        IndexMap::from_iter([("leaf".to_owned(), vec![Some(seven)])])
    );
}

#[test]
fn test_current_parameter_constants_reads_a_sccp_returned_actual() {
    let held = value(1, 1, 1);
    let argument = argument(1, Arg::Held(Held { value: held, width: 2 }));
    let call = op(2, Operation::Nothing, Kind::Call);
    // The copy is deliberately inserted before the argument: it models the
    // result materialized from seed() by interprocedural return propagation.
    let materialized = copy(0, held, 4, 2);
    let body = sealed(1, vec![MirBlock::new(1, vec![], vec![materialized, argument, call], vec![])]);
    assert_eq!(
        current_parameter_constants(
            &IndexMap::from_iter([("caller".to_owned(), body)]),
            &IndexMap::from_iter([("caller".to_owned(), calls(&[(2, "choose")]))]),
            &IndexMap::from_iter([("caller".to_owned(), IndexMap::from_iter([(2, BTreeSet::from([1]))]))]),
            &IndexMap::from_iter([("choose".to_owned(), vec![frame_parameter()])]),
            &names(&["choose"]),
        ),
        IndexMap::from_iter([("choose".to_owned(), vec![Some(Const::new(4, 2))])])
    );
}

#[test]
fn test_current_call_constants_keeps_a_per_call_fact_when_another_call_is_dynamic() {
    let constant = Value { id: 1, at: 0, flags: false, variable: 1, version: 1 };
    let dynamic = Value { id: 2, at: 0, flags: false, variable: 2, version: 1 };
    let known = copy(0, constant, 4, 2);
    let first = argument(1, Arg::Held(Held { value: constant, width: 2 }));
    let first_call = op(2, Operation::Nothing, Kind::Call);
    let second = argument(3, Arg::Held(Held { value: dynamic, width: 2 }));
    let second_call = op(4, Operation::Nothing, Kind::Call);
    let body = sealed(0, vec![MirBlock::new(0, vec![], vec![known, first, first_call, second, second_call], vec![])]);
    let parameters = IndexMap::from_iter([("choose".to_owned(), vec![frame_parameter()])]);
    let calls = calls(&[(2, "choose"), (4, "choose")]);
    let arguments = IndexMap::from_iter([(2, BTreeSet::from([1])), (4, BTreeSet::from([3]))]);
    assert_eq!(
        current_call_constants(&body, &calls, &arguments, &parameters),
        IndexMap::from_iter([(2, vec![Some(Const::new(4, 2))]), (4, vec![None])])
    );
    assert_eq!(
        current_parameter_constants(
            &IndexMap::from_iter([("caller".to_owned(), body)]),
            &IndexMap::from_iter([("caller".to_owned(), calls)]),
            &IndexMap::from_iter([("caller".to_owned(), arguments)]),
            &parameters,
            &names(&["choose"]),
        ),
        Parameters::default()
    );
}

#[test]
fn test_pure_call_removal_drops_its_exact_argument_pushes() {
    let pushed = argument(1, Arg::Const(Const::new(9, 2)));
    let result = value(2, 2, 2);
    let mut call = op(2, Operation::Nothing, Kind::Call);
    call.defines = vec![result];
    call.results = vec![Arg::Held(Held { value: result, width: 2 })];
    let answer = value(3, 2, 3);
    let constant = copy(2, answer, 42, 2);
    let ret = returning(3, answer);
    let body = sealed(1, vec![MirBlock::new(1, vec![], vec![pushed, call, constant, ret], vec![])]);

    let mut contract = runtime::worst("leaf");
    contract.cleanup = Some(0);
    contract.caller_cleanup = 2;

    let sites = argument_sites(&body, &IndexMap::from_iter([(2, contract)]));
    let made = remove_dead_pure_calls(&body, &calls(&[(2, "leaf")]), &names(&["leaf"]), &sites).unwrap();
    assert_eq!(made.blocks[0].ops.iter().map(|op| op.kind).collect::<Vec<_>>(), vec![Kind::Copy, Kind::Return]);
}

#[test]
fn test_purity_refuses_nontermination_and_nonlocal_stores() {
    let looping = sealed(1, vec![MirBlock::new(1, vec![], vec![], vec![1])]);
    let mut store = op(1, Operation::Nothing, Kind::Store);
    store.args = vec![Arg::Const(Const::new(1, 2))];
    store.results = vec![Arg::Cell(Cell { r#ref: global() })];
    store.stores = vec![global()];
    let returned = _returned(1, 1).blocks[0].ops.last().unwrap().clone();
    let writing = sealed(1, vec![MirBlock::new(1, vec![], vec![store, returned], vec![])]);
    let empty = IndexMap::default();
    assert_eq!(
        pure_procedures(&IndexMap::from_iter([
            ("loop".to_owned(), (&looping, &empty)),
            ("write".to_owned(), (&writing, &empty)),
        ])),
        BTreeSet::new()
    );
}

#[test]
fn test_readonly_procedure_allows_only_direct_nonvolatile_static_reads() {
    let held = value(1, 1, 1);
    let mut load = op(1, Operation::Move, Kind::Load);
    load.defines = vec![held];
    load.args = vec![Arg::Cell(Cell { r#ref: global() })];
    load.results = vec![Arg::Held(Held { value: held, width: 2 })];
    load.loads = vec![global()];
    let returned = returning(2, held);
    let body = sealed(1, vec![MirBlock::new(1, vec![], vec![load.clone(), returned.clone()], vec![])]);
    let mut volatile = global();
    volatile.volatile = true;
    let mut volatile_load = load;
    volatile_load.args = vec![Arg::Cell(Cell { r#ref: volatile.clone() })];
    volatile_load.loads = vec![volatile];
    let mut volatile_body = body.clone();
    volatile_body.blocks[0].ops = vec![volatile_load, returned.clone()];
    let mut store = op(1, Operation::Nothing, Kind::Store);
    store.args = vec![Arg::Const(Const::new(1, 2))];
    store.results = vec![Arg::Cell(Cell { r#ref: global() })];
    store.stores = vec![global()];
    let writing = sealed(1, vec![MirBlock::new(1, vec![], vec![store, returned], vec![])]);
    let empty = IndexMap::default();
    assert_eq!(
        readonly_procedures(&IndexMap::from_iter([
            ("read".to_owned(), (&body, &empty)),
            ("volatile".to_owned(), (&volatile_body, &empty)),
            ("write".to_owned(), (&writing, &empty)),
        ])),
        names(&["read"])
    );
}

#[test]
fn test_direct_noreturn_summary_prunes_only_the_callers_impossible_tail() {
    let spin = sealed(1, vec![MirBlock::new(1, vec![], vec![], vec![1])]);
    let call = op(2, Operation::Nothing, Kind::Call);
    let returned = op(3, Operation::Nothing, Kind::Return);
    let caller = sealed(2, vec![MirBlock::new(2, vec![], vec![call.clone(), returned], vec![])]);
    let (spin_calls, caller_calls) = (IndexMap::default(), calls(&[(2, "spin")]));
    let procedures = IndexMap::from_iter([
        ("spin".to_owned(), (&spin, &spin_calls)),
        ("caller".to_owned(), (&caller, &caller_calls)),
    ]);

    assert_eq!(noreturn_procedures(&procedures, &names(&["spin", "caller"])), names(&["spin", "caller"]));
    let pruned = terminal_calls(&caller, &caller_calls, &names(&["spin"]));
    assert_eq!(pruned.blocks[0].ops, vec![call]);
    assert!(pruned.blocks[0].succ.is_empty());
}

#[test]
fn test_noreturn_summary_does_not_make_an_exported_body_a_private_fact() {
    let spin = sealed(1, vec![MirBlock::new(1, vec![], vec![], vec![1])]);
    let empty = IndexMap::default();

    assert_eq!(
        noreturn_procedures(&IndexMap::from_iter([("exported".to_owned(), (&spin, &empty))]), &BTreeSet::new()),
        BTreeSet::new()
    );
}

#[test]
fn test_mutually_recursive_private_terminal_bodies_are_noreturn() {
    let call_b = op(2, Operation::Nothing, Kind::Call);
    let call_a = op(4, Operation::Nothing, Kind::Call);
    let first = sealed(1, vec![MirBlock::new(1, vec![], vec![call_b], vec![])]);
    let second = sealed(3, vec![MirBlock::new(3, vec![], vec![call_a], vec![])]);
    let (first_calls, second_calls) = (calls(&[(2, "second")]), calls(&[(4, "first")]));

    assert_eq!(
        noreturn_procedures(
            &IndexMap::from_iter([
                ("first".to_owned(), (&first, &first_calls)),
                ("second".to_owned(), (&second, &second_calls)),
            ]),
            &names(&["first", "second"]),
        ),
        names(&["first", "second"])
    );
}

#[test]
fn test_noreturn_scc_rejects_a_member_with_a_normal_return() {
    let call_b = op(2, Operation::Nothing, Kind::Call);
    let returned = op(4, Operation::Nothing, Kind::Return);
    let first = sealed(1, vec![MirBlock::new(1, vec![], vec![call_b], vec![])]);
    let second = sealed(3, vec![MirBlock::new(3, vec![], vec![returned], vec![])]);
    let (first_calls, second_calls) = (calls(&[(2, "second")]), IndexMap::default());

    assert_eq!(
        noreturn_procedures(
            &IndexMap::from_iter([
                ("first".to_owned(), (&first, &first_calls)),
                ("second".to_owned(), (&second, &second_calls)),
            ]),
            &names(&["first", "second"]),
        ),
        BTreeSet::new()
    );
}
