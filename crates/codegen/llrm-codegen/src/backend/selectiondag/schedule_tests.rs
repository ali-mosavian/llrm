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

struct CopyInfo {
    flags: [u16; 1],
    clobbers: bool,
}

impl SchedInfo for CopyInfo {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(matches!(opcode, ADD | LOAD | CMP))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        if matches!(opcode, CMP) { &self.flags } else { &[] }
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
        reg == self.flags[0]
    }

    fn copy_clobbers(&self) -> Option<u16> {
        self.clobbers.then_some(self.flags[0])
    }

    fn preference(&self) -> super::schedule::Preference {
        super::schedule::Preference::Ilp
    }
}

/// `cmp; copy; br`: the copy of a loop-carried value is chained before the
/// branch and nothing else orders it, so it landed between the compare and
/// its branch. On M68k a copy becomes a `move`, which sets the flags, and the
/// compiler saved them around it (`move.w %sr,-(%sp)` .. `move.w (%sp)+,%ccr`
/// in `hex`'s loop). A target that says its copies clobber the flag register
/// gets the copy before the compare.
#[test]
fn a_copy_that_clobbers_the_flags_is_not_scheduled_between_the_compare_and_its_branch() {
    let flags = super::dagisel_adapter::pin_register(super::dagisel_adapter::EFLAGS_ID).expect("a pin number");
    let build = |clobbers: bool| {
        let mut dag = dag();
        let entry = dag.entry_node();
        let a = load(&mut dag, 1, entry);
        let b = load(&mut dag, 2, chain_of(a));
        let cmp = machine(&mut dag, CMP, &[EVT::I32, EVT::I32], &[a, b]);
        let to_flags = dag.get_copy_to_phys_reg(
            entry,
            super::dagisel_adapter::EFLAGS_ID,
            SDValue { node: cmp.node, res: 1 },
            None,
        );
        let copy = dag.get_copy_to_reg(chain_of(b), 100, a);
        let branch = machine(&mut dag, BR, &[EVT::OTHER], &[copy, SDValue { node: to_flags.node, res: 1 }]);
        let order = schedule(&dag, branch, &CopyInfo { flags: [flags], clobbers });
        let (at_cmp, at_copy, at_branch) = (position(&order, cmp), position(&order, copy), position(&order, branch));
        (at_cmp < at_copy && at_copy < at_branch, format!("{order:?}"))
    };
    let (between, order) = build(false);
    assert!(between, "premise: a target that says nothing gets the copy between: {order}");
    let (between, order) = build(true);
    assert!(!between, "{order}");
}

/// A DAG in which two nodes are each other's operands is a cycle among the
/// units. The z80 selected `select i1` of a loaded bit into a compare of its
/// own result, and `calc_sethi_ullman` walked the cycle until the memory ran
/// out (tests/run/c `huffman_decode`: no end after 5 minutes). It stops with a
/// message naming the units.
#[test]
#[should_panic(expected = "dependence cycle")]
fn a_cycle_among_the_units_stops_the_scheduler_with_a_message() {
    let mut dag = dag();
    let entry = dag.entry_node();
    let a = load(&mut dag, 1, entry);
    let p = machine(&mut dag, ADD, &[EVT::I32], &[a, a]);
    let q = machine(&mut dag, MUL, &[EVT::I32], &[p, a]);
    let r = machine(&mut dag, ADD, &[EVT::I32], &[q, a]);
    dag.replace_all_uses_with(p, r);
    let root = store(&mut dag, r, 4, chain_of(a));
    schedule(&dag, root, &Info);
}

struct PressureInfo {
    limit: u32,
}

impl SchedInfo for PressureInfo {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(matches!(opcode, ADD | LOAD | MUL))
    }

    fn implicit_defs(
        &self,
        _opcode: u32,
    ) -> &[u16] {
        &[]
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }

    fn preference(&self) -> super::schedule::Preference {
        super::schedule::Preference::Ilp
    }

    fn reg_pressure_limit(
        &self,
        _class: u16,
    ) -> u32 {
        self.limit
    }
}

fn random_dag(seed: u64) -> (SelectionDAG, SDValue, Vec<SDValue>) {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = move |n: usize| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % n
    };
    let mut dag = dag();
    let entry = dag.entry_node();
    let mut values = Vec::new();
    for i in 0..3 + next(3) {
        values.push(load(&mut dag, 10 + i as u32, entry));
    }
    for _ in 0..2 + next(4) {
        let (x, y) = (values[next(values.len())], values[next(values.len())]);
        values.push(machine(&mut dag, ADD, &[EVT::I32], &[x, y]));
    }
    let mut stores = Vec::new();
    for i in 0..2 + next(2) {
        let v = values[values.len() - 1 - next(3.min(values.len()))];
        stores.push(store(&mut dag, v, 40 + i as u32, entry));
    }
    let root = dag.get_token_factor(&stores);
    (dag, root, values)
}

/// The most values live at once when the units run in `order`: a value is
/// live from its unit until the last unit that reads it.
fn peak_live(
    dag: &SelectionDAG,
    order: &[Unit],
) -> usize {
    let at = |node: u32| order.iter().position(|unit| unit.nodes.contains(&node));
    let mut peak = 0;
    for step in 0..order.len() {
        let live = order[..=step]
            .iter()
            .flat_map(|unit| unit.nodes.iter().copied())
            .filter(|&node| {
                matches!(
                    dag.node(SDValue { node, res: 0 }).opcode,
                    ISD::Machine(op) if matches!(op, ADD | LOAD)
                ) && dag.users(SDValue { node, res: 0 }).any(|user| at(user).is_some_and(|when| when > step))
            })
            .count();
        peak = peak.max(live);
    }
    peak
}

fn peak_with_limit(
    seed: u64,
    limit: u32,
) -> usize {
    let (dag, root, _) = random_dag(seed);
    peak_live(&dag, &schedule(&dag, root, &PressureInfo { limit }))
}

/// The ILP queue put the unit that adds the least pressure first in every
/// class, as the pin does with `getRegPressureLimit` 0, whatever the target
/// said: values were kept live to no end on a target with registers to spare
/// (the Z80's lz4 copy loop held five pairs). A class under its limit does not
/// count; one at it does. Seed 134: two values live at most at limit 0, four
/// when the class is never at its limit, two again once the limit is 2.
#[test]
fn a_class_under_its_pressure_limit_does_not_steer_the_ilp_queue() {
    assert_eq!(peak_with_limit(134, 0), 2);
    assert_eq!(peak_with_limit(134, 100), 4, "the limit is read");
    assert_eq!(peak_with_limit(134, 2), 2, "the pressure reaches the limit");
}

/// A target that sets no limit gets the pin's 0: m68k's schedules must not
/// move.
#[test]
fn a_target_without_a_pressure_limit_schedules_as_a_limit_of_zero() {
    struct Default;
    impl SchedInfo for Default {
        fn num_defs(
            &self,
            opcode: u32,
        ) -> usize {
            usize::from(matches!(opcode, ADD | LOAD | MUL))
        }
        fn implicit_defs(
            &self,
            _opcode: u32,
        ) -> &[u16] {
            &[]
        }
        fn is_call(
            &self,
            _opcode: u32,
        ) -> bool {
            false
        }
        fn preference(&self) -> super::schedule::Preference {
            super::schedule::Preference::Ilp
        }
    }
    for seed in 0..200 {
        let (dag, root, _) = random_dag(seed);
        let (zero_dag, zero_root, _) = random_dag(seed);
        assert_eq!(
            schedule(&dag, root, &Default),
            schedule(&zero_dag, zero_root, &PressureInfo { limit: 0 }),
            "seed {seed}"
        );
    }
}

struct FlagValue {
    fixed: bool,
    pref: super::schedule::Preference,
}

impl SchedInfo for FlagValue {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        usize::from(matches!(opcode, ADD | LOAD | CMP))
    }

    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16] {
        if matches!(opcode, ADD) { &[FLAGS] } else { &[] }
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }

    fn fixed_register_def(
        &self,
        opcode: u32,
        _result: usize,
    ) -> Option<u16> {
        (self.fixed && opcode == CMP).then_some(FLAGS)
    }

    fn source_order(&self) -> bool {
        self.pref == super::schedule::Preference::Source
    }

    fn preference(&self) -> super::schedule::Preference {
        self.pref
    }
}

/// `cmp` and an `add` (which writes the flags) both feed `br`.
fn flag_dag(seed: u64) -> (SelectionDAG, SDValue, SDValue, SDValue) {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = move |n: usize| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 33) as usize) % n
    };
    let mut dag = dag();
    let entry = dag.entry_node();
    let mut vals: Vec<SDValue> = (0..2 + next(3)).map(|i| load(&mut dag, 1 + i as u32, entry)).collect();
    let order_cmp_first = next(2) == 0;
    let mut make_cmp = |dag: &mut SelectionDAG, vals: &[SDValue], next: &mut dyn FnMut(usize) -> usize| {
        machine(dag, CMP, &[EVT::I8], &[vals[next(vals.len())], vals[next(vals.len())]])
    };
    let mut cmp = None;
    if order_cmp_first {
        cmp = Some(make_cmp(&mut dag, &vals, &mut next));
    }
    for _ in 0..1 + next(3) {
        let (x, y) = (vals[next(vals.len())], vals[next(vals.len())]);
        vals.push(machine(&mut dag, ADD, &[EVT::I32], &[x, y]));
    }
    let cmp = cmp.unwrap_or_else(|| make_cmp(&mut dag, &vals, &mut next));
    let sum = *vals.last().unwrap();
    let keep = dag.get_copy_to_reg(entry, 100, sum);
    let branch = machine(&mut dag, BR, &[EVT::OTHER], &[cmp, keep]);
    (dag, branch, cmp, sum)
}

/// A compare whose result is the flags register itself (the Z80's `cp`, `outs
/// Flags:$f`) was a value to the scheduler, so an `add`, which writes the
/// flags, went between the compare and the branch that reads it, and the
/// compare was issued again (`find` under the ILP queue: 17 bytes, 379
/// T-states, against 16 and 337). A target that says the result is the register
/// gets the compare next to its branch.
#[test]
fn a_flag_writer_is_not_scheduled_between_a_compare_and_the_branch_that_reads_it() {
    let pref = super::schedule::Preference::Source;
    let between = |fixed: bool| {
        let (dag, branch, cmp, sum) = flag_dag(2);
        let order = schedule(&dag, branch, &FlagValue { fixed, pref });
        let (c, s, b) = (position(&order, cmp), position(&order, sum), position(&order, branch));
        (c < s && s < b, format!("cmp {c} add {s} br {b}"))
    };
    let (split, order) = between(false);
    assert!(split, "premise: a target that says nothing gets the add between: {order}");
    let (split, order) = between(true);
    assert!(!split, "{order}");
}
