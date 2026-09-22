//! Port of `tests/test_loopexit.py`.
//!
//! Skipped: every test builds its body from an OMF fixture through modules
//! not yet ported (`wholeseg.emitted`, `corpus.loaded`, `mir.bodies`,
//! `transform.applied`):
//! - test_accumulation_has_no_backedge
//! - test_loop_exit_requires_a_complete_proof
//! - test_accumulation_exit_wraps_at_its_own_width
//! - test_index_sum_uses_the_exact_triangular_coefficient
//! - test_a_doubled_accumulator_is_not_a_linear_sum
//! - test_addrm_long_sum_is_computed_outside_the_store_loop
//! - test_partial_exit_rewrite_preserves_observations
//! - test_exit_evaluation_crosses_lcssa_boundary
//! - test_closed_exit_does_not_hide_an_observed_recurrence
//! - test_exit_evaluation_rewrites_downstream_phi_edges
//!
//! In their place, one hand-built `i < 4; s += i` loop, with every expected
//! listing taken from running Python `loopexit.evaluated` on the same body.

use crate::analysis::{consts, loops};
use crate::model::mir::{
    Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OrderedMap, Phi, Value,
};

use super::evaluated;

fn value(id: u32, at: i64, variable: u32) -> Value {
    Value {
        variable,
        ..Value::new(id, at)
    }
}

fn held(value: Value) -> Arg {
    Arg::Held(Held { value, width: 2 })
}

fn op(
    at: i64,
    kind: Kind,
    defines: Vec<Value>,
    uses: Vec<Value>,
    args: Vec<Arg>,
    results: Vec<Arg>,
) -> Op {
    let mut op = Op::new(at, None, "", defines, uses);
    op.kind = kind;
    op.args = args;
    op.results = results;
    op
}

/// `stored`: nothing, the accumulator's update ("s"), or the counter's ("i").
fn build(stored: Option<&str>) -> MirBody {
    let start = value(924, 0, 5);
    let counter = value(925, 1, 5);
    let following = value(926, 2, 5);
    let flags = Value {
        flags: true,
        ..Value::new(927, 1)
    };
    let s0 = value(940, 0, 6);
    let s = value(941, 1, 6);
    let s1 = value(942, 2, 6);
    let zero = || Arg::Const(Const::new(0, 2));
    let init = op(
        0,
        Kind::Copy,
        vec![start],
        vec![],
        vec![zero()],
        vec![held(start)],
    );
    let init_s = op(
        0,
        Kind::Copy,
        vec![s0],
        vec![],
        vec![zero()],
        vec![held(s0)],
    );
    let compare = op(
        1,
        Kind::Sub,
        vec![flags],
        vec![counter],
        vec![held(counter), Arg::Const(Const::new(4, 2))],
        vec![],
    );
    let mut branch = op(1, Kind::Branch, vec![], vec![flags], vec![], vec![]);
    branch.test = Some(Kind::AboveEq);
    branch.target = Some(3);
    let add = op(
        2,
        Kind::Add,
        vec![s1],
        vec![s, counter],
        vec![held(s), held(counter)],
        vec![held(s1)],
    );
    let increment = op(
        2,
        Kind::Increment,
        vec![following],
        vec![counter],
        vec![held(counter)],
        vec![held(following)],
    );
    let mut latch = vec![add, increment];
    if let Some(which) = stored {
        let target = if which == "s" { s1 } else { following };
        let reference = MemRef::new(None, 2);
        let mut store = op(
            2,
            Kind::Store,
            vec![],
            vec![target],
            vec![held(target)],
            vec![Arg::Cell(Cell {
                r#ref: reference.clone(),
            })],
        );
        store.stores = vec![reference];
        latch.push(store);
    }
    let ret = op(3, Kind::Return, vec![], vec![s], vec![held(s)], vec![]);
    let phi = |result: Value, from_entry: Value, from_latch: Value| {
        let mut incoming = OrderedMap::new();
        incoming.insert(0, from_entry);
        incoming.insert(2, from_latch);
        Phi { result, incoming }
    };
    MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![init, init_s], vec![1]),
            MirBlock::new(
                1,
                vec![phi(counter, start, following), phi(s, s0, s1)],
                vec![compare, branch],
                vec![2, 3],
            ),
            MirBlock::new(2, vec![], latch, vec![1]),
            MirBlock::new(3, vec![], vec![ret], vec![]),
        ],
    )
}

fn listing(body: &MirBody) -> Vec<String> {
    let names = |values: &[Value]| {
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut out = Vec::new();
    for block in &body.blocks {
        let phis = block.phis.iter().map(|phi| phi.result).collect::<Vec<_>>();
        out.push(format!("{} [{}] {:?}", block.at, names(&phis), block.succ));
        for op in &block.ops {
            out.push(format!(
                "  {} ({}) ({}) {:?}",
                op.kind.name(),
                names(&op.defines),
                names(&op.uses),
                op.target
            ));
        }
    }
    out
}

fn known(body: &MirBody, id: u32) -> Option<(i64, u32)> {
    consts::known(body)
        .iter()
        .find(|(value, _)| value.id == id)
        .map(|(_, fact)| (i64::try_from(&fact.n).expect("small"), fact.width))
}

#[test]
fn disposable_loop_becomes_its_exit_values() {
    let body = build(None);
    let result = evaluated(&body).expect("evaluated");
    assert!(loops::loops(&result.blocks, Some(result.entry)).is_empty());
    assert_eq!(
        listing(&result),
        [
            "0 [] [1]",
            "  COPY (v924) () None",
            "  COPY (v940) () None",
            "1 [] [3]",
            "  ADD (v943) (v924) None",
            "  ADD (v944) (v943) None",
            "  COPY (v925) (v944) None",
            "  ADD (v945) (v940) None",
            "  MUL (v946) (v924) None",
            "  ADD (v947) (v945,v946) None",
            "  ADD (v948) (v947) None",
            "  COPY (v941) (v948) None",
            "  NOTHING () () None",
            "  JUMP () () Some(3)",
            "2 [] []",
            "  NOTHING () () None",
            "  NOTHING () () None",
            "3 [] []",
            "  RETURN () (v941) None",
        ]
    );
    assert_eq!(known(&result, 925), Some((4, 2)));
    assert_eq!(known(&result, 941), Some((6, 2)));
}

#[test]
fn accumulator_read_by_a_store_is_kept() {
    let body = build(Some("s"));
    assert_eq!(evaluated(&body).expect("evaluated"), body);
}

#[test]
fn constant_exit_replaces_an_unobserved_accumulator() {
    let body = build(Some("i"));
    let result = evaluated(&body).expect("evaluated");
    assert_eq!(loops::loops(&result.blocks, Some(result.entry)).len(), 1);
    assert_eq!(
        listing(&result)[listing(&result).len() - 3..],
        [
            "3 [] []",
            "  COPY (v943) () None",
            "  RETURN () (v943) None"
        ]
    );
    assert_eq!(known(&result, 943), Some((6, 2)));
}
