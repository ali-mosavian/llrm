//! A switch lowered as gcc 13.4 lowers one: a lookup table where every case
//! only picks constants, bit tests for few targets in a narrow range, and
//! otherwise a balanced tree of range tests.
//!
//! # How it works
//! The cases are sorted and the ones that share a target and follow one
//! another are made one range (`group_case_labels`); cases to the default
//! are dropped. The values are ordered signed or unsigned, whichever makes
//! the narrower span: a `-1, 0, 1` switch is three adjacent values.
//!
//! Switch conversion (gcc's `-ftree-switch-conversion`, -O2 and up; LLVM's
//! `SimplifyCFG::SwitchToLookupTable`). Every case target is the join `J` or
//! an empty block that jumps to it, and every phi of `J` takes a constant from
//! each. Where bit tests are not better (`switch_conversion::expand`), the
//! phis become loads from a private constant array indexed by `x - min`,
//! behind one `x - min <= span` test; a hole holds the default's constant.
//! One decision makes it: at least three case values at a density of 10% of
//! the span (40% tuned for size; LLVM's `isSwitchDense`), and the lookup
//! priced under the tree it replaces by the target's costs. A path of the
//! tree is the average depth of a balanced tree of compare and branch, and a
//! move for each phi; the table's the range test and a load for each phi, a
//! widening where its cells are narrower. Tuned for size the costs are bytes
//! and the whole tree is set against the code and the data of the table.
//!
//! Bit tests (`bit_test_cluster`). A run of clusters narrower than a word with
//! at most 3 targets, and 3 cases for one target, 5 for two or 6 for three, is
//! `1 << (x - min)` tested against one mask per target.
//!
//! The tree (`switch_decision_tree`). What is left is balanced by count and
//! tested as `emit_case_nodes` does: a single value by equality, a range by
//! `x - low <= span` when it is a leaf, a node with children by the compares
//! `x > high`, `x >= low`.
//!
//! Jump tables (`jump_table_cluster`), over the plain clusters left: the fewest
//! runs whose span is at most 8 times (3 tuned for size) the comparisons they
//! replace, and at least the target's `jump_table` cases (gcc's
//! `case_values_threshold`, 5 on x86; 0: no table jump). Such a run is `x -
//! low` behind its range test and a `switch table` over the indices `0..n`,
//! every index a block that only jumps on; the backend makes it a jump through
//! a table of code addresses. `-fno-jump-tables` leaves them in the tree. A run
//! dense as a whole is one table without the search for the fewest runs
//! (LLVM's `findJumpTables` "cheap case").
//!
//! # Runs
//! As two passes, gcc's: `switchconv` (`pass_convert_switch`, -O2 and up),
//! which makes the lookup tables and so adds loads, and last of the function
//! passes `switchlower` (`pass_lower_switch`), every level that optimizes,
//! which adds no memory operation: the module analyses frozen at the end of the
//! pipeline stand past it.
//!
//! # Example
//! Before:
//!
//! ```mir
//! b0:
//!   switch i32 %k, label %d [ i32 0, label %a
//!                             i32 1, label %a
//!                             i32 2, label %a
//!                             i32 40, label %b ]
//! ```
//!
//! After, the first three one range and the tree two ranges:
//!
//! ```mir
//! b0:
//!   %t = icmp ugt i32 %k, 2
//!   br i1 %t, label %m, label %a
//! ```

use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::target::OperationCosts;
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::IndexSet;

use crate::edges;

/// gcc's `bit_test_cluster::m_max_case_bit_tests`.
const MOST_TARGETS: usize = 3;

/// `convert`: gcc's `pass_convert_switch`, which makes the lookup tables
/// (-O2 and up) and so adds the loads; else `pass_lower_switch`, which adds no
/// memory operation and leaves the module analyses frozen at the end of the
/// pipeline standing.
pub struct SwitchLower {
    pub convert: bool,
    /// A dense switch may be a jump table (`-fjump-tables`).
    pub jump_tables: bool,
    /// Tuned for size: gcc's jump tables may grow the code less.
    pub size: bool,
}

impl FunctionPass for SwitchLower {
    fn name(&self) -> &'static str {
        if self.convert { "switchconv" } else { "switchlower" }
    }

    fn adds_memory_operations(&self) -> bool {
        self.convert
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let mut threshold = None;
        let mut costs: Option<OperationCosts> = None;
        let word = unit.layout.largest_legal_integer().min(64);
        let pointer = unit.layout.pointer(0).bits;
        let mut changed = false;
        for block in unit.function.layout().to_vec() {
            let Some(switch) = switch_at(unit.context, unit.function, block) else { continue };
            // Asked of the target once a function has a switch to lower.
            let threshold = *threshold
                .get_or_insert_with(|| if self.jump_tables { analyses.outer().target().costs().jump_table } else { 0 });
            let costs = self
                .convert
                .then(
                    || costs
                        .get_or_insert_with(|| {
                            let target = analyses.outer().target();
                            if self.size { target.size_costs() } else { target.costs() }
                        })
                        .clone(),
                );
            let mut lowering = Lowering {
                unit: &mut *unit,
                switch,
                word,
                pointer,
                threshold,
                costs,
                size: self.size,
                flip: 0,
                edges: Vec::new(),
            };
            changed |= lowering.run(self.convert);
        }
        if !changed {
            return PreservedAnalyses::all();
        }
        llrm_analysis::cfg::_unreachable(unit.context, unit.function);
        crate::cfg::merged(unit.function);
        PreservedAnalyses::none()
    }
}

/// One case label after grouping: the values `low..=high` in `Order`'s keys.
#[derive(Clone)]
struct Cluster {
    low: u128,
    high: u128,
    target: BlockId,
    /// A run of cases as bit tests: the mask of each target, from `low`.
    tests: Vec<(u128, BlockId)>,
    /// A run of cases as a jump table: each case's values and target.
    parts: Vec<(u128, u128, BlockId)>,
}

impl Cluster {
    fn single(&self) -> bool {
        self.low == self.high && self.tests.is_empty()
    }
}

/// One phi of the join, as the array it becomes: `width` bits a cell, widened
/// to its `bits` by `extend`.
struct Table {
    phi: InstId,
    ty: TypeId,
    bits: u32,
    width: u32,
    extend: CastOp,
    cells: Vec<u128>,
}

struct Switch {
    block: BlockId,
    inst: InstId,
    value: Operand,
    ty: TypeId,
    bits: u32,
    default: BlockId,
    cases: Vec<(u128, BlockId)>,
}

fn switch_at(
    context: &Context,
    function: &Function,
    block: BlockId,
) -> Option<Switch> {
    let inst = function.terminator(block)?;
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Switch {
        return None;
    }
    let value = instruction.operands[0];
    let ty = function.operand_type(context, value)?;
    let bits = context.types.int_bits(ty)?;
    let Operand::Block(default) = instruction.operands[1] else { return None };
    let mut cases = Vec::new();
    for pair in instruction.operands[2..].chunks(2) {
        let [Operand::Constant(case), Operand::Block(target)] = *pair else { return None };
        let ConstantKind::Int(case) = context.get(case).kind else { return None };
        if target != default {
            cases.push((case, target));
        }
    }
    Some(Switch { block, inst, value, ty, bits, default, cases })
}

/// The tree node for a cluster, as gcc's `case_tree_node`.
struct Node {
    cluster: Cluster,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn has_child(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }

    fn leaf_single(&self) -> bool {
        !self.has_child() && self.cluster.single()
    }
}

/// `balance_case_nodes`: lists of up to two stay a chain; longer ones are
/// split at the middle.
fn balanced(clusters: &[Cluster]) -> Option<Box<Node>> {
    let node = |cluster: &Cluster, left, right| Some(Box::new(Node { cluster: cluster.clone(), left, right }));
    match clusters {
        [] => None,
        [one] => node(one, None, None),
        [first, second] => node(first, None, node(second, None, None)),
        _ => {
            let middle = clusters.len() / 2;
            node(&clusters[middle], balanced(&clusters[..middle]), balanced(&clusters[middle + 1..]))
        }
    }
}

struct Lowering<'a, 'b> {
    unit: &'a mut Unit<'b>,
    switch: Switch,
    /// The widest native integer, and the pointer's width.
    word: u32,
    pointer: u32,
    /// The fewest cases of a jump table, 0 where the target has none, and
    /// whether the code is tuned for size.
    threshold: i64,
    /// What the target prices the operations at, in clocks or, tuned for
    /// size, bytes: asked only to decide a lookup table.
    costs: Option<OperationCosts>,
    size: bool,
    /// The order's flip: a key is the value xor this.
    flip: u128,
    /// The edges the lowering made, source and target, for the targets' phis.
    edges: Vec<(BlockId, BlockId)>,
}

impl Lowering<'_, '_> {
    /// Whether the switch was lowered: `convert` makes only the lookup table
    /// and leaves any other switch be.
    fn run(
        &mut self,
        convert: bool,
    ) -> bool {
        let sign = 1u128 << (self.switch.bits - 1);
        let grouped = |flip: u128| {
            let mut cases: Vec<(u128, BlockId)> =
                self.switch.cases.iter().map(|&(value, target)| (value ^ flip, target)).collect();
            cases.sort();
            let mut clusters: Vec<Cluster> = Vec::new();
            for (key, target) in cases {
                match clusters.last_mut() {
                    Some(last) if last.target == target && last.high + 1 == key => last.high = key,
                    _ => clusters.push(Cluster { low: key, high: key, target, tests: Vec::new(), parts: Vec::new() }),
                }
            }
            clusters
        };
        let span = |clusters: &[Cluster]| clusters.last().map_or(0, |last| last.high - clusters[0].low);
        let (unsigned, ordered) = (grouped(0), grouped(sign));
        let clusters = if span(&ordered) < span(&unsigned) {
            self.flip = sign;
            ordered
        } else {
            unsigned
        };
        if convert {
            return !clusters.is_empty() && self.converted(&clusters);
        }
        let clusters = self.with_bit_tests(clusters);
        let clusters = self.with_jump_tables(clusters);
        let root = balanced(&clusters);
        let originals = self.originals();
        let block = self.switch.block;
        self.unit.function.erase(self.switch.inst).expect("a terminator defines nothing");
        match &root {
            Some(root) if clusters.len() == 1 && !root.cluster.tests.is_empty() => {
                self.tests(block, &root.cluster, true)
            }
            Some(root) if clusters.len() == 1 && !root.cluster.parts.is_empty() => {
                self.table(block, &root.cluster, true)
            }
            _ => self.node(block, root.as_deref()),
        }
        self.fixed(&originals);
        true
    }

    /// Switch conversion: the phis of the join every case reaches become loads
    /// from tables. False where `clusters` do not qualify.
    fn converted(
        &mut self,
        clusters: &[Cluster],
    ) -> bool {
        let function = &*self.unit.function;
        let switch = &self.switch;
        let block = switch.block;
        // The block an empty one reached from the switch alone jumps on to.
        let forwarded = |one: BlockId| {
            let [only] = function.block(one).instructions()[..] else { return None };
            let jump = function.instruction(only);
            let [Operand::Block(to)] = jump.operands[..] else { return None };
            (jump.opcode == Opcode::Br && function.predecessors(one) == [block]).then_some(to)
        };
        let first = clusters[0].target;
        let Some(join) = (function.predecessors(first).len() > 1).then_some(first).or_else(|| forwarded(first)) else {
            return false;
        };
        let reaches = |one: BlockId| one == join || forwarded(one) == Some(join);
        let from = |one: BlockId| if one == join { block } else { one };
        if join == block || !reaches(switch.default) || !clusters.iter().all(|one| reaches(one.target)) {
            return false;
        }
        let phis = edges::phis(function, join);
        let constant = |value: Operand| match value {
            Operand::Constant(id) => match self.unit.context.get(id).kind {
                ConstantKind::Int(bits) => Some(bits),
                _ => None,
            },
            _ => None,
        };
        let incoming = |phi: InstId, pred: BlockId| {
            function
                .instruction(phi)
                .operands
                .chunks(2)
                .find(|pair| pair[1] == Operand::Block(pred))
                .map(|pair| pair[0])
        };
        // One column of values per phi, the default's constant first.
        let mut columns: Vec<(InstId, TypeId, u128, Vec<u128>)> = Vec::new();
        for &phi in &phis {
            let ty = function.instruction(phi).ty;
            if self.unit.context.types.int_bits(ty).is_none() {
                return false;
            }
            let Some(hole) = incoming(phi, from(switch.default)).and_then(constant) else { return false };
            let mut values = Vec::new();
            for one in clusters {
                let Some(value) = incoming(phi, from(one.target)).and_then(constant) else { return false };
                values.push(value);
            }
            columns.push((phi, ty, hole, values));
        }
        // Distinct targets by what they give the phis, as gcc's `m_uniq`.
        let mut distinct: Vec<Vec<u128>> = Vec::new();
        for at in 0..clusters.len() {
            let row: Vec<u128> = columns.iter().map(|column| column.3[at]).collect();
            if !distinct.contains(&row) {
                distinct.push(row);
            }
        }
        let count: u128 = clusters.iter().map(|one| 1 + u128::from(one.high != one.low)).sum();
        let span = clusters[clusters.len() - 1].high - clusters[0].low;
        let word = u128::from(self.word);
        // gcc's `expand`: bit tests first where they are better.
        if span < word && distinct.len() <= MOST_TARGETS && beneficial(count as usize, distinct.len()) {
            return false;
        }
        let tables: Vec<Table> = columns
            .iter()
            .map(|(phi, ty, hole, values)| {
                let bits = self.unit.context.types.int_bits(*ty).expect("an integer phi");
                let mut cells = vec![*hole; span as usize + 1];
                for (one, &value) in clusters.iter().zip(values) {
                    cells[(one.low - clusters[0].low) as usize..=(one.high - clusters[0].low) as usize].fill(value);
                }
                let (width, extend) = cell_width(&cells, bits);
                Table { phi: *phi, ty: *ty, bits, width, extend, cells }
            })
            .collect();
        if !self.table_pays(clusters, &tables) {
            return false;
        }
        self.tabled(clusters, join, tables);
        true
    }

    /// LLVM's `SwitchToLookupTable`: a table of at least three cases at a
    /// density of 10% of the span (40% tuned for size, `isSwitchDense`), where
    /// the target prices the lookup under the compares of the tree it would
    /// otherwise be: the clocks of a path (the average depth of a balanced tree
    /// of compares and a move for each phi, against the range test and a load
    /// for each), or, tuned for size, the bytes of all of it with the table's.
    fn table_pays(
        &self,
        clusters: &[Cluster],
        tables: &[Table],
    ) -> bool {
        let values: u128 = clusters.iter().map(|one| one.high - one.low + 1).sum();
        let span = clusters[clusters.len() - 1].high - clusters[0].low + 1;
        let density: u128 = if self.size { 40 } else { 10 };
        let Some(costs) = &self.costs else { return false };
        if values < 3 || values * 100 < span * density {
            return false;
        }
        let (clusters, phis) = (clusters.len() as i64, tables.len() as i64);
        let compare = costs.add + costs.branch;
        let range = 2 * costs.add + costs.branch;
        let loads: i64 =
            tables.iter().map(|one| costs.load + if one.width < one.bits { costs.extend } else { 0 }).sum();
        if self.size {
            let tree = clusters * (compare + phis * costs.r#move + costs.branch);
            let data: i64 = tables.iter().map(|one| span as i64 * i64::from(one.width) / 8).sum();
            return range + loads + data < tree;
        }
        let depth = i64::from(u64::BITS - (clusters as u64).leading_zeros());
        range + loads < depth * compare + phis * costs.r#move
    }

    /// The tables, the range test and the loads that replace the switch.
    fn tabled(
        &mut self,
        clusters: &[Cluster],
        join: BlockId,
        tables: Vec<Table>,
    ) {
        let (block, default) = (self.switch.block, self.switch.default);
        let (low, span) = (clusters[0].low, clusters[clusters.len() - 1].high - clusters[0].low);
        let cases: Vec<BlockId> = clusters.iter().map(|one| one.target).filter(|&one| one != join).collect();
        self.unit.function.erase(self.switch.inst).expect("a terminator defines nothing");
        let index = self.offset(block, low);
        let limit = Operand::Constant(self.unit.context.int(self.switch.ty, span as i128));
        let inside = self.compare(block, IntPredicate::Ule, index, limit);
        let load = self.fresh(block);
        self.branch(block, inside, load, default);
        let pointer = self.pointer;
        let wide = self.unit.context.types.int(pointer);
        let index = match self.switch.bits.cmp(&pointer) {
            std::cmp::Ordering::Less => self.put(load, Opcode::Cast(CastOp::ZExt), wide, vec![index]),
            std::cmp::Ordering::Greater => self.put(load, Opcode::Cast(CastOp::Trunc), wide, vec![index]),
            std::cmp::Ordering::Equal => index,
        };
        let address = self.unit.context.types.ptr(0);
        let zero = Operand::Constant(self.unit.context.int(wide, 0));
        for Table { phi, ty, bits, width, extend, cells } in tables {
            let (bits, width) = (bits, width);
            let cell = self.unit.context.types.int(width);
            let array = self.unit.context.types.intern(Type::Array { element: cell, count: cells.len() as u64 });
            let members: Vec<ConstantId> = cells.iter().map(|&one| self.unit.context.int(cell, one as i128)).collect();
            let initial = self.unit.context.constant(Constant { ty: array, kind: ConstantKind::Aggregate(members) });
            let table = self.unit.declared.table("switch.table", array, initial);
            let base = self.unit.context.constant(Constant { ty: address, kind: ConstantKind::Global(table) });
            let place = self.put(
                load,
                Opcode::GetElementPtr { source: array },
                address,
                vec![Operand::Constant(base), zero, index],
            );
            let mut loaded = self.put(load, Opcode::Load { align: None, volatile: false }, cell, vec![place]);
            if width < bits {
                loaded = self.put(load, Opcode::Cast(extend), ty, vec![loaded]);
            }
            // The cases' entries go, the default's stays, the load joins.
            let operands = self.unit.function.instruction(phi).operands.clone();
            // The switch's block keeps one edge, the default's, where that
            // goes to the join.
            let mut kept = false;
            let mut out: Vec<Operand> = Vec::new();
            for pair in operands.chunks(2) {
                let Operand::Block(from) = pair[1] else { continue };
                let keep = if from == block {
                    !std::mem::replace(&mut kept, true) && default == join
                } else {
                    !cases.contains(&from)
                };
                if keep {
                    out.extend_from_slice(pair);
                }
            }
            out.extend([loaded, Operand::Block(load)]);
            self.unit.function.set_operands(phi, out);
        }
        self.jump(load, join);
    }

    /// The targets the switch had, once each.
    fn originals(&self) -> Vec<BlockId> {
        let mut all = IndexSet::default();
        all.insert(self.switch.default);
        all.extend(self.switch.cases.iter().map(|&(_, target)| target));
        all.into_iter().collect()
    }

    fn value_of(
        &self,
        key: u128,
    ) -> u128 {
        (key ^ self.flip) & mask(self.switch.bits)
    }

    fn constant(
        &mut self,
        key: u128,
    ) -> Operand {
        let value = self.value_of(key);
        Operand::Constant(self.unit.context.int(self.switch.ty, value as i128))
    }

    fn fresh(
        &mut self,
        after: BlockId,
    ) -> BlockId {
        let block = self.unit.function.create_block(None);
        self.unit.function.insert_block(block, Some(after)).expect("a new block is placed");
        block
    }

    fn put(
        &mut self,
        block: BlockId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
    ) -> Operand {
        let inst = self.unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.unit.function.insert(inst, Position::End(block)).expect("a placed instruction");
        self.unit.function.instruction(inst).result.map_or(Operand::Block(block), Operand::Value)
    }

    fn jump(
        &mut self,
        from: BlockId,
        to: BlockId,
    ) {
        self.edges.push((from, to));
        let void = self.unit.context.types.void();
        self.put(from, Opcode::Br, void, vec![Operand::Block(to)]);
    }

    fn branch(
        &mut self,
        from: BlockId,
        condition: Operand,
        yes: BlockId,
        no: BlockId,
    ) {
        self.edges.extend([(from, yes), (from, no)]);
        let void = self.unit.context.types.void();
        self.put(from, Opcode::Br, void, vec![condition, Operand::Block(yes), Operand::Block(no)]);
    }

    fn compare(
        &mut self,
        from: BlockId,
        predicate: IntPredicate,
        left: Operand,
        right: Operand,
    ) -> Operand {
        let one = self.unit.context.types.int(1);
        self.put(from, Opcode::ICmp(predicate), one, vec![left, right])
    }

    /// `if value <predicate> key goto yes else no`, in the order's sense.
    fn test(
        &mut self,
        from: BlockId,
        predicate: Order,
        key: u128,
        yes: BlockId,
        no: BlockId,
    ) {
        let predicate = match (predicate, self.flip != 0) {
            (Order::Equal, _) => IntPredicate::Eq,
            (Order::Greater, false) => IntPredicate::Ugt,
            (Order::Greater, true) => IntPredicate::Sgt,
            (Order::AtLeast, false) => IntPredicate::Uge,
            (Order::AtLeast, true) => IntPredicate::Sge,
            (Order::Less, false) => IntPredicate::Ult,
            (Order::Less, true) => IntPredicate::Slt,
        };
        let key = self.constant(key);
        let condition = self.compare(from, predicate, self.switch.value, key);
        self.branch(from, condition, yes, no);
    }

    /// `value - low`, in the switch's type.
    fn offset(
        &mut self,
        block: BlockId,
        low: u128,
    ) -> Operand {
        let low = self.constant(low);
        if low == Operand::Constant(self.unit.context.int(self.switch.ty, 0)) {
            return self.switch.value;
        }
        self.put(block, Opcode::Binary(BinaryOp::Sub), self.switch.ty, vec![self.switch.value, low])
    }

    /// `emit_case_nodes`: every way out of `block` is a jump or a branch, and
    /// what gcc leaves to fall through goes to the default.
    fn node(
        &mut self,
        block: BlockId,
        node: Option<&Node>,
    ) {
        let default = self.switch.default;
        let Some(node) = node else { return self.jump(block, default) };
        let cluster = &node.cluster;
        let (left, right) = (node.left.as_deref(), node.right.as_deref());
        if cluster.single() {
            let next = if node.has_child() { self.fresh(block) } else { default };
            self.test(block, Order::Equal, cluster.low, cluster.target, next);
            match (left, right) {
                (Some(left), Some(right)) if left.leaf_single() && right.leaf_single() => {
                    let last = self.fresh(next);
                    self.test(next, Order::Equal, right.cluster.low, right.cluster.target, last);
                    self.test(last, Order::Equal, left.cluster.low, left.cluster.target, default);
                }
                (Some(left), Some(right)) => {
                    let (below, above) = (self.fresh(next), self.fresh(next));
                    self.test(next, Order::Greater, cluster.high, above, below);
                    self.node(below, Some(left));
                    self.node(above, Some(right));
                }
                (None, Some(only)) | (Some(only), None) => {
                    let after = right.is_some();
                    if only.has_child() || !only.cluster.single() {
                        let rest = self.fresh(next);
                        let (order, key) =
                            if after { (Order::Less, cluster.low) } else { (Order::Greater, cluster.high) };
                        self.test(next, order, key, default, rest);
                        self.node(rest, Some(only));
                    } else {
                        self.test(next, Order::Equal, only.cluster.low, only.cluster.target, default);
                    }
                }
                (None, None) => {}
            }
        } else if node.has_child() || !cluster.tests.is_empty() || !cluster.parts.is_empty() {
            let above = right.map(|_| self.fresh(block));
            let inside = self.fresh(block);
            if cluster.high == mask(self.switch.bits) {
                self.jump(block, inside);
            } else {
                self.test(block, Order::Greater, cluster.high, above.unwrap_or(default), inside);
            }
            let special = !cluster.tests.is_empty() || !cluster.parts.is_empty();
            let hit = if special { self.fresh(inside) } else { cluster.target };
            let below = left.map(|_| self.fresh(inside));
            if cluster.low == 0 {
                self.jump(inside, hit);
            } else {
                self.test(inside, Order::AtLeast, cluster.low, hit, below.unwrap_or(default));
            }
            if !cluster.tests.is_empty() {
                self.tests(hit, cluster, false);
            } else if !cluster.parts.is_empty() {
                self.table(hit, cluster, false);
            }
            if let (Some(below), Some(left)) = (below, left) {
                self.node(below, Some(left));
            }
            if let (Some(above), Some(right)) = (above, right) {
                self.node(above, Some(right));
            }
        } else {
            let offset = self.offset(block, cluster.low);
            let span = Operand::Constant(self.unit.context.int(self.switch.ty, (cluster.high - cluster.low) as i128));
            let inside = self.compare(block, IntPredicate::Ule, offset, span);
            self.branch(block, inside, cluster.target, default);
        }
    }

    /// `bit_test_cluster::emit`: `1 << (x - low)` against one mask per target.
    fn tests(
        &mut self,
        mut block: BlockId,
        cluster: &Cluster,
        entry: bool,
    ) {
        let default = self.switch.default;
        let (word, bits) = (self.word, self.switch.bits);
        // Values all inside 0..word need no subtraction, the masks shifted
        // instead (gcc: where that costs the masks no more).
        let low = self.value_of(cluster.low);
        let unshifted = self.flip == 0 && low > 0 && cluster.high < u128::from(word);
        let (base, shift) = if unshifted { (0, low as u32) } else { (cluster.low, 0) };
        let index = self.offset(block, base);
        if entry {
            let limit = Operand::Constant(self.unit.context.int(self.switch.ty, (cluster.high - base) as i128));
            let outside = self.compare(block, IntPredicate::Ugt, index, limit);
            let next = self.fresh(block);
            self.branch(block, outside, default, next);
            block = next;
        }
        let wide = self.unit.context.types.int(word);
        let amount = match bits.cmp(&word) {
            std::cmp::Ordering::Less => self.put(block, Opcode::Cast(CastOp::ZExt), wide, vec![index]),
            std::cmp::Ordering::Greater => self.put(block, Opcode::Cast(CastOp::Trunc), wide, vec![index]),
            std::cmp::Ordering::Equal => index,
        };
        let one = Operand::Constant(self.unit.context.int(wide, 1));
        let bit = self.put(block, Opcode::Binary(BinaryOp::Shl), wide, vec![one, amount]);
        for (at, &(tested, target)) in cluster.tests.iter().enumerate() {
            let tested = Operand::Constant(self.unit.context.int(wide, (tested << shift) as i128));
            let both = self.put(block, Opcode::Binary(BinaryOp::And), wide, vec![bit, tested]);
            let zero = Operand::Constant(self.unit.context.int(wide, 0));
            let set = self.compare(block, IntPredicate::Ne, both, zero);
            let next = if at + 1 == cluster.tests.len() { default } else { self.fresh(block) };
            self.branch(block, set, target, next);
            block = next;
        }
    }

    /// `find_jump_tables`: over each run of clusters left plain, the fewest
    /// that a table can take (values at most 8 times the comparisons they
    /// replace, 3 times tuned for size); the runs of at least the target's
    /// threshold become one cluster.
    fn with_jump_tables(
        &self,
        clusters: Vec<Cluster>,
    ) -> Vec<Cluster> {
        let threshold = self.threshold as usize;
        if threshold == 0 {
            return clusters;
        }
        let ratio: u128 = if self.size { 300 } else { 800 };
        let plain = |one: &Cluster| one.tests.is_empty() && one.parts.is_empty();
        let mut out = Vec::new();
        let mut run: Vec<Cluster> = Vec::new();
        for one in clusters {
            if plain(&one) {
                run.push(one);
            } else {
                out.extend(tabled_runs(std::mem::take(&mut run), threshold, ratio));
                out.push(one);
            }
        }
        out.extend(tabled_runs(run, threshold, ratio));
        out
    }

    /// `jump_table_cluster::emit`: the index in the pointer's width, and a
    /// `switch` the backend makes a table jump of; each index goes to a block
    /// that only jumps on, so the edges are not critical.
    fn table(
        &mut self,
        mut block: BlockId,
        cluster: &Cluster,
        entry: bool,
    ) {
        let default = self.switch.default;
        let index = self.offset(block, cluster.low);
        let span = cluster.high - cluster.low;
        if entry {
            let limit = Operand::Constant(self.unit.context.int(self.switch.ty, span as i128));
            let outside = self.compare(block, IntPredicate::Ugt, index, limit);
            let next = self.fresh(block);
            self.branch(block, outside, default, next);
            block = next;
        }
        let (pointer, bits) = (self.pointer, self.switch.bits);
        let wide = self.unit.context.types.int(pointer);
        let index = match bits.cmp(&pointer) {
            std::cmp::Ordering::Less => self.put(block, Opcode::Cast(CastOp::ZExt), wide, vec![index]),
            std::cmp::Ordering::Greater => self.put(block, Opcode::Cast(CastOp::Trunc), wide, vec![index]),
            std::cmp::Ordering::Equal => index,
        };
        let mut landings: IdMap<BlockId, BlockId> = IdMap::new();
        let mut lands: Vec<BlockId> = Vec::new();
        let mut operands = vec![index, Operand::Block(default)];
        // The parts are in order and apart: the one that holds an index is at
        // or after the one that held the index before.
        let mut part = 0;
        for at in 0..=span {
            while cluster.parts.get(part).is_some_and(|&(_, high, _)| high - cluster.low < at) {
                part += 1;
            }
            let target = cluster
                .parts
                .get(part)
                .filter(|&&(low, _, _)| low - cluster.low <= at)
                .map_or(default, |&(_, _, target)| target);
            let land = match landings.get(&target) {
                Some(&land) => land,
                None => {
                    let land = self.fresh(block);
                    self.jump(land, target);
                    landings.insert(target, land);
                    lands.push(land);
                    land
                }
            };
            operands.extend([Operand::Constant(self.unit.context.int(wide, at as i128)), Operand::Block(land)]);
        }
        // The default is never taken, the index being in range: any case's.
        operands[1] = operands[3];
        let void = self.unit.context.types.void();
        let inst = self.unit.function.create_instruction(Opcode::Switch, void, operands, Flags::TABLE, None);
        self.unit.function.insert(inst, Position::End(block)).expect("a placed instruction");
        self.edges.extend(lands.iter().map(|&land| (block, land)));
    }

    /// `find_bit_tests`: the fewest runs of clusters, each narrower than a
    /// word with at most 3 targets, that a bit test can take; the ones that
    /// gain by it become one cluster.
    fn with_bit_tests(
        &self,
        clusters: Vec<Cluster>,
    ) -> Vec<Cluster> {
        let word = u128::from(self.word);
        let count = clusters.len();
        let targets = |run: &[Cluster]| {
            let mut all: Vec<BlockId> = Vec::new();
            for one in run {
                if !all.contains(&one.target) {
                    all.push(one.target);
                }
            }
            all
        };
        let mut best = vec![(usize::MAX, 0usize); count + 1];
        best[0] = (0, 0);
        for end in 1..=count {
            for start in (0..end).rev() {
                let run = &clusters[start..end];
                if start + 1 != end
                    && (run[run.len() - 1].high - run[0].low + 1 >= word || targets(run).len() > MOST_TARGETS)
                {
                    break;
                }
                if best[start].0 + 1 <= best[end].0 {
                    best[end] = (best[start].0 + 1, start);
                }
            }
        }
        let mut out = Vec::new();
        let mut end = count;
        while end > 0 {
            let start = best[end].1;
            let run = &clusters[start..end];
            let unique = targets(run);
            if beneficial(run.len(), unique.len()) && start + 1 != end {
                let low = run[0].low;
                let mut tests: Vec<(u128, BlockId)> = unique.iter().map(|&target| (0, target)).collect();
                for one in run {
                    let at = unique.iter().position(|&target| target == one.target).expect("a target");
                    tests[at].0 |= (one.low - low..=one.high - low).fold(0u128, |all, bit| all | 1 << bit);
                }
                tests.sort_by_key(|&(bits, _)| std::cmp::Reverse(bits.count_ones()));
                out.push(Cluster { low, high: run[run.len() - 1].high, target: unique[0], tests, parts: Vec::new() });
            } else {
                out.extend(run.iter().rev().cloned());
            }
            end = start;
        }
        out.reverse();
        out
    }

    /// The phis of the switch's targets, which named the switch's block once
    /// for each of its edges, name the blocks that jump to them now once for
    /// each of theirs.
    fn fixed(
        &mut self,
        originals: &[BlockId],
    ) {
        let block = self.switch.block;
        let mut reached_from: IdMap<BlockId, Vec<BlockId>> = IdMap::new();
        for &(from, to) in &self.edges {
            match reached_from.get_mut(&to) {
                Some(sources) => sources.push(from),
                None => {
                    reached_from.insert(to, vec![from]);
                }
            }
        }
        for &target in originals {
            let sources = reached_from.get(&target).map_or(&[][..], Vec::as_slice);
            for phi in edges::phis(self.unit.function, target) {
                let operands = self.unit.function.instruction(phi).operands.clone();
                let mut out = Vec::new();
                let mut replaced = false;
                for pair in operands.chunks(2) {
                    if pair[1] != Operand::Block(block) {
                        out.extend_from_slice(pair);
                    } else if !replaced {
                        replaced = true;
                        out.extend(sources.iter().flat_map(|&from| [pair[0], Operand::Block(from)]));
                    }
                }
                self.unit.function.set_operands(phi, out);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Order {
    Equal,
    Greater,
    AtLeast,
    Less,
}

/// The narrowest integer a table of `cells` can hold for a `bits`-wide value,
/// and how it is widened: zero extended where the cells fit unsigned,
/// otherwise sign extended.
fn cell_width(
    cells: &[u128],
    bits: u32,
) -> (u32, CastOp) {
    for width in [8, 16, 32] {
        if width >= bits {
            break;
        }
        if cells.iter().all(|&one| one <= mask(width)) {
            return (width, CastOp::ZExt);
        }
        if cells.iter().all(|&one| (signed(one, bits) as i128) == signed(one & mask(width), width)) {
            return (width, CastOp::SExt);
        }
    }
    (bits, CastOp::ZExt)
}

thread_local! {
    /// Partitions `tabled_runs` has tried, for a test of its slope.
    static TRIED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `jump_table_cluster::find_jump_tables` over `run`, clusters in order.
fn tabled_runs(
    run: Vec<Cluster>,
    threshold: usize,
    ratio: u128,
) -> Vec<Cluster> {
    let count = run.len();
    let comparisons = |one: &Cluster| 1 + u128::from(one.high != one.low);
    // The fewest clusters up to each, where the last starts, and the cases
    // left out of tables on the way.
    let mut best = vec![(usize::MAX, 0usize, usize::MAX); count + 1];
    best[0] = (0, 0, 0);
    // `compared[i]` is the comparisons of the clusters before `i`.
    let compared: Vec<u128> = std::iter::once(0)
        .chain(run.iter().scan(0, |sum, one| {
            *sum += comparisons(one);
            Some(*sum)
        }))
        .collect();
    let dense = |start: usize, end: usize| {
        start + 1 == end || 100 * (run[end - 1].high - run[start].low + 1) <= ratio * (compared[end] - compared[start])
    };
    // MIRROR `findJumpTables`, "Cheap case"
    // (https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SwitchLoweringUtils.cpp#L83-L96):
    // a run dense as a whole is one table, the fewest partitions there are,
    // and the quadratic search below is skipped.
    // DEVIATE 1: density is gcc's span against comparisons, not the pin's
    // `isSuitableForJumpTable` percentage, as this pass lowers as gcc 13.4.
    if count > 0 && dense(0, count) {
        best[count] = (1, 0, if count < threshold { count } else { 0 });
    } else {
        for end in 1..=count {
            for start in 0..end {
                TRIED.with(|tried| tried.set(tried.get() + 1));
                let left = best[start].2 + if end - start < threshold { end - start } else { 0 };
                let (fewer, tied) = (best[start].0 + 1 < best[end].0, best[start].0 + 1 == best[end].0);
                if (fewer || (tied && left < best[end].2)) && dense(start, end) {
                    best[end] = (best[start].0 + 1, start, left);
                }
            }
        }
    }
    if best[count].0 == count {
        return run;
    }
    let mut out = Vec::new();
    let mut end = count;
    while end > 0 {
        let start = best[end].1;
        let one = &run[start..end];
        if one.len() > 1 && one.len() >= threshold {
            let parts = one.iter().map(|case| (case.low, case.high, case.target)).collect();
            out.push(Cluster {
                low: one[0].low,
                high: one[one.len() - 1].high,
                target: one[0].target,
                tests: Vec::new(),
                parts,
            });
        } else {
            out.extend(one.iter().rev().cloned());
        }
        end = start;
    }
    out.reverse();
    out
}

/// `bit_test_cluster::is_beneficial`.
fn beneficial(
    count: usize,
    unique: usize,
) -> bool {
    matches!((unique, count), (1, 3..) | (2, 5..) | (3, 6..))
}

#[cfg(test)]
#[path = "switchlower_tests.rs"]
mod tests;
