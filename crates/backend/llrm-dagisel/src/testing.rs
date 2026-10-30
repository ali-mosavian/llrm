//! A small DAG for the tests and for pricing: nodes in a vector, uses found by
//! scanning.

use crate::dag::{Dag, FpClass, Hooks, LoadExt, MemRef, SDValue, X86Predicates};
use crate::generated::ids::{isd, mvt};
use crate::matcher::{Opcode, Vt};
use crate::x86_addr::{AddrDag, Symbol, Wrapped};

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub opcode: Opcode,
    pub machine: Option<u16>,
    pub vts: Vec<Vt>,
    pub ops: Vec<SDValue>,
    pub constant: Option<i64>,
    pub register: Option<u16>,
    pub load: Option<LoadExt>,
    pub memory_vt: Option<Vt>,
    pub symbol: Option<Wrapped>,
    /// A `VTSDNode`'s type.
    pub vt_node: Option<Vt>,
    /// The segment register the memory node addresses through.
    pub segment: Option<u16>,
    pub dead: bool,
    /// `SDNodeFlags::hasNoUnsignedWrap` (`add nuw`).
    pub no_unsigned_wrap: bool,
    /// `AtomicSDNode::getMergedOrdering` as LLVM's number; none for a plain access.
    pub atomic_ordering: Option<u8>,
}

#[derive(Default)]
pub struct TestDag {
    pub nodes: Vec<Node>,
}

impl TestDag {
    pub fn new() -> TestDag {
        let mut dag = TestDag::default();
        dag.add(Node { opcode: isd::EntryToken, vts: vec![mvt::Other], ..Node::default() });
        dag
    }

    pub fn add(
        &mut self,
        node: Node,
    ) -> u32 {
        self.nodes.push(node);
        (self.nodes.len() - 1) as u32
    }

    #[allow(dead_code)]
    pub fn make_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node { opcode: isd::Constant, vts: vec![vt], constant: Some(value), ..Node::default() }),
            0,
        )
    }

    /// A `VTSDNode`.
    pub fn make_value_type_node(
        &mut self,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node { opcode: isd::VALUETYPE, vts: vec![mvt::Other], vt_node: Some(vt), ..Node::default() }),
            0,
        )
    }

    pub fn frame_index(
        &mut self,
        index: i32,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node {
                opcode: isd::FrameIndex,
                vts: vec![vt],
                constant: Some(i64::from(index)),
                ..Node::default()
            }),
            0,
        )
    }

    /// `Wrapper (TargetGlobalAddress id + offset)`.
    pub fn wrapped_global(
        &mut self,
        id: u32,
        offset: i64,
        vt: Vt,
    ) -> SDValue {
        let symbol = Wrapped { symbol: Symbol { kind: crate::x86_addr::SymbolKind::Global, id }, offset, flags: 0 };
        let global =
            self.add(Node { opcode: isd::TargetGlobalAddress, vts: vec![vt], symbol: Some(symbol), ..Node::default() });
        SDValue::new(self.node(crate::generated::ids::x86isd::Wrapper, &[vt], &[SDValue::new(global, 0)]), 0)
    }

    /// `WrapperRIP (TargetGlobalAddress id + offset)`.
    pub fn wrapped_global_rip(
        &mut self,
        id: u32,
        offset: i64,
        vt: Vt,
    ) -> SDValue {
        let symbol = Wrapped { symbol: Symbol { kind: crate::x86_addr::SymbolKind::Global, id }, offset, flags: 0 };
        let global =
            self.add(Node { opcode: isd::TargetGlobalAddress, vts: vec![vt], symbol: Some(symbol), ..Node::default() });
        SDValue::new(self.node(crate::generated::ids::x86isd::WrapperRIP, &[vt], &[SDValue::new(global, 0)]), 0)
    }

    /// A value held in a register: `CopyFromReg` of the entry chain.
    pub fn input(
        &mut self,
        register: u16,
        vt: Vt,
    ) -> SDValue {
        let reg = self.register(register, vt);
        let node = self.add(Node {
            opcode: isd::CopyFromReg,
            vts: vec![vt, mvt::Other],
            ops: vec![SDValue::new(0, 0), reg],
            ..Node::default()
        });
        SDValue::new(node, 0)
    }

    pub fn node(
        &mut self,
        opcode: Opcode,
        vts: &[Vt],
        ops: &[SDValue],
    ) -> u32 {
        self.add(Node { opcode, vts: vts.to_vec(), ops: ops.to_vec(), ..Node::default() })
    }

    /// The nodes still in the DAG.
    pub fn live(&self) -> impl Iterator<Item = (u32, &Node)> {
        self.nodes.iter().enumerate().filter(|(_, n)| !n.dead).map(|(i, n)| (i as u32, n))
    }

    fn uses(
        &self,
        of: u32,
    ) -> impl Iterator<Item = SDValue> + '_ {
        self.live().flat_map(|(_, n)| n.ops.iter().copied()).filter(move |v| v.node == of)
    }
}

impl Dag for TestDag {
    fn opcode(
        &self,
        node: u32,
    ) -> Opcode {
        let n = &self.nodes[node as usize];
        n.machine.map_or(n.opcode, |m| !m)
    }
    fn is_machine(
        &self,
        node: u32,
    ) -> bool {
        self.nodes[node as usize].machine.is_some()
    }
    fn num_operands(
        &self,
        node: u32,
    ) -> usize {
        self.nodes[node as usize].ops.len()
    }
    fn operand(
        &self,
        node: u32,
        index: usize,
    ) -> SDValue {
        self.nodes[node as usize].ops[index]
    }
    fn num_values(
        &self,
        node: u32,
    ) -> usize {
        self.nodes[node as usize].vts.len()
    }
    fn value_type(
        &self,
        value: SDValue,
    ) -> Vt {
        self.nodes[value.node as usize].vts[usize::from(value.res)]
    }
    fn has_one_use(
        &self,
        value: SDValue,
    ) -> bool {
        self.uses(value.node).filter(|u| u.res == value.res).count() == 1
    }
    fn count_non_chain_uses(
        &self,
        node: u32,
        limit: usize,
    ) -> usize {
        self.uses(node).filter(|u| self.value_type(*u) != mvt::Other).take(limit + 1).count()
    }
    fn use_empty(
        &self,
        node: u32,
    ) -> bool {
        self.uses(node).next().is_none()
    }
    fn node_has_one_use(
        &self,
        node: u32,
    ) -> bool {
        self.uses(node).count() == 1
    }
    fn users(
        &self,
        value: SDValue,
    ) -> Vec<(u32, usize)> {
        self.live()
            .flat_map(|(i, n)| n.ops.iter().enumerate().filter(move |(_, o)| **o == value).map(move |(k, _)| (i, k)))
            .collect()
    }
    fn glued_user(
        &self,
        node: u32,
    ) -> Option<u32> {
        let last = SDValue::new(node, (self.nodes[node as usize].vts.len() - 1) as u8);
        (self.value_type(last) == mvt::Glue).then(|| self.users(last).first().map(|u| u.0)).flatten()
    }
    fn is_only_user_of(
        &self,
        node: u32,
        def: u32,
    ) -> bool {
        self.live().all(|(i, n)| i == node || n.ops.iter().all(|o| o.node != def))
    }
    fn has_n_uses_of_value(
        &self,
        value: SDValue,
        n: usize,
    ) -> bool {
        self.uses(value.node).filter(|u| u.res == value.res).count() == n
    }
    fn constant(
        &self,
        value: SDValue,
    ) -> Option<i64> {
        self.nodes[value.node as usize].constant
    }
    fn cond_code(
        &self,
        _: SDValue,
    ) -> Option<u8> {
        None
    }
    fn register_of(
        &self,
        node: u32,
    ) -> Option<u16> {
        self.nodes[node as usize].register
    }
    fn is_call_argument_store(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn value_type_node(
        &self,
        value: SDValue,
    ) -> Option<Vt> {
        self.nodes[value.node as usize].vt_node
    }
    fn is_undef(
        &self,
        _: SDValue,
    ) -> bool {
        false
    }
    fn mem_operands(
        &self,
        node: u32,
    ) -> Option<Vec<MemRef>> {
        self.nodes[node as usize].memory_vt.map(|_| vec![node])
    }
    fn load_ext_type(
        &self,
        node: u32,
    ) -> Option<LoadExt> {
        self.nodes[node as usize].load
    }
    fn is_indexed(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn is_truncating_store(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn memory_vt(
        &self,
        node: u32,
    ) -> Vt {
        self.nodes[node as usize].memory_vt.expect("a memory node")
    }
    fn memory_store_size(
        &self,
        node: u32,
    ) -> u64 {
        match self.memory_vt(node) {
            v if v == mvt::i8 => 1,
            v if v == mvt::i16 => 2,
            v if v == mvt::i32 || v == mvt::f32 => 4,
            _ => 8,
        }
    }
    fn alignment(
        &self,
        _: u32,
    ) -> u64 {
        4
    }
    fn is_simple(
        &self,
        _: u32,
    ) -> bool {
        true
    }
    fn fp_class(
        &self,
        _: SDValue,
    ) -> Option<FpClass> {
        None
    }
    fn has_disjoint_flag(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn atomic_ordering(
        &self,
        node: u32,
    ) -> Option<u8> {
        self.nodes[node as usize].atomic_ordering
    }
    fn has_no_unsigned_wrap(
        &self,
        node: u32,
    ) -> bool {
        self.nodes[node as usize].no_unsigned_wrap
    }
    /// Known bits of the two shapes a pattern asks about: a constant, and an
    /// `and` with a constant (a bit the constant clears is zero). Anything else
    /// is not known to be zero.
    fn masked_value_is_zero(
        &self,
        value: SDValue,
        mask: u64,
    ) -> bool {
        let node = &self.nodes[value.node as usize];
        let zeros = |constant: i64| constant as u64 & mask == 0;
        match (node.opcode, node.constant) {
            (_, Some(constant)) => zeros(constant),
            (op, None) if op == isd::AND => {
                node.ops.iter().any(|operand| self.nodes[operand.node as usize].constant.is_some_and(|c| zeros(c)))
            }
            _ => false,
        }
    }
    fn have_no_common_bits_set(
        &self,
        _: SDValue,
        _: SDValue,
    ) -> bool {
        false
    }
    fn mem_ref_is_load(
        &self,
        mem: MemRef,
    ) -> bool {
        self.nodes[mem as usize].load.is_some()
    }
    fn mem_ref_is_store(
        &self,
        mem: MemRef,
    ) -> bool {
        self.nodes[mem as usize].load.is_none()
    }
    fn set_node_mem_refs(
        &mut self,
        _: u32,
        _: &[MemRef],
    ) {
    }

    fn entry_node(&mut self) -> SDValue {
        SDValue::new(0, 0)
    }
    fn target_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node { opcode: isd::TargetConstant, vts: vec![vt], constant: Some(value), ..Node::default() }),
            0,
        )
    }
    fn convert_to_target(
        &mut self,
        value: SDValue,
    ) -> SDValue {
        match self.nodes[value.node as usize].constant {
            Some(c) if self.nodes[value.node as usize].opcode == isd::Constant => {
                let vt = self.value_type(value);
                self.target_constant(c, vt)
            }
            _ => value,
        }
    }
    fn register(
        &mut self,
        register: u16,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node { opcode: isd::Register, vts: vec![vt], register: Some(register), ..Node::default() }),
            0,
        )
    }
    fn copy_to_reg(
        &mut self,
        chain: SDValue,
        register: u16,
        value: SDValue,
        glue: Option<SDValue>,
    ) -> u32 {
        let vt = self.value_type(value);
        let reg = self.register(register, vt);
        let mut ops = vec![chain, reg, value];
        ops.extend(glue);
        self.add(Node { opcode: isd::CopyToReg, vts: vec![mvt::Other, mvt::Glue], ops, ..Node::default() })
    }
    fn copy_from_reg(
        &mut self,
        chain: SDValue,
        register: u16,
        vt: Vt,
        glue: SDValue,
    ) -> u32 {
        let reg = self.register(register, vt);
        self.add(Node {
            opcode: isd::CopyFromReg,
            vts: vec![vt, mvt::Other, mvt::Glue],
            ops: vec![chain, reg, glue],
            ..Node::default()
        })
    }
    fn copy_from_reg_node(
        &mut self,
        chain: SDValue,
        register: SDValue,
        vt: Vt,
        glue: Option<SDValue>,
    ) -> u32 {
        let mut ops = vec![chain, register];
        ops.extend(glue);
        let mut vts = vec![vt, mvt::Other];
        if glue.is_some() {
            vts.push(mvt::Glue);
        }
        self.add(Node { opcode: isd::CopyFromReg, vts, ops, ..Node::default() })
    }
    fn token_factor(
        &mut self,
        chains: &[SDValue],
    ) -> SDValue {
        SDValue::new(self.node(isd::TokenFactor, &[mvt::Other], chains), 0)
    }
    fn machine_node(
        &mut self,
        opcode: u16,
        vts: &[Vt],
        operands: &[SDValue],
    ) -> u32 {
        self.add(Node { machine: Some(opcode), vts: vts.to_vec(), ops: operands.to_vec(), ..Node::default() })
    }
    fn morph_node_to(
        &mut self,
        node: u32,
        opcode: u16,
        vts: &[Vt],
        operands: &[SDValue],
    ) -> (u32, Vec<u32>) {
        let n = &mut self.nodes[node as usize];
        n.machine = Some(opcode);
        n.vts = vts.to_vec();
        n.ops = operands.to_vec();
        (node, Vec::new())
    }
    fn update_node_operands(
        &mut self,
        node: u32,
        operands: &[SDValue],
    ) -> u32 {
        self.nodes[node as usize].ops = operands.to_vec();
        node
    }
    fn replace_uses(
        &mut self,
        from: SDValue,
        to: SDValue,
    ) {
        for n in self.nodes.iter_mut().filter(|n| !n.dead) {
            for op in &mut n.ops {
                if *op == from {
                    *op = to;
                }
            }
        }
    }
    fn replace_node(
        &mut self,
        from: u32,
        to: u32,
    ) {
        for res in 0..self.nodes[from as usize].vts.len() as u8 {
            self.replace_uses(SDValue::new(from, res), SDValue::new(to, res));
        }
        self.remove_dead_node(from);
    }
    fn remove_dead_node(
        &mut self,
        node: u32,
    ) {
        self.nodes[node as usize].dead = true;
    }
    fn remove_dead_nodes(
        &mut self,
        nodes: &[u32],
    ) {
        for &n in nodes {
            self.remove_dead_node(n);
        }
    }
    fn has_predecessor_helper(
        &self,
        node: u32,
        visited: &mut crate::dag::Visited,
        worklist: &mut Vec<u32>,
        max: usize,
    ) -> bool {
        if visited.contains(node) {
            return true;
        }
        while let Some(m) = worklist.pop() {
            if m == node {
                return true;
            }
            if max != 0 && visited.len() >= max {
                return true;
            }
            for op in &self.nodes[m as usize].ops {
                if visited.insert(op.node) {
                    worklist.push(op.node);
                }
            }
        }
        false
    }
}

/// Hooks for a 32-bit target whose address pattern the test supplies.
pub struct TestHooks {
    /// What `selectAddr` answers for the address it is asked about: base,
    /// index, scale, disp.
    pub address: Option<(SDValue, SDValue, i64, i64)>,
}

impl Hooks<TestDag> for TestHooks {
    fn pointer_type(&self) -> Vt {
        mvt::i32
    }
    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        crate::generated::x86_i486::pattern_predicate(&OptSize, predicate)
    }
    fn check_node_predicate(
        &self,
        dag: &TestDag,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        crate::generated::x86_i486::node_predicate(dag, self, value, predicate)
    }
    fn check_complex_pattern(
        &mut self,
        dag: &mut TestDag,
        _root: u32,
        _parent: Option<u32>,
        _value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        // Only `selectAddr` is stubbed; the other complex patterns match
        // nothing.
        let (0, Some((base, index, scale, disp))) = (pattern, self.address) else { return false };
        let scale = dag.target_constant(scale, mvt::i8);
        let disp = dag.target_constant(disp, mvt::i32);
        let segment = dag.register(0, mvt::i16);
        results.extend([base, scale, index, disp, segment].map(|v| (v, None)));
        true
    }
    fn run_xform(
        &mut self,
        _: &mut TestDag,
        value: SDValue,
        _: u8,
    ) -> SDValue {
        value
    }
    fn is_profitable_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn is_legal_to_fold(
        &self,
        _: &TestDag,
        _: SDValue,
        _: u32,
        _: u32,
    ) -> bool {
        true
    }
    fn check_and_mask(
        &self,
        _: &TestDag,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn check_or_mask(
        &self,
        _: &TestDag,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }
    fn may_load_store(
        &self,
        _: u16,
    ) -> (bool, bool) {
        (true, true)
    }
}

impl X86Predicates<TestDag> for TestHooks {
    fn has_sse_unaligned_mem(&self) -> bool {
        false
    }
    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn is_unneeded_shift_mask(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn is_sext_absolute_symbol_ref(
        &self,
        _: &TestDag,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }
    fn has_no_carry_flag_uses(
        &self,
        _: &TestDag,
        _: SDValue,
    ) -> bool {
        false
    }
    fn is_def32(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        false
    }
    fn check_tc_ret_enough_regs(
        &self,
        _: &TestDag,
        _: u32,
    ) -> bool {
        true
    }
}

pub struct OptSize;

impl crate::dag::PatternContext for OptSize {
    fn should_opt_for_size(&self) -> bool {
        false
    }
    fn has_min_size(&self) -> bool {
        false
    }
    fn has_frame_pointer(&self) -> bool {
        false
    }
    fn is_legal_to_call_immediate_addr(&self) -> bool {
        false
    }
}

impl AddrDag for TestDag {
    fn is_add_like(
        &self,
        value: SDValue,
    ) -> bool {
        let n = &self.nodes[value.node as usize];
        n.opcode == isd::OR && self.has_disjoint_flag(value.node)
    }
    fn frame_index(
        &self,
        value: SDValue,
    ) -> Option<i32> {
        let n = &self.nodes[value.node as usize];
        (n.opcode == isd::FrameIndex).then(|| n.constant.unwrap() as i32)
    }
    fn wrapped_symbol(
        &self,
        operand: SDValue,
    ) -> Option<Wrapped> {
        self.nodes[operand.node as usize].symbol
    }
    fn has_materializing_use(
        &self,
        value: SDValue,
    ) -> bool {
        self.users(value)
            .iter()
            .any(
                |&(user, _)| {
                    let n = &self.nodes[user as usize];
                    n.opcode == isd::CopyToReg || (n.opcode == isd::STORE && n.ops.get(1) == Some(&value))
                },
            )
    }
    fn get_node(
        &mut self,
        opcode: u16,
        vt: Vt,
        ops: &[SDValue],
    ) -> SDValue {
        SDValue::new(self.node(opcode, &[vt], ops), 0)
    }
    fn get_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> SDValue {
        self.make_constant(value, vt)
    }
    fn segment_override(
        &self,
        node: u32,
    ) -> Option<u16> {
        self.nodes[node as usize].segment
    }
    fn is_non_temporal(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn target_frame_index(
        &mut self,
        index: i32,
        vt: Vt,
    ) -> SDValue {
        SDValue::new(
            self.add(Node {
                opcode: isd::TargetFrameIndex,
                vts: vec![vt],
                constant: Some(i64::from(index)),
                ..Node::default()
            }),
            0,
        )
    }
    fn target_symbol(
        &mut self,
        symbol: Symbol,
        disp: i32,
        flags: u8,
    ) -> SDValue {
        let wrapped = Wrapped { symbol, offset: i64::from(disp), flags };
        SDValue::new(
            self.add(Node {
                opcode: isd::TargetGlobalAddress,
                vts: vec![mvt::i32],
                symbol: Some(wrapped),
                ..Node::default()
            }),
            0,
        )
    }
}

#[cfg(test)]
mod visited_tests {
    use super::*;
    use crate::dag::Visited;

    #[test]
    fn a_visited_set_counts_each_node_once() {
        let mut seen = Visited::of(7);
        assert!(seen.contains(7) && !seen.contains(8) && !seen.contains(100_000));
        assert!(!seen.insert(7) && seen.insert(8) && seen.insert(100_000));
        assert_eq!(seen.len(), 3);
        seen.clear();
        assert!(seen.is_empty() && !seen.contains(7));
    }

    /// `hasPredecessorHelper` kept its visited nodes in a list searched for
    /// each operand: a walk of n nodes cost n squared (37 calls of
    /// `IsLegalToFold` were 349 M of 3.8 G instructions on a 256-statement
    /// block). A chain of 400,000 nodes is walked in well under a second; the
    /// list took about six seconds.
    #[test]
    fn walking_a_long_chain_costs_the_nodes_not_their_square() {
        let mut dag = TestDag::new();
        let mut last = 0;
        for _ in 0..400_000 {
            last = dag.add(Node {
                opcode: isd::CopyFromReg,
                vts: vec![mvt::i16],
                ops: vec![SDValue::new(last, 0)],
                ..Node::default()
            });
        }
        let started = std::time::Instant::now();
        let mut visited = Visited::new();
        let mut worklist = vec![last];
        assert!(!dag.has_predecessor_helper(u32::MAX - 1, &mut visited, &mut worklist, 0));
        assert!(visited.len() > 300_000);
        assert!(started.elapsed().as_secs_f64() < 1.0, "{:?}", started.elapsed());
    }
}
