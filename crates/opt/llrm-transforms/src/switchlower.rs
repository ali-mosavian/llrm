//! A switch lowered as LLVM's SelectionDAG lowers one: the likeliest range
//! peeled off, jump tables and bit tests for what is dense, and a balanced
//! tree of range tests over the rest. Switch conversion (lookup tables) is the
//! `switchconv` pass.
//!
//! # How it works
//! `visitSwitch` ([`switchcg`](crate::switchcg) holds the decisions, a
//! translation of `SwitchLoweringUtils.cpp`): cases to one target that follow
//! one another are one range, values ordered signed; `peelDominantCaseCluster`
//! takes a range of at least 66% out ahead; `findJumpTables` and
//! `findBitTestClusters` replace runs by tables and bit tests.
//!
//! `splitWorkItem` splits more than three clusters at the pivot that balances
//! their probability, `value < pivot` to the one side, the other to the rest;
//! at most three are `lowerWorkItem`: the likeliest first, each falling to the
//! next and the last to the default, two values one bit apart one `(x | bit)
//! == both` compare, a range `x - low <= span`.
//!
//! A jump table is the range test of `x - low` and a `switch` over the indices
//! `0..n`, every index a block that only jumps on; the backend makes it a jump
//! through a table of code addresses. Bit tests are `1 << (x - low)` against
//! one mask per target (`visitBitTestCase`: a single bit or all but one is a
//! compare). Where the default is `unreachable` the range test goes.
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
//! The deviations (probabilities, cases to the default) are in
//! [`switchcg`](crate::switchcg).
//!
//! # Runs
//! As two passes: `switchconv` (gcc's `pass_convert_switch`, -O2 and up),
//! which makes the lookup tables and so adds loads, and last of the function
//! passes `switchlower` (`pass_lower_switch`), every level that optimizes,
//! which adds no memory operation: the module analyses frozen at the end of the
//! pipeline stand past it.
//!
//! DEVIATE LP-6 (no mid-end pin pass: SelectionDAGBuilder lowers a switch at
//! isel (`visitSwitch`), here MIR is lowered before selection (CG1); exit: isel
//! lowers switches and this is deleted; owner w-6; review 2026-11-20).
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
//! After, the first three one range, tested first as the likelier:
//!
//! ```mir
//! b0:
//!   %t = icmp ule i32 %k, 2
//!   br i1 %t, label %a, label %m
//! m:
//!   %u = icmp eq i32 %k, 40
//!   br i1 %u, label %b, label %d
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
use crate::switchcg::{
    BitTests, CaseCluster, Kind, Probability, Split, SwitchTarget, WorkItem, find_bit_test_clusters, find_jump_tables,
    peel_dominant_case_cluster, scale_case_probability, sort_and_rangeify, split_work_item,
};

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
                fallthrough_unreachable: false,
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

/// One case label after grouping, for switch conversion: the values
/// `low..=high` in the keys of the order (`Lowering::flip`).
#[derive(Clone)]
struct Cluster {
    low: u128,
    high: u128,
    target: BlockId,
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
    /// The default is `unreachable`: its last test is dropped.
    fallthrough_unreachable: bool,
    /// The edges the lowering made, source and target, for the targets' phis.
    edges: Vec<(BlockId, BlockId)>,
}

/// `SwitchWorkListItem`: the clusters `first..=last` to lower in `block`,
/// knowing the value is at least `floor` and below `ceiling`.
struct Item {
    block: BlockId,
    first: usize,
    last: usize,
    floor: Option<i128>,
    ceiling: Option<i128>,
    default_probability: Probability,
}

impl Lowering<'_, '_> {
    /// Whether the switch was lowered: `convert` makes only the lookup table
    /// and leaves any other switch be.
    fn run(
        &mut self,
        convert: bool,
    ) -> bool {
        if !convert {
            return self.clustered();
        }
        let sign = 1u128 << (self.switch.bits - 1);
        let grouped = |flip: u128| {
            let mut cases: Vec<(u128, BlockId)> =
                self.switch.cases.iter().map(|&(value, target)| (value ^ flip, target)).collect();
            cases.sort();
            let mut clusters: Vec<Cluster> = Vec::new();
            for (key, target) in cases {
                match clusters.last_mut() {
                    Some(last) if last.target == target && last.high + 1 == key => last.high = key,
                    _ => clusters.push(Cluster { low: key, high: key, target }),
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
        !clusters.is_empty() && self.converted(&clusters)
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
            let mut body = function.iter_block(one);
            let (Some(only), None) = (body.next(), body.next()) else { return None };
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

    /// `SelectionDAGBuilder::visitSwitch`: `switchcg`'s clusters, the likeliest
    /// peeled off, the rest a balanced tree of work items each lowered as
    /// `lowerWorkItem`. DEVIATE S4: a case to the default is dropped before
    /// (`switch_at`), as LLVM's SimplifyCFG leaves none.
    fn clustered(&mut self) -> bool {
        let (bits, block, default) = (self.switch.bits, self.switch.block, self.switch.default);
        let probability = Probability::new(1, self.switch.cases.len() as u32 + 1);
        let mut clusters: Vec<CaseCluster> = (self.switch.cases.iter())
            .map(|&(value, target)| CaseCluster::range(signed(value, bits), signed(value, bits), target, probability))
            .collect();
        sort_and_rangeify(&mut clusters);
        let originals = self.originals();
        self.fallthrough_unreachable = self.default_is_unreachable();
        let target = SwitchTarget {
            jump_tables: self.threshold != 0,
            optimize_for_size: self.size,
            ..SwitchTarget::x86(self.pointer)
        };
        self.unit.function.erase(self.switch.inst).expect("a terminator defines nothing");
        let mut root = block;
        let mut default_probability = probability;
        if let Some((peeled, top)) = peel_dominant_case_cluster(&mut clusters, &target) {
            root = self.fresh(block);
            self.range_test(block, &peeled, root, false);
            default_probability = scale_case_probability(default_probability, top);
        }
        if clusters.is_empty() {
            self.jump(root, default);
        } else {
            find_jump_tables(&mut clusters, &target, default);
            find_bit_test_clusters(&mut clusters, &target);
            let last = clusters.len() - 1;
            let mut work = vec![Item { block: root, first: 0, last, floor: None, ceiling: None, default_probability }];
            while let Some(item) = work.pop() {
                if item.last - item.first + 1 > 3 && target.optimizing && !target.minimize_size {
                    self.split(&mut work, &item, &clusters);
                } else {
                    self.lower_item(&item, &mut clusters);
                }
            }
        }
        self.fixed(&originals);
        true
    }

    /// The default's first instruction, phis aside, is `unreachable`.
    fn default_is_unreachable(&self) -> bool {
        let function = &*self.unit.function;
        let mut instructions = function.iter_block(self.switch.default).map(|one| function.instruction(one));
        instructions.find(|one| one.opcode != Opcode::Phi).is_some_and(|one| one.opcode == Opcode::Unreachable)
    }

    /// `splitWorkItem`: the clusters of `item` as two items on either side of
    /// the pivot, the first on the right, tested by `value < pivot`.
    fn split(
        &mut self,
        work: &mut Vec<Item>,
        item: &Item,
        clusters: &[CaseCluster],
    ) {
        let plain = WorkItem { first: item.first, last: item.last, default_probability: item.default_probability };
        let Split { last_left, first_right, .. } = split_work_item(clusters, &plain);
        let pivot = clusters[first_right].low;
        let half = item.default_probability.divided(2);
        // A side of one range that fills what the bounds leave it goes straight
        // to its target.
        let (only_left, only_right) = (&clusters[item.first], &clusters[item.last]);
        let left = if item.first == last_left
            && only_left.is_range()
            && item.floor == Some(only_left.low)
            && only_left.high + 1 == pivot
        {
            only_left.target
        } else {
            let left = self.fresh(item.block);
            let (first, last, floor) = (item.first, last_left, item.floor);
            work.push(Item { block: left, first, last, floor, ceiling: Some(pivot), default_probability: half });
            left
        };
        let right = if first_right == item.last
            && only_right.is_range()
            && item.ceiling.is_some_and(|ceiling| only_right.high + 1 == ceiling)
        {
            only_right.target
        } else {
            let right = self.fresh(if left == item.block { item.block } else { left });
            let (first, last, ceiling) = (first_right, item.last, item.ceiling);
            work.push(Item { block: right, first, last, floor: Some(pivot), ceiling, default_probability: half });
            right
        };
        let value = self.switch.value;
        let pivot = Operand::Constant(self.unit.context.int(self.switch.ty, pivot));
        let below = self.compare(item.block, IntPredicate::Slt, value, pivot);
        self.branch(item.block, below, left, right);
    }

    /// `lowerWorkItem`: the clusters in turn, the likeliest first, each falling
    /// through to the next and the last to the default.
    fn lower_item(
        &mut self,
        item: &Item,
        clusters: &mut [CaseCluster],
    ) {
        let default = self.switch.default;
        if item.last - item.first == 1 && item.block == self.switch.block && self.one_bit_apart(item, clusters) {
            return;
        }
        let range = &mut clusters[item.first..=item.last];
        // A tie in probability is broken by the value.
        range.sort_by(|a, b| b.probability.cmp(&a.probability).then(a.low.cmp(&b.low)));
        // The last cluster falls through if one that ranks no lower is the
        // block that follows.
        let following = self.following(item.block);
        let end = range.len() - 1;
        for at in (0..end).rev() {
            if range[at].probability > range[end].probability {
                break;
            }
            if range[at].is_range() && Some(range[at].target) == following {
                range.swap(at, end);
                break;
            }
        }
        let mut current = item.block;
        let mut placed = item.block;
        for (at, cluster) in range.iter().enumerate() {
            let last = at == end;
            let (next, unreachable) = if last {
                (default, self.fallthrough_unreachable)
            } else {
                placed = self.fresh(placed);
                (placed, false)
            };
            match &cluster.kind {
                Kind::JumpTable { table } => self.jump_table(current, cluster, table, next, unreachable, &mut placed),
                Kind::BitTests(tests) => self.bit_tests(current, tests, next, unreachable, &mut placed),
                Kind::Range => self.range_test(current, cluster, next, unreachable),
            }
            current = next;
        }
    }

    /// The block after `block` in the layout.
    fn following(
        &self,
        block: BlockId,
    ) -> Option<BlockId> {
        let layout = self.unit.function.layout();
        layout.iter().position(|&one| one == block).and_then(|at| layout.get(at + 1)).copied()
    }

    /// `if (X == 6 || X == 4)` is `if ((X | 2) == 6)`: two single values to one
    /// target that differ in one bit.
    fn one_bit_apart(
        &mut self,
        item: &Item,
        clusters: &[CaseCluster],
    ) -> bool {
        let (small, big) = (&clusters[item.first], &clusters[item.last]);
        let width = mask(self.switch.bits);
        let (small_value, big_value) = (small.low as u128 & width, big.low as u128 & width);
        let common = big_value ^ small_value;
        if small.low != small.high || big.low != big.high || small.target != big.target || !common.is_power_of_two() {
            return false;
        }
        let ty = self.switch.ty;
        let bit = Operand::Constant(self.unit.context.int(ty, common as i128));
        let both = Operand::Constant(self.unit.context.int(ty, (big_value | small_value) as i128));
        let merged = self.put(item.block, Opcode::Binary(BinaryOp::Or), ty, vec![self.switch.value, bit]);
        let equal = self.compare(item.block, IntPredicate::Eq, merged, both);
        self.branch(item.block, equal, small.target, self.switch.default);
        true
    }

    /// `value - low`, in the switch's type.
    fn minus(
        &mut self,
        block: BlockId,
        low: i128,
    ) -> Operand {
        if low == 0 {
            return self.switch.value;
        }
        let low = Operand::Constant(self.unit.context.int(self.switch.ty, low));
        self.put(block, Opcode::Binary(BinaryOp::Sub), self.switch.ty, vec![self.switch.value, low])
    }

    /// `index` in the pointer's width, as the jump table's register is.
    fn as_pointer(
        &mut self,
        block: BlockId,
        index: Operand,
    ) -> Operand {
        let wide = self.unit.context.types.int(self.pointer);
        self.resized(block, index, wide, self.switch.bits.cmp(&self.pointer))
    }

    fn resized(
        &mut self,
        block: BlockId,
        value: Operand,
        wide: TypeId,
        from: std::cmp::Ordering,
    ) -> Operand {
        match from {
            std::cmp::Ordering::Less => self.put(block, Opcode::Cast(CastOp::ZExt), wide, vec![value]),
            std::cmp::Ordering::Greater => self.put(block, Opcode::Cast(CastOp::Trunc), wide, vec![value]),
            std::cmp::Ordering::Equal => value,
        }
    }

    /// `visitSwitchCase` for a range: `low <= value <= high` to the cluster's
    /// target, else `next`; no test where `next` is unreachable.
    fn range_test(
        &mut self,
        block: BlockId,
        cluster: &CaseCluster,
        next: BlockId,
        unreachable: bool,
    ) {
        if unreachable || cluster.target == next {
            return self.jump(block, cluster.target);
        }
        let ty = self.switch.ty;
        let value = self.switch.value;
        let condition = if cluster.low == cluster.high {
            let key = Operand::Constant(self.unit.context.int(ty, cluster.low));
            self.compare(block, IntPredicate::Eq, value, key)
        } else if cluster.low == -(1i128 << (self.switch.bits - 1)) {
            let high = Operand::Constant(self.unit.context.int(ty, cluster.high));
            self.compare(block, IntPredicate::Sle, value, high)
        } else {
            let offset = self.minus(block, cluster.low);
            let span = Operand::Constant(self.unit.context.int(ty, cluster.high - cluster.low));
            self.compare(block, IntPredicate::Ule, offset, span)
        };
        self.branch(block, condition, cluster.target, next);
    }

    /// `visitJumpTableHeader` and `visitJumpTable`: a range test of
    /// `value - low` against `next`, then a `switch` in a block of its own that
    /// the backend makes a table jump of. Each index goes to a block that only
    /// jumps on, so the edges are not critical.
    fn jump_table(
        &mut self,
        block: BlockId,
        cluster: &CaseCluster,
        table: &[BlockId],
        next: BlockId,
        unreachable: bool,
        placed: &mut BlockId,
    ) {
        let default = self.switch.default;
        let offset = self.minus(block, cluster.low);
        let index = self.as_pointer(block, offset);
        let jump = self.fresh(*placed);
        *placed = jump;
        if unreachable {
            self.jump(block, jump);
        } else {
            let span = Operand::Constant(self.unit.context.int(self.switch.ty, cluster.high - cluster.low));
            let outside = self.compare(block, IntPredicate::Ugt, offset, span);
            self.branch(block, outside, next, jump);
        }
        let wide = self.unit.context.types.int(self.pointer);
        let mut landings: IdMap<BlockId, BlockId> = IdMap::new();
        let mut lands: Vec<BlockId> = Vec::new();
        let mut operands = vec![index, Operand::Block(default)];
        for (at, &target) in table.iter().enumerate() {
            let land = match landings.get(&target) {
                Some(&land) => land,
                None => {
                    let land = self.fresh(*placed);
                    *placed = land;
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
        self.unit.function.insert(inst, Position::End(jump)).expect("a placed instruction");
        self.edges.extend(lands.iter().map(|&land| (jump, land)));
    }

    /// `visitBitTestHeader` and `visitBitTestCase`: a range test of
    /// `value - low_bound` against `next`, then one test per target, the last
    /// dropped where the range test has proved it.
    fn bit_tests(
        &mut self,
        block: BlockId,
        tests: &BitTests,
        next: BlockId,
        unreachable: bool,
        placed: &mut BlockId,
    ) {
        let bits = self.switch.bits;
        let offset = self.minus(block, tests.low_bound);
        // The register is the switch's type, the pointer's where that is not
        // legal or a mask does not fit it.
        let fits = tests.cases.iter().all(|case| u128::from(case.mask) <= mask(bits));
        let (register, wide) = if bits <= self.word && fits {
            (offset, self.switch.ty)
        } else {
            let wide = self.unit.context.types.int(self.pointer);
            (self.as_pointer(block, offset), wide)
        };
        let leave_last = (tests.contiguous || unreachable) && tests.cases.len() >= 2;
        let emitted = tests.cases.len() - usize::from(leave_last);
        let blocks: Vec<BlockId> = (0..emitted)
            .map(|_| {
                *placed = self.fresh(*placed);
                *placed
            })
            .collect();
        if unreachable {
            self.jump(block, blocks[0]);
        } else {
            let range = Operand::Constant(self.unit.context.int(self.switch.ty, tests.range));
            let outside = self.compare(block, IntPredicate::Ugt, offset, range);
            self.branch(block, outside, next, blocks[0]);
        }
        for (at, case) in tests.cases[..emitted].iter().enumerate() {
            let from = blocks[at];
            let after = if leave_last && at + 2 == tests.cases.len() {
                tests.cases[at + 1].target
            } else if at + 1 == tests.cases.len() {
                next
            } else {
                blocks[at + 1]
            };
            let ones = case.mask.count_ones();
            let condition = if ones == 1 {
                let bit = Operand::Constant(self.unit.context.int(wide, i128::from(case.mask.trailing_zeros())));
                self.compare(from, IntPredicate::Eq, register, bit)
            } else if i128::from(ones) == tests.range {
                let hole = Operand::Constant(self.unit.context.int(wide, i128::from(case.mask.trailing_ones())));
                self.compare(from, IntPredicate::Ne, register, hole)
            } else {
                let one = Operand::Constant(self.unit.context.int(wide, 1));
                let bit = self.put(from, Opcode::Binary(BinaryOp::Shl), wide, vec![one, register]);
                let tested = Operand::Constant(self.unit.context.int(wide, case.mask as i128));
                let both = self.put(from, Opcode::Binary(BinaryOp::And), wide, vec![bit, tested]);
                let zero = Operand::Constant(self.unit.context.int(wide, 0));
                self.compare(from, IntPredicate::Ne, both, zero)
            };
            if case.target == after {
                self.jump(from, case.target);
            } else {
                self.branch(from, condition, case.target, after);
            }
        }
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
