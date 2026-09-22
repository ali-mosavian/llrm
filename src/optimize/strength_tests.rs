//! Ports of the Python tests that call `strength` directly.
//!
//! tests/test_cpu_profile.py:
//!   test_formula_selection_prices_complete_sibling_groups
//!   test_formula_selection_recomputes_a_cheap_scaled_index_under_pressure
//!   test_formula_selection_prices_a_complete_affine_formula_under_pressure
//!   test_formula_selection_uses_67h_before_spilling_or_recomputing
//! tests/test_induction_identity.py:
//!   test_zero_extended_counter_product_is_carried_as_a_wide_recurrence
//!   test_composed_offset_can_carry_an_invariant_pointer
//!   test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable
//!   test_reduction_preserves_every_live_product_result
//!   test_reduction_does_not_speculate_on_a_loop_bypass
//!   test_reduced_product_keeps_the_current_iteration_on_exit
//!   test_inserted_counter_operations_own_their_insertion_location
//!
//! Skipped, they need modules with no Rust port (the pipeline in
//! `transform.applied`, the frontends, `wholeseg`, `Strength`):
//!   test_flow.py: test_strength_reduction_replaces_a_loop_multiply_with_an_add
//!   test_ranges.py, test_ivshare.py: monkeypatch `strength` inside the pipeline
//!   test_cpu_profile.py: the two `transform.applied(..., only="strength")` tests
//!   test_induction_identity.py: the tests patching `strength.reduced` in the
//!     pipeline, test_strength_does_not_spill_cheap_loop_work, and the
//!     matrix/harr `wholeseg` fixtures

use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use super::{_DEFAULT_COSTS, _answer, _formula_set, _secondary_indexes, reduced};
use crate::analysis::induction::{self, Affine, AffineOperand, Derived};
use crate::analysis::loops::Loop;
use crate::analysis::occurrence::{OpOccurrence, operations};
use crate::model::ir::Operation;
use crate::model::mir::{
    Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value,
};
use crate::model::passes::{AddressForm, OperationCosts};
use crate::objectfile::module::{Addr, Space};

fn value(id: u32, at: i64, variable: u32) -> Value {
    Value {
        variable,
        ..Value::new(id, at)
    }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: i64, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

fn affine_const(n: i64, width: u32) -> AffineOperand {
    AffineOperand::Const(Const::new(n, width))
}

/// Python's `mir.Op(at, operation, name, defines, uses, kind=, args=, results=)`.
fn op(
    at: i64,
    operation: Operation,
    name: &str,
    defines: Vec<Value>,
    uses: Vec<Value>,
    kind: Kind,
    args: Vec<Arg>,
    results: Vec<Arg>,
) -> Op {
    let mut op = Op::new(at, Some(OpCode::Operation(operation)), name, defines, uses);
    op.kind = kind;
    op.args = args;
    op.results = results;
    op
}

fn phi(result: Value, incoming: &[(i64, Value)]) -> Phi {
    Phi {
        result,
        incoming: incoming.iter().copied().collect::<OrderedMap<_, _>>(),
    }
}

/// A body holding `ops` in one block, for formulas that name them.
fn holding(ops: Vec<Op>) -> (MirBody, Vec<OpOccurrence>) {
    let body = MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]);
    let occurrences = operations(&body).map(|(at, _, _)| at).collect();
    (body, occurrences)
}

fn occurrence(body: &MirBody, block: usize, index: usize) -> OpOccurrence {
    operations(body)
        .find(|(at, _, _)| at.block_index() == block && at.operation_index() == index)
        .map(|(at, _, _)| at)
        .expect("present")
}

fn reduced_default(body: &MirBody, registers: i64) -> MirBody {
    reduced(
        body,
        None,
        registers,
        &BTreeSet::new(),
        0,
        &_DEFAULT_COSTS,
        &[],
        true,
    )
    .expect("reduces")
}

/// `tests/test_induction_identity.py:body`.
fn body() -> (MirBody, Loop) {
    let start = value(10, 0, 7);
    let counter = value(11, 1, 7);
    let following = value(12, 1, 7);
    let unrelated = value(13, 1, 7);
    let answer = value(14, 1, 8);
    let step = op(
        1,
        Operation::Unary,
        "",
        vec![following],
        vec![counter],
        Kind::Increment,
        vec![held(counter, 2)],
        vec![held(following, 2)],
    );
    let multiply = op(
        2,
        Operation::Binary,
        "",
        vec![answer],
        vec![unrelated],
        Kind::Mul,
        vec![held(unrelated, 2), constant(2, 2)],
        vec![held(answer, 2)],
    );
    let blocks = vec![
        MirBlock::new(0, vec![], vec![], vec![1]),
        MirBlock::new(
            1,
            vec![phi(counter, &[(0, start), (1, following)])],
            vec![step, multiply],
            vec![1, 2],
        ),
        MirBlock::new(2, vec![], vec![], vec![]),
    ];
    (
        MirBody::new(0, blocks),
        Loop {
            header: 1,
            latches: BTreeSet::from([1]),
            body: BTreeSet::from([1]),
        },
    )
}

#[test]
fn test_formula_selection_prices_complete_sibling_groups() {
    let group = |start: u32,
                 count: u32,
                 ops: &mut Vec<Op>|
     -> Vec<(usize, Affine, Arg, Vec<(Arg, BigInt)>)> {
        let counter = Value::new(start, 0);
        let product = Value::new(start + 1, 1);
        let affine = Affine {
            value: counter.id,
            start: affine_const(0, 2),
            step: affine_const(1, 2),
            header: 1,
        };
        ops.push(op(
            1,
            Operation::Multiply,
            "imul",
            vec![product],
            vec![counter],
            Kind::Mul,
            vec![held(counter, 2), constant(8, 2)],
            vec![held(product, 2)],
        ));
        let mut out = vec![(ops.len() - 1, affine.clone(), constant(8, 2), vec![])];
        for index in 0..count {
            let answer = Value::new(start + 2 + index, 2 + i64::from(index));
            ops.push(op(
                2 + i64::from(index),
                Operation::Binary,
                "add",
                vec![answer],
                vec![product],
                Kind::Add,
                vec![held(product, 2), constant(i64::from(index) * 16, 2)],
                vec![held(answer, 2)],
            ));
            out.push((
                ops.len() - 1,
                affine.clone(),
                constant(8, 2),
                vec![(constant(i64::from(index) * 16, 2), BigInt::from(1))],
            ));
        }
        out
    };
    let mut ops = Vec::new();
    let small = group(10, 2, &mut ops);
    let large = group(100, 3, &mut ops);
    let (built, at) = holding(ops);
    let derived = |one: &(usize, Affine, Arg, Vec<(Arg, BigInt)>)| Derived {
        op: at[one.0],
        of: one.1.clone(),
        by: one.2.clone(),
        offsets: one.3.clone(),
        pointer: None,
    };
    let small = small.iter().map(derived).collect::<Vec<_>>();
    let large = large.iter().map(derived).collect::<Vec<_>>();
    let candidates = [small.clone(), large.clone()].concat();
    let costly_addresses = OperationCosts {
        add: 1,
        address: 100,
        load: 1,
        memory_update: 1,
        ..OperationCosts::default()
    };

    let selected = _formula_set(
        &built,
        &candidates,
        Some(4),
        &BTreeSet::new(),
        &BTreeSet::new(),
        &costly_addresses,
        None,
    );

    assert_eq!(
        selected,
        [vec![small[0].clone()], large[1..].to_vec()].concat()
    );
}

#[test]
fn test_formula_selection_recomputes_a_cheap_scaled_index_under_pressure() {
    let counter = Value::new(10, 0);
    let answer = Value::new(11, 1);
    let affine = Affine {
        value: counter.id,
        start: affine_const(0, 2),
        step: affine_const(1, 2),
        header: 1,
    };
    let formula = |by: Arg| {
        let multiply = op(
            1,
            Operation::Multiply,
            "imul",
            vec![answer],
            vec![counter],
            Kind::Mul,
            vec![held(counter, 2), by.clone()],
            vec![held(answer, 2)],
        );
        let (built, at) = holding(vec![multiply]);
        let one = Derived {
            op: at[0],
            of: affine.clone(),
            by,
            offsets: vec![],
            pointer: None,
        };
        (built, one)
    };
    let costs = OperationCosts {
        add: 2,
        multiply: 22,
        shift: 3,
        load: 4,
        memory_update: 8,
        ..OperationCosts::default()
    };
    let references = BTreeMap::from([(answer.id, 1)]);
    let empty = BTreeSet::new();

    let (built, one) = formula(constant(2, 2));
    assert!(
        _formula_set(
            &built,
            &[one],
            Some(0),
            &empty,
            &empty,
            &costs,
            Some(&references)
        )
        .is_empty()
    );
    let (built, one) = formula(held(Value::new(12, 0), 2));
    assert!(
        !_formula_set(
            &built,
            &[one],
            Some(0),
            &empty,
            &empty,
            &costs,
            Some(&references)
        )
        .is_empty()
    );
}

#[test]
fn test_formula_selection_prices_a_complete_affine_formula_under_pressure() {
    let counter = Value::new(10, 0);
    let answer = Value::new(11, 1);
    let seed = Value::new(12, 0);
    let affine = Affine {
        value: counter.id,
        start: affine_const(0, 4),
        step: affine_const(1, 4),
        header: 1,
    };
    let leaf = op(
        1,
        Operation::Binary,
        "add",
        vec![answer],
        vec![counter, seed],
        Kind::Add,
        vec![held(counter, 4), held(seed, 4)],
        vec![held(answer, 4)],
    );
    let (built, at) = holding(vec![leaf]);
    let complete = Derived {
        op: at[0],
        of: affine,
        by: constant(24, 4),
        offsets: vec![
            (constant(-128, 4), BigInt::from(1)),
            (held(seed, 4), BigInt::from(1)),
        ],
        pointer: None,
    };
    let costs = OperationCosts {
        add: 2,
        multiply: 22,
        shift: 3,
        address: 2,
        load: 4,
        memory_update: 8,
        ..OperationCosts::default()
    };
    let empty = BTreeSet::new();

    assert_eq!(
        _formula_set(
            &built,
            &[complete.clone()],
            Some(0),
            &empty,
            &empty,
            &costs,
            Some(&BTreeMap::from([(answer.id, 1)])),
        ),
        vec![complete]
    );
}

#[test]
fn test_formula_selection_uses_67h_before_spilling_or_recomputing() {
    let counter = Value::new(10, 0);
    let answer = Value::new(11, 1);
    let affine = Affine {
        value: counter.id,
        start: affine_const(0, 2),
        step: affine_const(1, 2),
        header: 1,
    };
    let multiply = op(
        1,
        Operation::Multiply,
        "imul",
        vec![answer],
        vec![counter],
        Kind::Mul,
        vec![held(counter, 2), constant(2, 2)],
        vec![held(answer, 2)],
    );
    let (built, at) = holding(vec![multiply]);
    let formula = Derived {
        op: at[0],
        of: affine,
        by: constant(2, 2),
        offsets: vec![],
        pointer: None,
    };
    let secondary = AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, 1, 4, true, None).unwrap();

    let activated = _secondary_indexes(
        &built,
        &[formula],
        0,
        &BTreeSet::new(),
        &BTreeMap::from([(at[0], (2, secondary))]),
        Some(&BTreeMap::from([(answer.id, 1)])),
    );

    assert_eq!(activated, BTreeSet::from([at[0]]));
}

#[test]
fn test_zero_extended_counter_product_is_carried_as_a_wide_recurrence() {
    let start = value(1, 0, 1);
    let counter = value(2, 1, 1);
    let following = value(3, 2, 1);
    let flags = Value {
        flags: true,
        ..Value::new(4, 1)
    };
    let extended = value(5, 2, 2);
    let product = value(6, 2, 3);

    let define = op(
        0,
        Operation::Move,
        "",
        vec![start],
        vec![],
        Kind::Copy,
        vec![constant(0, 2)],
        vec![held(start, 2)],
    );
    let compare = op(
        1,
        Operation::Compare,
        "",
        vec![flags],
        vec![counter],
        Kind::Sub,
        vec![held(counter, 2), constant(64, 2)],
        vec![],
    );
    let mut branch = op(
        1,
        Operation::Branch,
        "",
        vec![],
        vec![flags],
        Kind::Branch,
        vec![],
        vec![],
    );
    branch.test = Some(Kind::AboveEq);
    branch.target = Some(3);
    let widen = op(
        2,
        Operation::Extend,
        "",
        vec![extended],
        vec![counter],
        Kind::ZeroExtend,
        vec![held(counter, 2)],
        vec![held(extended, 4)],
    );
    let multiply = op(
        2,
        Operation::Multiply,
        "",
        vec![product],
        vec![extended],
        Kind::Mul,
        vec![held(extended, 4), constant(109, 4)],
        vec![held(product, 4)],
    );
    let consume = op(
        2,
        Operation::Push,
        "",
        vec![],
        vec![product],
        Kind::Arg,
        vec![held(product, 4)],
        vec![],
    );
    let step = op(
        2,
        Operation::Binary,
        "",
        vec![following],
        vec![counter],
        Kind::Add,
        vec![held(counter, 2), constant(1, 2)],
        vec![held(following, 2)],
    );
    let mut jump = op(
        2,
        Operation::Jump,
        "",
        vec![],
        vec![],
        Kind::Jump,
        vec![],
        vec![],
    );
    jump.target = Some(1);
    let built = MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![define], vec![1]),
            MirBlock::new(
                1,
                vec![phi(counter, &[(0, start), (2, following)])],
                vec![compare, branch],
                vec![2, 3],
            ),
            MirBlock::new(
                2,
                vec![],
                vec![widen, multiply, consume, step, jump],
                vec![1],
            ),
            MirBlock::new(3, vec![], vec![], vec![]),
        ],
    );

    let result = reduced_default(&built, 6);
    let latch = result.blocks.iter().find(|block| block.at == 2).unwrap();
    assert!(!latch.ops.iter().any(|op| op.kind == Kind::Mul));
    assert!(
        latch
            .ops
            .iter()
            .any(|op| op.kind == Kind::Add && op.args.contains(&constant(109, 4)))
    );
}

#[test]
fn test_composed_offset_can_carry_an_invariant_pointer() {
    let (built, loop_) = body();
    let header = built.blocks[1].clone();
    let counter = header.phis[0].result;
    let following = header.ops[0].defines[0];
    let mut step = header.ops[0].clone();
    step.args = vec![held(counter, 4)];
    step.results = vec![held(following, 4)];
    let offset = header.ops[1].defines[0];
    let mut multiply = header.ops[1].clone();
    multiply.uses = vec![counter];
    multiply.args = vec![held(counter, 4), constant(2, 4)];
    multiply.results = vec![held(offset, 4)];
    let base = value(40, 0, 40);
    let pointer = value(41, 1, 41);
    let displaced = value(42, 1, 42);
    let add = op(
        3,
        Operation::Binary,
        "",
        vec![displaced],
        vec![offset],
        Kind::Add,
        vec![held(offset, 4), constant(6, 4)],
        vec![held(displaced, 4)],
    );
    let address = op(
        4,
        Operation::Binary,
        "",
        vec![pointer],
        vec![base, displaced],
        Kind::PtrOffset,
        vec![held(base, 4), held(displaced, 4)],
        vec![held(pointer, 4)],
    );
    let phi_ = phi(
        counter,
        &[
            (0, *header.phis[0].incoming.get(&0).unwrap()),
            (1, following),
        ],
    );
    let mut new_header = header.clone();
    new_header.phis = vec![phi_];
    new_header.ops = vec![step, multiply, add, address];
    let built = MirBody::new(
        0,
        vec![
            built.blocks[0].clone(),
            new_header.clone(),
            built.blocks[2].clone(),
        ],
    );
    let derived = induction::derived(&built, &loop_, None, None).unwrap();
    let carried = derived
        .iter()
        .find(|one| one.op == occurrence(&built, 1, 3))
        .unwrap();
    assert_eq!(carried.pointer, Some(held(base, 4)));
    assert_eq!(carried.of.value, counter.id);
    assert_eq!(carried.by, constant(2, 4));
    assert_eq!(carried.offsets, vec![(constant(6, 4), BigInt::from(1))]);
    let cell = MemRef {
        base: Some(pointer),
        base_width: 4,
        pointer: true,
        ..MemRef::new(None, 2)
    };
    let mut store = op(
        5,
        Operation::Move,
        "",
        vec![],
        vec![pointer],
        Kind::Store,
        vec![constant(1, 2)],
        vec![],
    );
    store.stores = vec![cell];
    let mut exit = built.blocks[2].clone();
    exit.ops = vec![store];
    let result = reduced_default(
        &MirBody::new(0, vec![built.blocks[0].clone(), new_header, exit]),
        0,
    );
    let carried_store = result
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.stores)
        .find(|reference| reference.pointer)
        .unwrap();
    assert!(carried_store.base.is_some());
    assert_ne!(carried_store.base.unwrap().variable, pointer.variable);
}

#[test]
fn test_a_reduced_counter_has_its_own_loop_phi_and_fresh_variable() {
    for address in [false, true] {
        let (built, _loop) = body();
        let header = built.blocks[1].clone();
        let counter = header.phis[0].result;
        let answer = header.ops[1].defines[0];
        let mut multiply = header.ops[1].clone();
        multiply.uses = vec![counter];
        multiply.args = vec![held(counter, 2), constant(2, 2)];
        let livein = value(99, 0, 77);
        let mut use_ = op(
            4,
            Operation::Push,
            "",
            vec![],
            vec![answer, livein],
            Kind::Arg,
            vec![held(answer, 2), held(livein, 2)],
            vec![],
        );
        if address {
            let memory = MemRef {
                base: Some(answer),
                ..MemRef::new(
                    Some(Addr {
                        index: 1,
                        ..Addr::new(Space::Segment, 0x20)
                    }),
                    2,
                )
            };
            use_.args = vec![
                Arg::Cell(Cell {
                    r#ref: memory.clone(),
                }),
                held(livein, 2),
            ];
            use_.loads = vec![memory];
        }
        let mut new_header = header.clone();
        new_header.ops = vec![header.ops[0].clone(), multiply, use_];
        let built = MirBody::new(
            0,
            vec![built.blocks[0].clone(), new_header, built.blocks[2].clone()],
        );
        let result = reduced_default(&built, 0);
        let after = &result.blocks[1];
        let added = after
            .phis
            .iter()
            .filter(|phi| phi.result != counter)
            .collect::<Vec<_>>();
        assert_eq!(added.len(), 1);
        let phi = added[0];
        assert!(phi.result.variable > livein.variable);
        let consumer = after.ops.iter().find(|op| op.kind == Kind::Arg).unwrap();

        let source = |value: Value| -> Value {
            let definition = after.ops.iter().find(|op| op.defines.contains(&value));
            match definition {
                Some(definition) if definition.kind == Kind::Copy => match &definition.args[0] {
                    Arg::Held(held) => held.value,
                    _ => panic!("a copy of a value"),
                },
                _ => value,
            }
        };

        if address {
            let Arg::Cell(cell) = &consumer.args[0] else {
                panic!("a cell");
            };
            assert_eq!(source(cell.r#ref.base.unwrap()), phi.result);
            assert_eq!(source(consumer.loads[0].base.unwrap()), phi.result);
        } else {
            let Arg::Held(held) = &consumer.args[0] else {
                panic!("a value");
            };
            assert_eq!(source(held.value), phi.result);
        }
        assert_ne!(phi.incoming.get(&0), phi.incoming.get(&1));
        let incoming = *phi.incoming.get(&1).unwrap();
        let step = after
            .ops
            .iter()
            .find(|op| op.defines.contains(&incoming))
            .unwrap();
        assert!(matches!(&step.args[0], Arg::Held(held) if held.value == phi.result));
    }
}

#[test]
fn test_reduction_preserves_every_live_product_result() {
    for use_ in ["low", "high", "both_through_phis"] {
        let (built, _loop) = body();
        let header = built.blocks[1].clone();
        let low = header.ops[1].defines[0];
        let high = value(30, 1, 9);
        let middle = value(31, 2, 9);
        let final_ = value(32, 3, 9);
        let mut product = header.ops[1].clone();
        product.defines = vec![low, high];
        product.results = vec![held(low, 2), held(high, 2)];
        let values = match use_ {
            "low" => vec![low],
            "high" => vec![high],
            _ => vec![low, final_],
        };
        let consume = op(
            3,
            Operation::Push,
            "",
            vec![],
            values.clone(),
            Kind::Arg,
            values.iter().map(|one| held(*one, 2)).collect(),
            vec![],
        );
        let mut new_header = header.clone();
        new_header.ops = vec![header.ops[0].clone(), product];
        let built = MirBody::new(
            0,
            vec![
                built.blocks[0].clone(),
                new_header,
                MirBlock::new(2, vec![phi(middle, &[(1, high)])], vec![], vec![3]),
                MirBlock::new(3, vec![phi(final_, &[(2, middle)])], vec![consume], vec![]),
            ],
        );
        assert_eq!(
            _answer(&built, occurrence(&built, 1, 1)),
            if use_ == "low" { Some(low) } else { None }
        );
    }
}

#[test]
fn test_reduction_does_not_speculate_on_a_loop_bypass() {
    for bypass in [false, true] {
        let (built, _loop) = body();
        let header = built.blocks[1].clone();
        let counter = header.phis[0].result;
        let answer = header.ops[1].defines[0];
        let memory = MemRef::new(
            Some(Addr {
                index: 1,
                ..Addr::new(Space::Segment, 0x20)
            }),
            2,
        );
        let mut product = header.ops[1].clone();
        product.uses = vec![counter];
        product.args = vec![
            held(counter, 2),
            Arg::Cell(Cell {
                r#ref: memory.clone(),
            }),
        ];
        product.loads = vec![memory];
        let consume = op(
            4,
            Operation::Push,
            "",
            vec![],
            vec![answer],
            Kind::Arg,
            vec![held(answer, 2)],
            vec![],
        );
        let mut entry = built.blocks[0].clone();
        entry.succ = if bypass { vec![1, 2] } else { vec![1] };
        let mut new_header = header.clone();
        new_header.ops = vec![header.ops[0].clone(), product, consume];
        let built = MirBody::new(0, vec![entry, new_header, built.blocks[2].clone()]);
        assert_eq!(reduced_default(&built, 0) == built, bypass);
    }
}

#[test]
fn test_reduced_product_keeps_the_current_iteration_on_exit() {
    let (built, _loop) = body();
    let header = built.blocks[1].clone();
    let counter = header.phis[0].result;
    let mut product = header.ops[1].clone();
    product.uses = vec![counter];
    product.args = vec![held(counter, 2), constant(3, 2)];
    let answer = product.defines[0];
    let exit_value = value(40, 2, 10);
    let consume = op(
        5,
        Operation::Push,
        "",
        vec![],
        vec![exit_value],
        Kind::Arg,
        vec![held(exit_value, 2)],
        vec![],
    );
    let mut new_header = header.clone();
    new_header.ops = vec![header.ops[0].clone(), product];
    let built = MirBody::new(
        0,
        vec![
            built.blocks[0].clone(),
            new_header,
            MirBlock::new(
                2,
                vec![phi(exit_value, &[(1, answer)])],
                vec![consume],
                vec![],
            ),
        ],
    );
    let result = reduced_default(&built, 0);
    let after = &result.blocks[1];
    let phi = after.phis.iter().find(|one| one.result != counter).unwrap();
    let preserved = after.ops.iter().find(|op| op.defines.contains(&answer));
    let preserved = preserved.expect("preserved");
    assert_eq!(preserved.kind, Kind::Copy);
    assert_eq!(preserved.args, vec![held(phi.result, 2)]);
    assert_eq!(*result.blocks[2].phis[0].incoming.get(&1).unwrap(), answer);
    assert!(
        matches!(&preserved.args[0], Arg::Held(held) if Some(&held.value) != phi.incoming.get(&1))
    );
}

#[test]
fn test_inserted_counter_operations_own_their_insertion_location() {
    let (built, _loop) = body();
    let header = built.blocks[1].clone();
    let counter = header.phis[0].result;
    let mut product = header.ops[1].clone();
    product.uses = vec![counter];
    product.args = vec![held(counter, 2), constant(3, 2)];
    let answer = product.defines[0];
    let consume = op(
        4,
        Operation::Push,
        "",
        vec![],
        vec![answer],
        Kind::Arg,
        vec![held(answer, 2)],
        vec![],
    );
    let mut new_header = header.clone();
    new_header.ops = vec![header.ops[0].clone(), product, consume];
    let built = MirBody::new(
        0,
        vec![built.blocks[0].clone(), new_header, built.blocks[2].clone()],
    );
    let result = reduced_default(&built, 0);
    let setup = &result.blocks[0].ops[0];
    let update = result.blocks[1].ops.last().unwrap();
    assert!(setup.at == 0 && setup.inserted());
    assert!(update.at == 4 && update.inserted());
}
