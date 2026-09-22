//! Port of `tests/test_raising_longs.py` and `tests/test_raising_unary.py`.
//!
//! Skipped, needing `wholeseg`:
//! `test_event_arithmetic_keeps_a_pair_when_its_flags_cross_the_machine_exit`,
//! `test_removed_half_flags_do_not_become_machine_exit_inputs`,
//! `test_production_does_not_recognize_long_pairs_after_optimization`,
//! `test_nots_stores_whole_unary_results_without_stack_splitting`,
//! `test_arith_passes_whole_results_without_splitting_them`.
//! Skipped, needing `transform.applied`:
//! `test_addrm_stores_and_reuses_the_signed_whole_value`,
//! `test_nbody_whole_position_loads_leave_the_inner_loop`.
//! Skipped, monkeypatching a `raising_longs` stage out of `mir.bodies`
//! (`scalar`, directly or through the `nbody` fixture; `_negated_whole`,
//! `sign_fills`, `arguments` or `unary`):
//! `test_whole_negation_requires_exact_carry_chain`,
//! `test_signed_store_requires_the_matching_sign_word`,
//! `test_accumulator_initializers_are_whole_values`,
//! `test_constant_stores_require_identical_adjacent_addresses`,
//! `test_position_arithmetic_is_scalar_before_optimization`,
//! `test_nbody_scalar_results_feed_constant_and_accumulator_arithmetic`,
//! `test_nbody_stores_whole_results_through_half_copies`,
//! `test_live_half_flags_prevent_scalar_arithmetic`,
//! `test_same_machine_address_with_different_ssa_base_is_not_a_pair`,
//! `test_argument_join_requires_ordered_adjacent_halves`,
//! `test_long_negation_keeps_observed_intermediate_results`.
//!
//! Meanwhile two synthetic bodies cover `scalar` and `unary`; their expected
//! text is the same bodies run through Python.

use std::sync::Arc;

use iced_x86::Register;

use super::*;
use crate::model::ir::nodes::{Data, Node, TableKind};
use crate::model::ir::{Effects, Imm, Loc, Reg, Semantics};
use crate::model::mir::MirBlock;
use crate::model::mir::MirBody;
use crate::objectfile::module::Addr;
use crate::support::pyrepr::Repr;
use crate::support::testing::{self, all_ops, nth, ops, width};

const AX: Loc = Loc::Reg(Reg { register: Register::AX, width: 2 });
const DX: Loc = Loc::Reg(Reg { register: Register::DX, width: 2 });
/// The pair recognizer reads only register operands.
const MEMORY: Loc = Loc::Imm(Imm { value: 0, width: 2, address: None });

fn cell(disp: i64) -> MemRef {
    MemRef::new(Some(Addr::new(Space::Frame, disp)), 2)
}

fn v(n: u32) -> Value {
    Value::new(n, n as i64)
}

fn f(n: u32) -> Value {
    Value { flags: true, ..v(n) }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

#[allow(clippy::too_many_arguments)]
fn raised(
    at: i64,
    operation: Operation,
    name: &str,
    kind: Kind,
    defines: Vec<Value>,
    uses: Vec<Value>,
    args: Vec<Arg>,
    results: Vec<Arg>,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    loads: Vec<MemRef>,
    stores: Vec<MemRef>,
) -> Op {
    let mut op = Op::new(at, OpCode::Operation(operation), name, defines, uses);
    op.kind = kind;
    op.raised = Some((args.clone(), results.clone()));
    op.args = args;
    op.results = results;
    op.loads = loads;
    op.stores = stores;
    let mut data = Data::new(0, 0, TableKind::Jump, Vec::new(), Effects::no_effect());
    data.semantics = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(operation) };
    mir::raising_occurrence(&op, (at, at + 3), Vec::new(), Some(Arc::new(Node::Data(data))))
}

fn summary(body: &RaisedBody) -> String {
    let refs = |refs: &[MemRef]| {
        let each: Vec<String> = refs
            .iter()
            .map(|r| format!("{}:{}", r.addr.map_or("None".to_owned(), |addr| addr.disp.to_string()), r.width))
            .collect();
        format!("[{}]", each.join(", "))
    };
    let values = |values: &[Value]| format!("[{}]", values.iter().map(Value::repr).collect::<Vec<_>>().join(", "));
    let args = |args: &[Arg]| format!("[{}]", args.iter().map(Arg::repr).collect::<Vec<_>>().join(", "));
    let mut lines = Vec::new();
    for block in &body.blocks {
        for op in &block.ops {
            let merges: Vec<String> =
                op.merges.iter().map(|(source, result)| format!("({}, {})", source.repr(), result.repr())).collect();
            let symbol = match op.symbol {
                None => "None",
                Some(true) => "True",
                Some(false) => "False",
            };
            lines.push(format!(
                "{:#x} {} {} args={} results={} defines={} uses={} loads={} stores={} merges=[{}] ranges={:?} symbol={}",
                op.at,
                op.kind.as_str(),
                op.name,
                args(&op.args),
                args(&op.results),
                values(&op.defines),
                values(&op.uses),
                refs(&op.loads),
                refs(&op.stores),
                merges.join(", "),
                mir::raising_ranges(op),
                symbol,
            ));
        }
    }
    lines.join("\n")
}

fn arithmetic() -> RaisedBody {
    let (a1, d1, a2, d2, f1, f2) = (v(1), v(2), v(3), v(4), f(5), f(6));
    let ops = vec![
        raised(0x00, Operation::Move, "mov", Kind::Load, vec![a1], vec![], vec![Arg::Cell(Cell { r#ref: cell(-4) })],
            vec![held(a1, 2)], vec![AX], vec![MEMORY], vec![cell(-4)], vec![]),
        raised(0x03, Operation::Move, "mov", Kind::Load, vec![d1], vec![], vec![Arg::Cell(Cell { r#ref: cell(-2) })],
            vec![held(d1, 2)], vec![DX], vec![MEMORY], vec![cell(-2)], vec![]),
        raised(0x06, Operation::Binary, "add", Kind::Add, vec![a2, f1], vec![a1],
            vec![held(a1, 2), Arg::Cell(Cell { r#ref: cell(-8) })], vec![held(a2, 2)], vec![AX], vec![AX, MEMORY],
            vec![cell(-8)], vec![]),
        raised(0x09, Operation::Binary, "adc", Kind::AddCarry, vec![d2, f2], vec![d1, f1],
            vec![held(d1, 2), Arg::Cell(Cell { r#ref: cell(-6) })], vec![held(d2, 2)], vec![DX], vec![DX, MEMORY],
            vec![cell(-6)], vec![]),
        raised(0x0C, Operation::Move, "mov", Kind::Store, vec![], vec![a2], vec![held(a2, 2)],
            vec![Arg::Cell(Cell { r#ref: cell(-12) })], vec![MEMORY], vec![AX], vec![], vec![cell(-12)]),
        raised(0x0F, Operation::Move, "mov", Kind::Store, vec![], vec![d2], vec![held(d2, 2)],
            vec![Arg::Cell(Cell { r#ref: cell(-10) })], vec![MEMORY], vec![DX], vec![], vec![cell(-10)]),
        raised(0x12, Operation::Move, "mov", Kind::Store, vec![], vec![], vec![Arg::Const(Const::new(-2, 2))],
            vec![Arg::Cell(Cell { r#ref: cell(-16) })], vec![MEMORY], vec![MEMORY], vec![], vec![cell(-16)]),
        raised(0x15, Operation::Move, "mov", Kind::Store, vec![], vec![], vec![Arg::Const(Const::new(-1, 2))],
            vec![Arg::Cell(Cell { r#ref: cell(-14) })], vec![MEMORY], vec![MEMORY], vec![], vec![cell(-14)]),
    ];
    let mut body = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]));
    body.origin =
        [(a1, Register::EAX), (a2, Register::EAX), (d1, Register::EDX), (d2, Register::EDX)].into_iter().collect();
    body
}

fn negation() -> RaisedBody {
    let (w, a, d, n1, c, n2, f1, f2, f3) = (v(1), v(2), v(3), v(4), v(5), v(6), f(7), f(8), f(9));
    let wide = MemRef { width: 4, ..cell(-4) };
    let mut whole = Op::new(0x00, OpCode::Operation(Operation::Move), "mov", vec![w], vec![]);
    whole.kind = Kind::Load;
    whole.args = vec![Arg::Cell(Cell { r#ref: wide.clone() })];
    whole.results = vec![held(w, 4)];
    whole.loads = vec![wide];
    let extract = |word: Value, offset: i64| {
        let mut op = Op::new(0x00, OpCode::Synth(Synth::HalfToLow), "extract", vec![word], vec![w]);
        op.kind = Kind::Extract;
        op.args = vec![held(w, 4), Arg::Const(Const::new(offset, 4))];
        op.results = vec![held(word, 2)];
        op
    };
    let ops = vec![
        whole,
        extract(a, 0),
        extract(d, 16),
        raised(0x03, Operation::Unary, "neg", Kind::Neg, vec![n1, f1], vec![a], vec![held(a, 2)], vec![held(n1, 2)],
            vec![AX], vec![AX], vec![], vec![]),
        raised(0x06, Operation::Binary, "adc", Kind::AddCarry, vec![c, f2], vec![d, f1],
            vec![held(d, 2), Arg::Const(Const::new(0, 2))], vec![held(c, 2)], vec![DX], vec![DX, MEMORY], vec![],
            vec![]),
        raised(0x09, Operation::Unary, "neg", Kind::Neg, vec![n2, f3], vec![c], vec![held(c, 2)], vec![held(n2, 2)],
            vec![DX], vec![DX], vec![], vec![]),
        raised(0x0C, Operation::Move, "mov", Kind::Store, vec![], vec![n1], vec![held(n1, 2)],
            vec![Arg::Cell(Cell { r#ref: cell(-12) })], vec![MEMORY], vec![AX], vec![], vec![cell(-12)]),
        raised(0x0F, Operation::Move, "mov", Kind::Store, vec![], vec![n2], vec![held(n2, 2)],
            vec![Arg::Cell(Cell { r#ref: cell(-10) })], vec![MEMORY], vec![DX], vec![], vec![cell(-10)]),
    ];
    let mut body = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], ops, vec![])]));
    body.origin = [(a, Register::EAX), (n1, Register::EAX), (d, Register::EDX), (c, Register::EDX), (n2, Register::EDX)]
        .into_iter()
        .collect();
    body
}

fn memref(disp: &str, width: u32) -> String {
    format!(
        "MemRef(addr=[bp-{disp}], width={width}, base=None, segment=None, space=None, beyond=None, symbolic=None, \
         allocation=None, base_width=4, pointer=False, excludes=(), typed=None, within=None, provenance=None, \
         volatile=False, inbounds=False)"
    )
}

#[test]
fn scalar_widens_a_load_an_alu_and_a_store_pair_as_python_does() {
    let (m4, m8, m12, m16) = (memref("0x4", 4), memref("0x8", 4), memref("0xc", 4), memref("0x10", 4));
    let expected = [
        format!("0x0 load mov args=[Cell(ref={m4})] results=[Held(value=v1_1, width=4)] defines=[v1_1] uses=[] loads=[-4:4] stores=[] merges=[] ranges=[(0, 6)] symbol=None"),
        "0x3 extract extract args=[Held(value=v1_1, width=4), Const(n=0, width=4)] results=[Held(value=v1, width=2)] defines=[v1] uses=[v1_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
        "0x3 extract extract args=[Held(value=v1_1, width=4), Const(n=16, width=4)] results=[Held(value=v2, width=2)] defines=[v2] uses=[v1_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
        format!("0x6 load mov args=[Cell(ref={m8})] results=[Held(value=v3_1, width=4)] defines=[v3_1] uses=[] loads=[-8:4] stores=[] merges=[] ranges=[(6, 12)] symbol=True"),
        "0x6 add add args=[Held(value=v1_1, width=4), Held(value=v3_1, width=4)] results=[Held(value=v2_1, width=4)] defines=[v2_1] uses=[v1_1, v3_1] loads=[] stores=[] merges=[] ranges=[] symbol=False".to_owned(),
        "0x9 extract extract args=[Held(value=v2_1, width=4), Const(n=0, width=4)] results=[Held(value=v3, width=2)] defines=[v3] uses=[v2_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
        "0x9 extract extract args=[Held(value=v2_1, width=4), Const(n=16, width=4)] results=[Held(value=v4, width=2)] defines=[v4] uses=[v2_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
        format!("0xc store mov args=[Held(value=v2_1, width=4)] results=[Cell(ref={m12})] defines=[] uses=[v2_1] loads=[] stores=[-12:4] merges=[] ranges=[(12, 18)] symbol=None"),
        format!("0x12 store mov args=[Const(n=4294967294, width=4)] results=[Cell(ref={m16})] defines=[] uses=[] loads=[] stores=[-16:4] merges=[] ranges=[(18, 24)] symbol=None"),
    ]
    .join("\n");
    assert_eq!(summary(&scalar(arithmetic()).unwrap()), expected);
}

#[test]
fn unary_and_scalar_widen_bc_long_negation_as_python_does() {
    let (m4, m12w, m10w, m12) = (memref("0x4", 4), memref("0xc", 2), memref("0xa", 2), memref("0xc", 4));
    let prefix = [
        format!("0x0 load mov args=[Cell(ref={m4})] results=[Held(value=v1, width=4)] defines=[v1] uses=[] loads=[-4:4] stores=[] merges=[] ranges=[] symbol=None"),
        "0x0 extract extract args=[Held(value=v1, width=4), Const(n=0, width=4)] results=[Held(value=v2, width=2)] defines=[v2] uses=[v1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
        "0x0 extract extract args=[Held(value=v1, width=4), Const(n=16, width=4)] results=[Held(value=v3, width=2)] defines=[v3] uses=[v1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
    ];
    let unary_expected = prefix
        .iter()
        .cloned()
        .chain([
            "0x3 neg neg args=[Held(value=v1, width=4)] results=[Held(value=v1_1, width=4)] defines=[v1_1] uses=[v1] loads=[] stores=[] merges=[] ranges=[(3, 12)] symbol=None".to_owned(),
            "0x9 extract extract args=[Held(value=v1_1, width=4), Const(n=0, width=4)] results=[Held(value=v4, width=2)] defines=[v4] uses=[v1_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
            "0x9 extract extract args=[Held(value=v1_1, width=4), Const(n=16, width=4)] results=[Held(value=v6, width=2)] defines=[v6] uses=[v1_1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
            format!("0xc store mov args=[Held(value=v4, width=2)] results=[Cell(ref={m12w})] defines=[] uses=[v4] loads=[] stores=[-12:2] merges=[] ranges=[(12, 15)] symbol=None"),
            format!("0xf store mov args=[Held(value=v6, width=2)] results=[Cell(ref={m10w})] defines=[] uses=[v6] loads=[] stores=[-10:2] merges=[] ranges=[(15, 18)] symbol=None"),
        ])
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(summary(&unary(negation())), unary_expected);

    let scalar_expected = prefix
        .iter()
        .cloned()
        .chain([
            "0x3 neg neg args=[Held(value=v2, width=2)] results=[Held(value=v4, width=2)] defines=[v4, f7] uses=[v2] loads=[] stores=[] merges=[] ranges=[(3, 6)] symbol=None".to_owned(),
            "0x6 addcarry adc args=[Held(value=v3, width=2), Const(n=0, width=2)] results=[Held(value=v5, width=2)] defines=[v5, f8] uses=[v3, f7] loads=[] stores=[] merges=[] ranges=[(6, 9)] symbol=None".to_owned(),
            "0x9 neg neg args=[Held(value=v5, width=2)] results=[Held(value=v6, width=2)] defines=[v6, f9] uses=[v5] loads=[] stores=[] merges=[] ranges=[(9, 12)] symbol=None".to_owned(),
            "0xc neg neg args=[Held(value=v1, width=4)] results=[Held(value=v1_1, width=4)] defines=[v1_1] uses=[v1] loads=[] stores=[] merges=[] ranges=[] symbol=None".to_owned(),
            format!("0xc store mov args=[Held(value=v1_1, width=4)] results=[Cell(ref={m12})] defines=[] uses=[v1_1] loads=[] stores=[-12:4] merges=[] ranges=[(12, 18)] symbol=None"),
        ])
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(summary(&scalar(negation()).unwrap()), scalar_expected);
}

/// UDTACC emitted two-word loads and ADD/ADC for y because its zero-offset access was unnamed.
#[test]
fn test_udtacc_second_field_is_a_whole_long() {
    let body = nth(&testing::raised("fixtures/regressions/udtacc-p-g2.obj"), 0);
    let load = ops(&body).into_iter().find(|op| op.at == 0xB1 && !op.loads.is_empty()).unwrap();
    assert_eq!(load.loads[0].width, 4);
    assert!(!ops(&body).iter().any(|op| op.kind == Kind::AddCarry));
}

/// PITSNAP split its signed byte into two stores and reloaded it, raising NBODY cost by 20.
#[test]
fn test_nbody_timing_helper_stores_the_signed_whole_value() {
    let body = nth(&testing::raised("fixtures/bench/nbody-v-g3.obj"), 1);
    let stores: Vec<Op> =
        ops(&body).into_iter().filter(|op| [0x47B, 0x47E].contains(&op.at) && !op.stores.is_empty()).collect();
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0].stores[0].width, 4);
    assert_eq!(stores[0].stores[0].addr.unwrap().disp, -0x22);
    assert_eq!(width(&stores[0].args[0]), 4);
}

/// NBODY's initial long 1 had an opaque high word, blocking whole-value loop phis.
#[test]
fn test_nbody_counter_seed_has_a_known_high_word() {
    use crate::analysis::consts::{self, Known};
    let path = "fixtures/bench/nbody-v-g3.obj";
    let found = testing::loaded(path).unwrap();
    let raised = nth(&testing::raised(path), 0);
    for (seed, expected) in [(0, 0), (1, 0), (0x7FFF, 0), (0x8000, 0xFFFF), (-1, 0xFFFF)] {
        let mut body = raised.clone();
        for block in &mut body.blocks {
            for op in &mut block.ops {
                if op.at == 0xE0 && op.kind == Kind::Copy {
                    op.args = vec![Arg::Const(Const::new(seed, 2))];
                }
            }
        }
        let body = std::rc::Rc::new(body);
        let facts = consts::known(&body, Some(&found.dgroup.members), Some(&found.calls), None, None);
        let high = ops(&body)
            .into_iter()
            .find(|op| op.at == 0xE3 && !op.results.is_empty() && width(&op.results[0]) == 2)
            .unwrap();
        let Arg::Held(result) = &high.results[0] else { panic!("{:?}", high.results) };
        assert_eq!(facts.get(&result.value), Some(&Known::new(expected, 2)), "{seed}");
    }
}

/// LOCALP kept ADD/ADC halves because sign extension was exposed after pair recognition.
#[test]
fn test_localp_signed_index_addition_is_a_whole_long() {
    for tag in ["q-O", "p-g2", "v-g3"] {
        let ops = all_ops(&testing::raised(format!("fixtures/regressions/localp-{tag}.obj").to_lowercase()));
        assert!(
            ops.iter().any(|op| op.kind == Kind::Add
                && op.results.len() == 1
                && matches!(&op.results[0], Arg::Held(one) if one.width == 4)),
            "{tag}"
        );
        assert!(!ops.iter().any(|op| op.kind == Kind::AddCarry), "{tag}");
    }
}

/// PARITYCONTROL retained split ADD/ADC accumulator updates in both arms: its
/// branch join's unused flag phis must not change the optimizer's path.
#[test]
fn test_control_branch_updates_are_whole_longs_before_optimization() {
    let raised = testing::raised("fixtures/parity/control-v-g3.obj");
    let body = &raised.values.iter().find(|(name, _)| name == "procedure PARITYCONTROL").unwrap().1;
    assert!(!ops(body).iter().any(|op| op.kind == Kind::AddCarry));
    let updates: Vec<Op> =
        ops(body).into_iter().filter(|op| [0x9D, 0xC9].contains(&op.at) && op.kind == Kind::Add).collect();
    assert_eq!(updates.len(), 2);
    assert!(updates.iter().all(|op| width(&op.results[0]) == 4));
}

/// NEGNOT passed split NEG/ADC/NEG chains to PRINT, blocking whole-value folding.
#[test]
fn test_negnot_raises_printed_long_negations() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let body = nth(&testing::raised(format!("fixtures/omf/negnot-{tag}.obj").to_lowercase()), 0);
        let ops = ops(&body);
        assert!(!ops.iter().any(|op| op.kind == Kind::AddCarry), "{tag}");
        assert_eq!(ops.iter().filter(|op| op.kind == Kind::Neg && width(&op.results[0]) == 4).count(), 3, "{tag}");
    }
}
