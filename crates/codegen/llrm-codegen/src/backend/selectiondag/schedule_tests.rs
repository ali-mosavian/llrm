use super::schedule::{SchedInfo, Unit, schedule};
use super::*;

const ADD: u32 = 1;
const LOAD: u32 = 2;
const STORE: u32 = 3;
const MUL: u32 = 4;
const CMP: u32 = 5;
const BR: u32 = 6;
const FLAGS: u16 = 9;

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(matches!(opcode, ADD | LOAD | MUL | CMP))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        if matches!(opcode, ADD | MUL | CMP) { &[FLAGS] } else { &[] }
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }
}

fn dag() -> SelectionDAG {
    SelectionDAG::new(DagConfig::default())
}

fn machine(
    dag: &mut SelectionDAG,
    op: u32,
    values: &[EVT],
    ops: &[SDValue],
) -> SDValue {
    dag.get_node_multi(ISD::Machine(op), values, ops)
}

fn load(
    dag: &mut SelectionDAG,
    reg: u32,
    chain: SDValue,
) -> SDValue {
    let address = dag.get_register(EVT::I32, reg);
    machine(dag, LOAD, &[EVT::I32, EVT::OTHER], &[address, chain])
}

fn chain_of(value: SDValue) -> SDValue {
    SDValue { node: value.node, res: 1 }
}

fn store(
    dag: &mut SelectionDAG,
    value: SDValue,
    reg: u32,
    chain: SDValue,
) -> SDValue {
    let address = dag.get_register(EVT::I32, reg);
    machine(dag, STORE, &[EVT::OTHER], &[value, address, chain])
}

fn position(
    order: &[Unit],
    value: SDValue,
) -> usize {
    order.iter().position(|unit| unit.nodes.contains(&value.node)).expect("scheduled")
}

/// `x = a + b; y = x * c`, both stored in order.
fn example() -> (SelectionDAG, SDValue, [SDValue; 7]) {
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = load(&mut dag, 1, entry);
    let b = load(&mut dag, 2, chain_of(a));
    let c = load(&mut dag, 3, chain_of(b));
    let x = machine(&mut dag, ADD, &[EVT::I32], &[a, b]);
    let y = machine(&mut dag, MUL, &[EVT::I32], &[x, c]);
    let s1 = store(&mut dag, x, 4, chain_of(c));
    let s2 = store(&mut dag, y, 5, s1);
    (dag, s2, [a, b, c, x, y, s1, s2])
}

/// A schedule that put a user above its operand, or a store above the store
/// that precedes it on the chain, would emit wrong code.
#[test]
fn every_unit_follows_its_data_and_chain_operands() {
    let (dag, root, [a, b, c, x, y, s1, s2]) = example();
    let order = schedule(&dag, root, &Info);
    assert_eq!(order.len(), 7, "{order:?}");
    for (earlier, later) in [(a, x), (b, x), (x, y), (c, y), (x, s1), (y, s2), (a, b), (b, c), (c, s1), (s1, s2)] {
        assert!(position(&order, earlier) < position(&order, later), "{earlier:?} before {later:?} in {order:?}");
    }
}

/// A compare glued to its branch is one unit, branch first: the flags it sets
/// are read by the instruction right after it.
#[test]
fn glued_nodes_are_one_unit() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = load(&mut dag, 1, entry);
    let b = load(&mut dag, 2, chain_of(a));
    let cmp = machine(&mut dag, CMP, &[EVT::I32, EVT::GLUE], &[a, b]);
    let glue = SDValue { node: cmp.node, res: 1 };
    let branch = machine(&mut dag, BR, &[EVT::OTHER], &[chain_of(b), glue]);
    let order = schedule(&dag, branch, &Info);
    let unit = order.iter().find(|unit| unit.nodes.contains(&cmp.node)).expect("scheduled");
    assert_eq!(unit.nodes, [branch.node, cmp.node]);
    assert_eq!(order.len(), 3, "{order:?}");
    assert_eq!(order.last(), Some(unit));
}

/// Nodes the builder left in the arena (here a dead add) are not scheduled.
#[test]
fn nodes_not_reachable_from_the_root_are_not_scheduled() {
    let (mut dag, root, [a, b, ..]) = example();
    let dead = machine(&mut dag, ADD, &[EVT::I32], &[b, a]);
    let order = schedule(&dag, root, &Info);
    assert!(order.iter().all(|unit| !unit.nodes.contains(&dead.node)), "{order:?}");
}

/// Two runs give one order (the queue breaks ties by arrival, not by address).
#[test]
fn the_schedule_is_deterministic() {
    let (dag, root, _) = example();
    assert_eq!(schedule(&dag, root, &Info), schedule(&dag, root, &Info));
}

const ADC: u32 = 7;
const TEST: u32 = 8;
const CMOV: u32 = 9;

struct FlagsInfo(u16);

impl SchedInfo for FlagsInfo {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(matches!(opcode, ADD | ADC | CMOV | LOAD))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        if matches!(opcode, ADD | ADC | TEST) { std::slice::from_ref(&self.0) } else { &[] }
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }

    fn has_cross_copy_class(
        &self,
        reg: u16,
    ) -> bool {
        reg == self.0
    }
}

/// An `add`'s carry is read by an `adc` while another unit that also needs the
/// flags (`test; cmov`) uses the `add`'s sum: scheduling the `adc` first left
/// every candidate waiting for EFLAGS, and the first held unit went anyway, a
/// `test` between the `add` and the `adc` (x_ll_arith's four carry saves). The
/// pin unschedules back to the `adc` and puts the `cmov` below it
/// (`BacktrackBottomUp`).
#[test]
fn a_unit_waiting_for_flags_backtracks_to_the_reader_of_the_live_ones() {
    let eflags_id = super::dagisel_adapter::EFLAGS_ID;
    let eflags = super::dagisel_adapter::pin_register(eflags_id).expect("EFLAGS has a pin number");
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = load(&mut dag, 1, entry);
    let b = load(&mut dag, 2, chain_of(a));
    let c = load(&mut dag, 3, chain_of(b));
    let d = load(&mut dag, 4, chain_of(c));
    let add = machine(&mut dag, ADD, &[EVT::I32, EVT::I32], &[a, b]);
    let first = dag.get_copy_to_phys_reg(entry, eflags_id, SDValue { node: add.node, res: 1 }, None);
    let adc = machine(&mut dag, ADC, &[EVT::I32], &[c, d, SDValue { node: first.node, res: 1 }]);
    let test = machine(&mut dag, TEST, &[EVT::I32], &[c, d]);
    let second = dag.get_copy_to_phys_reg(entry, eflags_id, test, None);
    let cmov = machine(&mut dag, CMOV, &[EVT::I32], &[add, c, SDValue { node: second.node, res: 1 }]);
    let store_cmov = store(&mut dag, cmov, 5, chain_of(d));
    let root = store(&mut dag, adc, 6, store_cmov);
    let order = schedule(&dag, root, &FlagsInfo(eflags));
    let (at_add, at_adc, at_test) = (position(&order, add), position(&order, adc), position(&order, test));
    assert!(at_add < at_adc, "{order:?}");
    assert!(!(at_add < at_test && at_test < at_adc), "a flag writer between the add and the adc: {order:?}");
}

/// The `adc` reads the `cmov`, which needs its own flags, so the `cmov` cannot
/// go below the `adc` and the `add`'s carry is live across the `test`: the
/// first held unit went anyway, a `test` between the `add` and the `adc`
/// (x_ll_arith's three remaining carry saves). The pin clones the `add` for
/// the `adc` (`CopyAndMoveSuccessors`).
#[test]
fn a_flag_definition_wanted_across_a_flag_writer_is_cloned() {
    let eflags_id = super::dagisel_adapter::EFLAGS_ID;
    let eflags = super::dagisel_adapter::pin_register(eflags_id).expect("EFLAGS has a pin number");
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = load(&mut dag, 1, entry);
    let b = load(&mut dag, 2, chain_of(a));
    let c = load(&mut dag, 3, chain_of(b));
    let d = load(&mut dag, 4, chain_of(c));
    let add = machine(&mut dag, ADD, &[EVT::I32, EVT::I32], &[a, b]);
    let first = dag.get_copy_to_phys_reg(entry, eflags_id, SDValue { node: add.node, res: 1 }, None);
    let test = machine(&mut dag, TEST, &[EVT::I32], &[c, d]);
    let second = dag.get_copy_to_phys_reg(entry, eflags_id, test, None);
    let cmov = machine(&mut dag, CMOV, &[EVT::I32], &[add, c, SDValue { node: second.node, res: 1 }]);
    let adc = machine(&mut dag, ADC, &[EVT::I32], &[cmov, d, SDValue { node: first.node, res: 1 }]);
    let root = store(&mut dag, adc, 6, chain_of(d));
    let order = schedule(&dag, root, &FlagsInfo(eflags));
    let adds: Vec<usize> = (0..order.len()).filter(|&i| order[i].nodes.contains(&add.node)).collect();
    let (at_test, at_adc) = (position(&order, test), position(&order, adc));
    let last_add = *adds.last().expect("scheduled");
    assert!(last_add < at_adc, "{order:?}");
    assert!(!(last_add < at_test && at_test < at_adc), "a flag writer between the add and the adc: {order:?}");
}

/// A load addressed by a target constant pool entry or frame slot got a unit
/// for the address, as `isPassiveNode` leaves none: the extra unit shifted the
/// queue order, so `recfloat` stacked its two FP constants the other way round.
#[test]
fn target_address_leaves_are_not_scheduled() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let pool = dag.get_constant_pool(EVT::I32, 0, EVT::I64, true);
    let slot = dag.get_target_frame_index(EVT::I32, 0);
    let a = machine(&mut dag, LOAD, &[EVT::I32, EVT::OTHER], &[pool, entry]);
    let b = machine(&mut dag, LOAD, &[EVT::I32, EVT::OTHER], &[slot, chain_of(a)]);
    let order = schedule(&dag, b, &Info);
    assert_eq!(order.len(), 2, "{order:?}");
}
