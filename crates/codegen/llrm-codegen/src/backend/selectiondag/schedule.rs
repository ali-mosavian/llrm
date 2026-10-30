//! The list schedulers of `ScheduleDAGRRList.cpp`: the `SUnit` graph
//! (`ScheduleDAGSDNodes`), `ScheduleDAGTopologicalSort`, and
//! `ScheduleDAGRRList` with the queue the target's [`Preference`] names:
//! `BURegReductionPriorityQueue`, the source-order queue (X86, AArch64) or the
//! ILP queue (M68k).
//!
//! # How it works
//! [`schedule`] groups the selected DAG's nodes into units (a node and
//! everything glued to it is one unit), adds an edge per data or chain
//! operand, numbers the units Sethi-Ullman, then schedules from the root
//! upward: the available unit that `bu_ls_rr_sort` ranks highest goes next,
//! and the order is reversed at the end. Latencies are one (zero for a
//! `TokenFactor`), as `createBURRListDAGScheduler` builds it
//! (`needlatency = false`), and register pressure is not tracked
//! (`tracksrp = false`).
//!
//! ```text
//! store2 ──chain──▶ store1        bottom-up picks store2, then store1,
//!   │                  │          then the adds they use, deepest last;
//!  add ──▶ load      add         the reversed list is the emit order.
//! ```
//!
//! # Example
//! `x = a + b; y = x * c` with both results stored (chain between the
//! stores) comes out as load a, load b, add, load c, mul, store x, store y
//! in one valid order; the stores keep their chain order whatever the
//! priorities say.
//!
//! # Runs
//! Nothing runs it yet: the emitter (D1b) takes [`schedule`]'s units. The
//! old selector stays the default; the DAG path is `--isel=dag` (shadow).
//!
//! The ILP queue (`ilp_ls_rr_sort`) tracks which definitions a scheduled
//! unit has used (`NumRegDefsLeft`) and puts the unit that adds the least
//! register pressure first, then the critical path, then `BURRSort`. A
//! target gives the call-frame opcodes ([`SchedInfo::call_frame_setup`]) so that
//! the units of one call sequence stay together: without them two calls'
//! stack arguments interleave.
//!
//! DEVIATE SD32: the register pressure per class is not kept. Reason: the
//! pressure functions compare it with `getRegPressureLimit`, 0 for M68k, the
//! only target on the ILP queue. Exit: a target with a limit asks for
//! [`Preference::Ilp`]. Owner: m68k-spike.
//!
//! DEVIATE SD30: part translated. Physical-register live interference
//! (`DelayForLiveRegsBottomUp`, `CheckForLiveRegDef`, `LiveRegDefs`,
//! `Interferences`, `releaseInterferences`) and the call-frame resource are
//! in; not translated:
//! `InsertCopiesAndMoveSuccs` (when no clone is possible the first held
//! unit goes anyway), `ClusterNodes`, and the
//! virtual-register cycle check (`initVRegCycle`, only for blocks that branch
//! to themselves). Reason: they need register classes of the matcher's output
//! for copies. Exit: before the switch gate. Owner: sdag-d. Review
//! 2026-12-03. Hits (vsgcc corpus, `--isel=dag`, m32 and m16, O1/O2/Os,
//! 2026-10-27): the held unit going anyway 0, the clone 0, `BacktrackBottomUp`
//! 15; the comparator (`BURRSort`, `checkSpecialNodes`, `BUCompareLatency`
//! without the virtual-register cycle, `calcMaxScratches`, `closestSucc`) is
//! the pin's.
//!
//! DEVIATE SD31: no IR order (`SDNode::getIROrder`), so the call-ordering tie
//! break in `BURRSort` never fires. Reason: SD10 drops `SDLoc` and IR order.
//! Exit: with the emitter, if line information needs them. Owner: sdag-d.
//! Review 2026-12-03.
//!
//! # See also
//! `ScheduleDAGRRList.cpp`
//! ([createBURRListDAGScheduler
//! L3082](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGRRList.cpp#L3082),
//! [ListScheduleBottomUp
//! L1548](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGRRList.cpp#L1548),
//! [BURRSort
//! L2504](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGRRList.cpp#L2504)),
//! `ScheduleDAGSDNodes::BuildSchedGraph`
//! ([L536](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/ScheduleDAGSDNodes.cpp#L536)),
//! `ScheduleDAG.cpp` (`SUnit::addPred`, `ScheduleDAGTopologicalSort`).

use llrm_support::hash::HashMap;

use super::dag::{NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;

/// What the scheduler asks of the target's instruction table (`TII->get`).
pub trait SchedInfo {
    /// The pin's number of the physical register `id` names; x86's by default.
    fn pin_register(
        &self,
        id: llrm_lir::registers::RegId,
    ) -> Option<u16> {
        super::dagisel_adapter::pin_register(id)
    }
    /// `MCInstrDesc::getNumDefs`.
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize;
    /// `MCInstrDesc::implicit_defs`, as register numbers.
    fn implicit_defs(
        &self,
        opcode: u32,
    ) -> &[u16];
    /// `MCInstrDesc::isCall`.
    fn is_call(
        &self,
        opcode: u32,
    ) -> bool;
    /// Whether the target runs the pin's source-order queue by default.
    fn source_order(&self) -> bool {
        false
    }
    /// Which of the pin's DAG schedulers the target gets: `createDefaultScheduler`
    /// picks `ILP` for a target that does not enable the machine scheduler
    /// (M68k), `Source` for one that does (X86, AArch64).
    fn preference(&self) -> Preference {
        if self.source_order() { Preference::Source } else { Preference::Burr }
    }
    /// `TRI->getCrossCopyRegClass(RC) != RC` for the register's class: the
    /// value cannot be copied within its class (x86's EFLAGS).
    fn has_cross_copy_class(
        &self,
        _reg: u16,
    ) -> bool {
        false
    }
    /// `TII->getCallFrameSetupOpcode`: where a call sequence begins. None for
    /// a target whose sequences the scheduler may interleave.
    fn call_frame_setup(&self) -> Option<u32> {
        None
    }
    /// `TII->getCallFrameDestroyOpcode`.
    fn call_frame_destroy(&self) -> Option<u32> {
        None
    }
    /// DEVIATE: the register a copy of a virtual register may clobber, as the
    /// move it can become after allocation does (M68k's `MOVE` sets CCR). A
    /// copy is then held back from between a flag-setter and its reader, where
    /// the pin leaves it and the flags are saved around it. None: copies
    /// clobber nothing.
    fn copy_clobbers(&self) -> Option<u16> {
        None
    }
    /// `TRI->regsOverlap`.
    fn regs_overlap(
        &self,
        a: u16,
        b: u16,
    ) -> bool {
        a == b
    }
}

/// The list scheduler a target asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    /// `createBURRListDAGScheduler`: `bu_ls_rr_sort`.
    Burr,
    /// `createSourceListDAGScheduler`: `src_ls_rr_sort`, IR order first.
    Source,
    /// `createILPListDAGScheduler`: `ilp_ls_rr_sort`, with register pressure
    /// and latencies tracked.
    Ilp,
}

/// One scheduled unit: its nodes, the bottom-most (`getNode()`) first, then
/// each node glued above it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub nodes: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Data,
    /// `SDep::Barrier`, a chain edge.
    Barrier,
    /// `SDep::Artificial`, an edge the scheduler added itself.
    Artificial,
}

/// `SDep`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dep {
    su: usize,
    kind: Kind,
    latency: u32,
    /// `SDep::getReg`: the physical register the edge carries (an assigned
    /// register dependence), by the pin's register number.
    reg: Option<u16>,
}

impl Dep {
    fn is_ctrl(self) -> bool {
        self.kind != Kind::Data
    }

    /// `SDep::overlaps`: the same unit by the same kind of edge.
    fn overlaps(
        self,
        other: Dep,
    ) -> bool {
        self.su == other.su && self.kind == other.kind && self.reg == other.reg
    }

    /// `SDep::isAssignedRegDep`.
    fn is_assigned_reg(self) -> bool {
        self.kind == Kind::Data && self.reg.is_some()
    }
}

#[derive(Clone, Debug)]
struct SUnit {
    nodes: Vec<u32>,
    preds: Vec<Dep>,
    succs: Vec<Dep>,
    num_preds: u32,
    num_succs: u32,
    num_preds_left: u32,
    num_succs_left: u32,
    /// The register definitions of the unit's nodes (`RegDefIter`).
    reg_defs: u32,
    /// `NumRegDefsLeft`: those no scheduled unit has used.
    num_reg_defs_left: u32,
    latency: u32,
    depth: u32,
    height: u32,
    depth_current: bool,
    height_current: bool,
    is_available: bool,
    is_scheduled: bool,
    queue_id: u32,
    is_call: bool,
    is_call_op: bool,
    is_schedule_low: bool,
    has_phys_reg_defs: bool,
    has_phys_reg_clobbers: bool,
}

impl SUnit {
    fn new(node: u32) -> SUnit {
        SUnit {
            nodes: vec![node],
            preds: Vec::new(),
            succs: Vec::new(),
            num_preds: 0,
            num_succs: 0,
            num_preds_left: 0,
            num_succs_left: 0,
            reg_defs: 0,
            num_reg_defs_left: 0,
            latency: 1,
            depth: 0,
            height: 0,
            depth_current: false,
            height_current: false,
            is_available: false,
            is_scheduled: false,
            queue_id: 0,
            is_call: false,
            is_call_op: false,
            is_schedule_low: false,
            has_phys_reg_defs: false,
            has_phys_reg_clobbers: false,
        }
    }

    /// `getNode()`: the bottom-most node of the glued sequence.
    fn node(&self) -> u32 {
        self.nodes[0]
    }
}

/// `isPassiveNode`: a non-scheduled leaf.
fn is_passive(opcode: ISD) -> bool {
    matches!(
        opcode,
        ISD::Constant
            | ISD::TargetConstant
            | ISD::ConstantFP
            | ISD::TargetConstantFP
            | ISD::Register
            | ISD::GlobalAddress
            | ISD::TargetGlobalAddress
            | ISD::ExternalSymbol
            | ISD::FrameIndex
            | ISD::ConstantPool
            | ISD::TargetConstantPool
            | ISD::TargetFrameIndex
            | ISD::TargetExternalSymbolNode
            | ISD::JumpTable
            | ISD::TargetJumpTable
            | ISD::RegisterMask
            | ISD::BasicBlock
            | ISD::EntryToken
    )
}

/// The units of `dag` reachable from `root` in schedule order, the pin's
/// `ScheduleDAGSDNodes::Run` with the BURR queue.
pub fn schedule(
    dag: &SelectionDAG,
    root: SDValue,
    info: &dyn SchedInfo,
) -> Vec<Unit> {
    let mut graph = Graph::build(dag, root, info);
    let mut topo = Topo::new(&graph.sus);
    topo.dirty = true;
    let mut queue = Queue::default();
    queue.init(&mut graph, &mut topo);
    let order = graph.list_schedule_bottom_up(root, &mut queue);
    order.into_iter().map(|su| Unit { nodes: std::mem::take(&mut graph.sus[su].nodes) }).collect()
}

struct Graph<'a> {
    dag: &'a SelectionDAG,
    info: &'a dyn SchedInfo,
    sus: Vec<SUnit>,
    /// `SDNode::getNodeId`: the unit a node is in; -1 for none.
    node_id: Vec<i32>,
    /// The live users of each node (`N->users()`): node `n`'s are
    /// `user_list[user_start[n]..user_start[n + 1]]`.
    user_start: Vec<u32>,
    user_list: Vec<u32>,
    cur_cycle: u32,
    /// `LiveRegDefs`: the unit whose definition of a physical register is
    /// still wanted by a unit already scheduled below it.
    live_reg_defs: HashMap<u16, usize>,
    /// `LiveRegGens`: the first scheduled unit that wants it.
    live_reg_gens: HashMap<u16, usize>,
    /// `CallSeqEndForStart`: the end of the call sequence a unit begins.
    call_seq_end_for_start: HashMap<usize, usize>,
    /// `Interferences`: available units held back for a live register, with
    /// the registers (`LRegsMap`).
    interferences: Vec<(usize, Vec<u16>)>,
    /// `Sequence`: the units scheduled so far, bottom first.
    sequence: Vec<usize>,
    /// The work stack of the depth and height walks, kept between calls.
    walk: Vec<usize>,
}

impl<'a> Graph<'a> {
    fn ops(
        &self,
        n: u32,
    ) -> &[SDValue] {
        &self.dag.node(SDValue { node: n, res: 0 }).ops
    }

    fn values(
        &self,
        n: u32,
    ) -> &[EVT] {
        &self.dag.node(SDValue { node: n, res: 0 }).values
    }

    fn opcode(
        &self,
        n: u32,
    ) -> ISD {
        self.dag.node(SDValue { node: n, res: 0 }).opcode
    }

    fn machine(
        &self,
        n: u32,
    ) -> Option<u32> {
        match self.opcode(n) {
            ISD::Machine(op) => Some(op),
            _ => None,
        }
    }

    /// `getGluedNode`: the node whose glue result `n` takes as its last
    /// operand.
    fn glued_node(
        &self,
        n: u32,
    ) -> Option<u32> {
        let last = self.ops(n).last()?;
        (self.dag.value_type(*last) == EVT::GLUE).then_some(last.node)
    }

    /// The nodes reachable from `root`, and their users among them: a node the
    /// builder or a combine left in the arena is not scheduled.
    fn live_users(
        dag: &SelectionDAG,
        root: SDValue,
    ) -> (Vec<u32>, Vec<u32>) {
        let reachable = Self::reachable_ops(dag, root);
        let mut start = vec![0u32; dag.node_count() + 1];
        for &(operand, _) in &reachable {
            start[operand as usize + 1] += 1;
        }
        for n in 0..dag.node_count() {
            start[n + 1] += start[n];
        }
        let mut next = start.clone();
        let mut list = vec![0u32; reachable.len()];
        for &(operand, user) in &reachable {
            list[next[operand as usize] as usize] = user;
            next[operand as usize] += 1;
        }
        (start, list)
    }

    /// The (operand, user) pairs among the nodes reachable from `root`, each
    /// pair once.
    fn reachable_ops(
        dag: &SelectionDAG,
        root: SDValue,
    ) -> Vec<(u32, u32)> {
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut seen = vec![false; dag.node_count()];
        let mut work = vec![root.node];
        seen[root.node as usize] = true;
        while let Some(n) = work.pop() {
            let first = pairs.len();
            for op in &dag.node(SDValue { node: n, res: 0 }).ops {
                if !pairs[first..].contains(&(op.node, n)) {
                    pairs.push((op.node, n));
                }
                if !std::mem::replace(&mut seen[op.node as usize], true) {
                    work.push(op.node);
                }
            }
        }
        pairs
    }

    fn users_of(
        &self,
        n: u32,
    ) -> &[u32] {
        &self.user_list[self.user_start[n as usize] as usize..self.user_start[n as usize + 1] as usize]
    }

    /// `BuildSchedGraph`.
    fn build(
        dag: &'a SelectionDAG,
        root: SDValue,
        info: &'a dyn SchedInfo,
    ) -> Graph<'a> {
        let (user_start, user_list) = Self::live_users(dag, root);
        let mut graph = Graph {
            dag,
            info,
            sus: Vec::new(),
            node_id: vec![-1; dag.node_count()],
            user_start,
            user_list,
            cur_cycle: 0,
            live_reg_defs: HashMap::default(),
            live_reg_gens: HashMap::default(),
            call_seq_end_for_start: HashMap::default(),
            interferences: Vec::new(),
            sequence: Vec::new(),
            walk: Vec::new(),
        };
        graph.build_sched_units(root);
        graph.add_sched_edges();
        graph
    }

    /// `BuildSchedUnits`.
    fn build_sched_units(
        &mut self,
        root: SDValue,
    ) {
        let mut visited = vec![false; self.node_id.len()];
        let mut work = vec![root.node];
        visited[root.node as usize] = true;
        let mut calls = Vec::new();
        while let Some(ni) = work.pop() {
            let dag = self.dag;
            for op in &dag.node(SDValue { node: ni, res: 0 }).ops {
                if !std::mem::replace(&mut visited[op.node as usize], true) {
                    work.push(op.node);
                }
            }
            if is_passive(self.opcode(ni)) || self.node_id[ni as usize] != -1 {
                continue;
            }
            let num = self.sus.len();
            let mut su = SUnit::new(ni);
            // Scan up to find glued preds.
            let mut n = ni;
            while let Some(above) = self.glued_node(n) {
                n = above;
                self.node_id[n as usize] = num as i32;
                su.nodes.push(n);
                if self.machine(n).is_some_and(|op| self.info.is_call(op)) {
                    su.is_call = true;
                }
            }
            // Scan down to find any glued succs: at most one user takes the
            // glue result.
            let mut n = ni;
            while self.values(n).last() == Some(&EVT::GLUE) {
                let glue = SDValue { node: n, res: (self.values(n).len() - 1) as u8 };
                let user = self.users_of(n).iter().copied().find(|&u| self.ops(u).contains(&glue));
                let Some(user) = user else { break };
                self.node_id[n as usize] = num as i32;
                n = user;
                su.nodes.insert(0, n);
                if self.machine(n).is_some_and(|op| self.info.is_call(op)) {
                    su.is_call = true;
                }
            }
            if su.is_call {
                calls.push(num);
            }
            if self.opcode(ni) == ISD::TokenFactor {
                su.is_schedule_low = true;
            }
            self.node_id[n as usize] = num as i32;
            su.latency = if self.opcode(n) == ISD::TokenFactor { 0 } else { 1 };
            // before `add_sched_edges`
            su.reg_defs = self.count_reg_defs(&su.nodes) as u32;
            su.num_reg_defs_left = su.reg_defs;
            self.sus.push(su);
        }
        // Find all call operands.
        while let Some(su) = calls.pop() {
            for n in self.sus[su].nodes.clone() {
                if self.opcode(n) != ISD::CopyToReg {
                    continue;
                }
                let src = self.ops(n)[2].node;
                if is_passive(self.opcode(src)) {
                    continue;
                }
                let id = self.node_id[src as usize] as usize;
                self.sus[id].is_call_op = true;
            }
        }
    }

    /// `InitNumRegDefsLeft`: the values `RegDefIter` visits in a unit's nodes,
    /// the bottom node first: the results a machine node defines (the first
    /// `getNumDefs`) and a `CopyFromReg`'s, that have a user.
    fn count_reg_defs(
        &self,
        nodes: &[u32],
    ) -> usize {
        let mut count = 0;
        for &n in nodes {
            let defs = match self.machine(n) {
                Some(op) => self.values(n).len().min(self.info.num_defs(op)),
                None if self.opcode(n) == ISD::CopyFromReg => 1,
                None => 0,
            };
            count += (0..defs).filter(|&i| self.has_any_use_of_value(n, i)).count();
        }
        count
    }

    /// `InstrEmitter::CountResults`.
    fn count_results(
        &self,
        n: u32,
    ) -> usize {
        let values = self.values(n);
        let mut count = values.len();
        while count > 0 && values[count - 1] == EVT::GLUE {
            count -= 1;
        }
        if count > 0 && values[count - 1] == EVT::OTHER {
            count -= 1;
        }
        count
    }

    fn has_any_use_of_value(
        &self,
        n: u32,
        res: usize,
    ) -> bool {
        self.users_of(n).iter().any(|&u| self.ops(u).iter().any(|op| op.node == n && usize::from(op.res) == res))
    }

    /// `AddSchedEdges`.
    fn add_sched_edges(&mut self) {
        for su in 0..self.sus.len() {
            let nodes = self.sus[su].nodes.clone();
            for &n in &nodes {
                if let Some(op) = self.machine(n)
                    && !self.info.implicit_defs(op).is_empty()
                {
                    self.sus[su].has_phys_reg_clobbers = true;
                    let mut used = self.count_results(n);
                    while used != 0 && !self.has_any_use_of_value(n, used - 1) {
                        used -= 1;
                    }
                    if used > self.info.num_defs(op) {
                        self.sus[su].has_phys_reg_defs = true;
                    }
                }
                for (at, operand) in self.ops(n).to_vec().into_iter().enumerate() {
                    let op_node = operand.node;
                    if is_passive(self.opcode(op_node)) {
                        continue;
                    }
                    let op_su = self.node_id[op_node as usize] as usize;
                    if op_su == su {
                        continue;
                    }
                    let vt = self.dag.value_type(operand);
                    debug_assert!(vt != EVT::GLUE, "glued nodes are in one unit");
                    let is_chain = vt == EVT::OTHER;
                    let mut latency = if is_chain { 1 } else { self.sus[op_su].latency };
                    if is_chain && self.opcode(op_node) == ISD::TokenFactor {
                        latency = 0;
                    }
                    let reg = if is_chain { None } else { self.phys_reg_dependency(operand, n, at) };
                    let dep = Dep { su: op_su, kind: if is_chain { Kind::Barrier } else { Kind::Data }, latency, reg };
                    // Several register uses in one unit count as one: the
                    // definitions left are reduced to match.
                    if !self.add_pred(su, dep) && !is_chain && self.sus[op_su].num_reg_defs_left > 1 {
                        self.sus[op_su].num_reg_defs_left -= 1;
                    }
                }
            }
        }
    }

    /// `CheckForPhysRegDependency`: the physical register `user` takes from
    /// `def`'s implicit result when `user` is a `CopyToReg` of it (operand 2).
    fn phys_reg_dependency(
        &self,
        def: SDValue,
        user: u32,
        at: usize,
    ) -> Option<u16> {
        if at != 2 || self.opcode(user) != ISD::CopyToReg {
            return None;
        }
        let reg = match self.dag.node(self.ops(user)[1]).data {
            NodeData::PhysReg(id) => self.info.pin_register(id)?,
            _ => return None,
        };
        // a physical register read back by `CopyFromReg` (a call's flags)
        if self.opcode(def.node) == ISD::CopyFromReg
            && matches!(
                self.dag.node(self.ops(def.node)[1]).data,
                NodeData::PhysReg(id) if self.info.pin_register(id) == Some(reg)
            )
        {
            return Some(reg);
        }
        let op = self.machine(def.node)?;
        let first = self.info.num_defs(op);
        let result = usize::from(def.res);
        (result >= first && self.info.implicit_defs(op).get(result - first) == Some(&reg)).then_some(reg)
    }

    /// `SUnit::addPred` (Required): false where the edge was already there.
    fn add_pred(
        &mut self,
        su: usize,
        dep: Dep,
    ) -> bool {
        let preds = self.sus[su].preds.clone();
        for (i, pred) in preds.iter().enumerate() {
            if pred.overlaps(dep) {
                if pred.latency < dep.latency {
                    let forward = Dep { su, ..*pred };
                    if let Some(s) = self.sus[pred.su].succs.iter_mut().find(|s| **s == forward) {
                        s.latency = dep.latency;
                    }
                    self.sus[su].preds[i].latency = dep.latency;
                    self.sus[su].depth_current = false;
                    self.set_height_dirty(dep.su);
                }
                return false;
            }
        }
        let n = dep.su;
        if dep.kind == Kind::Data {
            self.sus[su].num_preds += 1;
            self.sus[n].num_succs += 1;
        }
        if !self.sus[n].is_scheduled {
            self.sus[su].num_preds_left += 1;
        }
        if !self.sus[su].is_scheduled {
            self.sus[n].num_succs_left += 1;
        }
        self.sus[su].preds.push(dep);
        self.sus[n].succs.push(Dep { su, ..dep });
        self.sus[su].depth_current = false;
        self.set_height_dirty(n);
        true
    }

    /// `SUnit::removePred`.
    fn remove_pred(
        &mut self,
        su: usize,
        dep: Dep,
    ) {
        let Some(i) = self.sus[su].preds.iter().position(|p| *p == dep) else { return };
        let n = dep.su;
        let forward = Dep { su, ..dep };
        let j = self.sus[n].succs.iter().position(|s| *s == forward).expect("preds and succs match");
        if dep.kind == Kind::Data {
            self.sus[su].num_preds -= 1;
            self.sus[n].num_succs -= 1;
        }
        if !self.sus[n].is_scheduled {
            self.sus[su].num_preds_left -= 1;
        }
        if !self.sus[su].is_scheduled {
            self.sus[n].num_succs_left -= 1;
        }
        self.sus[n].succs.remove(j);
        self.sus[su].preds.remove(i);
        self.sus[su].depth_current = false;
        self.set_height_dirty(n);
    }

    /// `SUnit::setHeightDirty`: this unit and every predecessor's height.
    fn set_height_dirty(
        &mut self,
        su: usize,
    ) {
        if !self.sus[su].height_current {
            return;
        }
        let mut work = std::mem::take(&mut self.walk);
        work.push(su);
        while let Some(cur) = work.pop() {
            if !self.sus[cur].height_current {
                continue;
            }
            self.sus[cur].height_current = false;
            work.extend(self.sus[cur].preds.iter().map(|p| p.su));
        }
        self.walk = work;
    }

    /// `SUnit::getHeight`.
    fn height(
        &mut self,
        su: usize,
    ) -> u32 {
        if !self.sus[su].height_current {
            self.compute_height(su);
        }
        self.sus[su].height
    }

    /// `SUnit::getDepth`.
    fn depth(
        &mut self,
        su: usize,
    ) -> u32 {
        if !self.sus[su].depth_current {
            self.compute_depth(su);
        }
        self.sus[su].depth
    }

    fn compute_depth(
        &mut self,
        su: usize,
    ) {
        let mut work = std::mem::take(&mut self.walk);
        work.push(su);
        while let Some(&cur) = work.last() {
            if let Some(p) = self.sus[cur].preds.iter().find(|p| !self.sus[p.su].depth_current) {
                work.push(p.su);
                continue;
            }
            work.pop();
            let max = self.sus[cur].preds.iter().map(|p| self.sus[p.su].depth + p.latency).max().unwrap_or(0);
            self.sus[cur].depth = max;
            self.sus[cur].depth_current = true;
        }
        self.walk = work;
    }

    fn compute_height(
        &mut self,
        su: usize,
    ) {
        let mut work = std::mem::take(&mut self.walk);
        work.push(su);
        while let Some(&cur) = work.last() {
            if let Some(s) = self.sus[cur].succs.iter().find(|s| !self.sus[s.su].height_current) {
                work.push(s.su);
                continue;
            }
            work.pop();
            let max = self.sus[cur].succs.iter().map(|s| self.sus[s.su].height + s.latency).max().unwrap_or(0);
            self.sus[cur].height = max;
            self.sus[cur].height_current = true;
        }
        self.walk = work;
    }

    /// `SUnit::setHeightToAtLeast`.
    fn set_height_to_at_least(
        &mut self,
        su: usize,
        height: u32,
    ) {
        if height <= self.height(su) {
            return;
        }
        self.set_height_dirty(su);
        self.sus[su].height = height;
        self.sus[su].height_current = true;
    }

    /// `canClobberPhysRegDefs`: whether scheduling `su` below `succ` could
    /// clobber a physical register `succ` defines past its explicit results.
    fn can_clobber_phys_reg_defs(
        &self,
        succ: usize,
        su: usize,
    ) -> bool {
        let n = self.sus[succ].node();
        let op = self.machine(n).expect("a unit with physical register defs is a machine node");
        let num_defs = self.info.num_defs(op);
        let imp_defs = self.info.implicit_defs(op);
        for &node in &self.sus[su].nodes {
            let Some(sop) = self.machine(node) else { continue };
            let su_imp = self.info.implicit_defs(sop);
            if su_imp.is_empty() {
                continue;
            }
            for i in num_defs..self.values(n).len() {
                let Some(&reg) = imp_defs.get(i - num_defs) else { break };
                if su_imp.iter().any(|&r| self.info.regs_overlap(reg, r)) {
                    return true;
                }
            }
        }
        false
    }

    /// `ListScheduleBottomUp`, the order top first.
    fn list_schedule_bottom_up(
        &mut self,
        root: SDValue,
        queue: &mut Queue,
    ) -> Vec<usize> {
        self.sequence = Vec::with_capacity(self.sus.len());
        if !self.sus.is_empty() {
            let root_su = self.node_id[root.node as usize] as usize;
            debug_assert!(self.sus[root_su].succs.is_empty(), "the graph's root has no successors");
            self.sus[root_su].is_available = true;
            queue.push(&mut self.sus, root_su);
        }
        while let Some(su) = self.pick_node_to_schedule_bottom_up(queue) {
            // `AdvancePastStalls` is a no-op without a hazard recognizer and
            // with unit latencies: the height is at most the cycle.
            self.schedule_node_bottom_up(su, queue);
            self.sequence.push(su);
        }
        let mut sequence = std::mem::take(&mut self.sequence);
        sequence.reverse();
        sequence
    }

    /// `PickNodeToScheduleBottomUp`: the best available unit that clobbers no
    /// live physical register; the ones that would wait in `interferences`.
    ///
    /// DEVIATE SD30 (part): when every candidate is held back the pin
    /// backtracks, duplicates a node or inserts cross-class copies
    /// (`BacktrackBottomUp`, `CopyAndMoveSuccessors`); here the first held
    /// unit goes anyway.
    fn pick_node_to_schedule_bottom_up(
        &mut self,
        queue: &mut Queue,
    ) -> Option<usize> {
        let mut current = queue.pop(self);
        while let Some(su) = current {
            let live = self.delay_for_live_regs_bottom_up(su);
            if live.is_empty() {
                break;
            }
            self.interferences.push((su, live));
            current = queue.pop(self);
        }
        if current.is_some() {
            return current;
        }
        if self.interferences.is_empty() {
            return None;
        }
        // Every candidate waits for a live register: unschedule back to the
        // unit that wants it and force the waiting unit below it
        // (`BacktrackBottomUp`).
        for i in 0..self.interferences.len() {
            let (try_su, lregs) = self.interferences[i].clone();
            let mut bt: Option<usize> = None;
            for reg in &lregs {
                let Some(&gen_su) = self.live_reg_gens.get(reg) else { continue };
                if bt.is_none_or(|b| self.height(gen_su) < self.height(b)) {
                    bt = Some(gen_su);
                }
            }
            let Some(bt) = bt else { continue };
            if self.reaches(try_su, bt) {
                continue;
            }
            self.backtrack_bottom_up(try_su, bt, queue);
            // Force the waiting unit to be scheduled before the unit that
            // wants the register: `bt` precedes it in the program.
            if self.sus[bt].is_available {
                self.sus[bt].is_available = false;
                Self::dequeue(&mut self.sus, queue, bt);
            }
            self.add_pred(try_su, Dep { su: bt, kind: Kind::Barrier, latency: 0, reg: None });
            return if !self.sus[try_su].is_available || queue_free(&self.sus, try_su) {
                queue.pop(self)
            } else {
                Self::dequeue(&mut self.sus, queue, try_su);
                Some(try_su)
            };
        }
        // Nothing to backtrack to: the live register's definition is cloned
        // where the target cannot copy the register (`CopyAndMoveSuccessors`).
        let (try_su, lregs) = self.interferences[0].clone();
        if let [reg] = lregs[..]
            && self.info.has_cross_copy_class(reg)
            && let Some(&lr_def) = self.live_reg_defs.get(&reg)
            && let Some(new_def) = self.copy_and_move_successors(lr_def, queue)
        {
            self.live_reg_defs.insert(reg, new_def);
            self.add_pred(new_def, Dep { su: try_su, kind: Kind::Artificial, latency: 0, reg: None });
            self.sus[try_su].is_available = false;
            return Some(new_def);
        }
        // DEVIATE SD30 (part): the cross-class copies
        // (`InsertCopiesAndMoveSuccs`) are not translated; the first
        // held unit goes anyway.
        let (su, _) = self.interferences.remove(0);
        Some(su)
    }

    /// `CopyAndMoveSuccessors`: a clone of `su` with the same predecessors
    /// that takes over the successors already scheduled. None for a unit of
    /// several nodes or one with a chain or glue result (the pin unfolds a
    /// load there, which is not translated).
    fn copy_and_move_successors(
        &mut self,
        su: usize,
        queue: &mut Queue,
    ) -> Option<usize> {
        if self.sus[su].nodes.len() != 1
            || self.values(self.sus[su].node()).iter().any(|vt| *vt == EVT::GLUE || *vt == EVT::OTHER)
        {
            return None;
        }
        let new = self.sus.len();
        let mut clone = self.sus[su].clone();
        clone.preds.clear();
        clone.succs.clear();
        clone.num_preds = 0;
        clone.num_succs = 0;
        clone.num_preds_left = 0;
        clone.num_succs_left = 0;
        // `Clone` does not copy it
        clone.num_reg_defs_left = 0;
        clone.depth_current = false;
        clone.height_current = false;
        clone.is_available = false;
        clone.is_scheduled = false;
        clone.queue_id = 0;
        self.sus.push(clone);
        for pred in self.sus[su].preds.clone() {
            if pred.kind != Kind::Artificial {
                self.add_pred(new, pred);
            }
        }
        let mut moved = Vec::new();
        for succ in self.sus[su].succs.clone() {
            if succ.kind != Kind::Artificial && self.sus[succ.su].is_scheduled {
                self.add_pred(succ.su, Dep { su: new, ..succ });
                moved.push((succ.su, Dep { su, ..succ }));
            }
        }
        for (from, dep) in moved {
            self.remove_pred(from, dep);
        }
        // `updateNode(SU)` and `addNode(NewSU)`.
        queue.sethi_ullman.resize(self.sus.len(), 0);
        queue.sethi_ullman[su] = 0;
        queue.calc_sethi_ullman(&self.sus, su);
        queue.calc_sethi_ullman(&self.sus, new);
        Some(new)
    }

    /// Whether `to` is reached from `from` through successors.
    fn reaches(
        &self,
        from: usize,
        to: usize,
    ) -> bool {
        let mut seen = vec![false; self.sus.len()];
        let mut work = vec![from];
        while let Some(su) = work.pop() {
            for succ in &self.sus[su].succs {
                if succ.su == to {
                    return true;
                }
                if !seen[succ.su] {
                    seen[succ.su] = true;
                    work.push(succ.su);
                }
            }
        }
        false
    }

    /// `AvailableQueue->remove(su)`.
    fn dequeue(
        sus: &mut [SUnit],
        queue: &mut Queue,
        su: usize,
    ) {
        if let Some(at) = queue.queue.iter().position(|&one| one == su) {
            queue.queue.swap_remove(at);
            sus[su].queue_id = 0;
        }
    }

    /// `BacktrackBottomUp`: the scheduled units back to and including `bt`
    /// are unscheduled.
    fn backtrack_bottom_up(
        &mut self,
        _su: usize,
        bt: usize,
        queue: &mut Queue,
    ) {
        while let Some(old) = self.sequence.pop() {
            self.cur_cycle = self.height(old);
            self.unschedule_node_bottom_up(old, queue);
            if old == bt {
                break;
            }
        }
    }

    /// `UnscheduleNodeBottomUp`.
    fn unschedule_node_bottom_up(
        &mut self,
        su: usize,
        queue: &mut Queue,
    ) {
        for pred in self.sus[su].preds.clone() {
            if let Some(reg) = pred.reg.filter(|_| pred.is_assigned_reg())
                && self.live_reg_gens.get(&reg) == Some(&su)
            {
                self.live_reg_defs.remove(&reg);
                self.live_reg_gens.remove(&reg);
                self.release_interferences(Some(reg), queue);
            }
        }
        // The beginning of a sequence takes its resource back; its end gives
        // it up.
        if self.sus[su].nodes.iter().any(|&n| self.is_call_frame(n, false)) {
            let end = self.call_seq_end_for_start[&su];
            self.live_reg_defs.insert(CALL_RESOURCE, su);
            self.live_reg_gens.insert(CALL_RESOURCE, end);
        }
        if self.live_reg_gens.get(&CALL_RESOURCE) == Some(&su)
            && self.sus[su].nodes.iter().any(|&n| self.is_call_frame(n, true))
        {
            self.live_reg_defs.remove(&CALL_RESOURCE);
            self.live_reg_gens.remove(&CALL_RESOURCE);
            self.release_interferences(Some(CALL_RESOURCE), queue);
        }
        for succ in self.sus[su].succs.clone() {
            if let Some(reg) = succ.reg.filter(|_| succ.is_assigned_reg()) {
                self.live_reg_defs.entry(reg).or_insert(su);
                // The earliest scheduled unit that wants it.
                let mut gen_su = succ.su;
                for other in self.sus[su].succs.clone() {
                    if other.is_assigned_reg()
                        && other.reg == Some(reg)
                        && self.sus[other.su].is_scheduled
                        && self.height(other.su) < self.height(gen_su)
                    {
                        gen_su = other.su;
                    }
                }
                self.live_reg_gens.insert(reg, gen_su);
            }
        }
        // Its predecessors have a successor to wait for again.
        for pred in self.sus[su].preds.clone() {
            let p = pred.su;
            self.sus[p].num_succs_left += 1;
            if self.sus[p].is_available {
                self.sus[p].is_available = false;
                Self::dequeue(&mut self.sus, queue, p);
                self.interferences.retain(|(held, _)| *held != p);
            }
        }
        self.sus[su].is_scheduled = false;
        if self.sus[su].num_succs_left == 0 {
            self.sus[su].is_available = true;
            if queue_free(&self.sus, su) {
                queue.push(&mut self.sus, su);
            }
        }
    }

    /// `DelayForLiveRegsBottomUp`: the live physical registers `su` would
    /// clobber (or whose definition it would separate from its consumer).
    fn delay_for_live_regs_bottom_up(
        &self,
        su: usize,
    ) -> Vec<u16> {
        if self.live_reg_defs.is_empty() {
            return Vec::new();
        }
        let mut live = Vec::new();
        // The registers `su` takes from a unit that is not the live definition
        // of that register.
        for pred in &self.sus[su].preds {
            if let Some(reg) = pred.reg.filter(|_| pred.is_assigned_reg())
                && self.live_reg_defs.get(&reg) != Some(&su)
            {
                self.check_for_live_reg_def(pred.su, reg, &mut live);
            }
        }
        for &node in &self.sus[su].nodes {
            if let Some(reg) = self.info.copy_clobbers()
                && self.is_virtual_copy(node)
            {
                self.check_for_live_reg_def(su, reg, &mut live);
            }
            let Some(op) = self.machine(node) else { continue };
            // In the middle of a call sequence, another one does not begin.
            if self.is_call_frame(node, true)
                && let Some(&generator) = self.live_reg_gens.get(&CALL_RESOURCE)
            {
                let mut top = self.sus[generator].node();
                while let Some(above) = self.glued_node(top) {
                    top = above;
                }
                if !self.is_chain_dependent(top, node, 0) && !live.contains(&CALL_RESOURCE) {
                    live.push(CALL_RESOURCE);
                }
            }
            for &reg in self.info.implicit_defs(op) {
                self.check_for_live_reg_def(su, reg, &mut live);
            }
        }
        live
    }

    /// A `CopyToReg` into, or `CopyFromReg` out of, a virtual register.
    fn is_virtual_copy(
        &self,
        node: u32,
    ) -> bool {
        matches!(self.opcode(node), ISD::CopyToReg | ISD::CopyFromReg)
            && virtual_register(self, self.ops(node)[1])
    }

    /// `CheckForLiveRegDef`.
    fn check_for_live_reg_def(
        &self,
        su: usize,
        reg: u16,
        live: &mut Vec<u16>,
    ) {
        for (&other, &def) in &self.live_reg_defs {
            if other != CALL_RESOURCE && def != su && self.info.regs_overlap(reg, other) && !live.contains(&other) {
                live.push(other);
            }
        }
    }

    /// `ScheduleNodeBottomUp`.
    fn schedule_node_bottom_up(
        &mut self,
        su: usize,
        queue: &mut Queue,
    ) {
        let cycle = self.cur_cycle;
        self.set_height_to_at_least(su, cycle);
        if preference(self.info) == Preference::Ilp {
            self.scheduled_node(su);
        }
        // `AdvanceToCycle(CurCycle + 1)`: no hazard recognizer, one per cycle.
        self.cur_cycle += 1;
        for pred in self.sus[su].preds.clone() {
            self.release_pred(su, pred, queue);
            if let Some(reg) = pred.reg.filter(|_| pred.is_assigned_reg()) {
                // A physical register dependence: nothing that can clobber the
                // register may go between the definition and this unit.
                self.live_reg_defs.insert(reg, pred.su);
                self.live_reg_gens.entry(reg).or_insert(su);
            }
        }
        // A call sequence's end: nothing else may be scheduled into the
        // sequence, which runs to its beginning (the stack arguments of two
        // calls would share the same slots).
        if !self.live_reg_defs.contains_key(&CALL_RESOURCE)
            && let Some(end) = self.sus[su].nodes.iter().copied().find(|&n| self.is_call_frame(n, true))
        {
            let (mut nest, mut max_nest) = (0, 0);
            let start = self.find_call_seq_start(end, &mut nest, &mut max_nest).expect("a call sequence has a start");
            let def = self.node_id[start as usize] as usize;
            self.call_seq_end_for_start.insert(def, su);
            self.live_reg_defs.insert(CALL_RESOURCE, def);
            self.live_reg_gens.insert(CALL_RESOURCE, su);
        }
        // The definitions that were live for this unit are done.
        for succ in self.sus[su].succs.clone() {
            if let Some(reg) = succ.reg.filter(|_| succ.is_assigned_reg())
                && self.live_reg_defs.get(&reg) == Some(&su)
            {
                self.live_reg_defs.remove(&reg);
                self.live_reg_gens.remove(&reg);
                self.release_interferences(Some(reg), queue);
            }
        }
        // The beginning of the sequence in progress.
        if self.live_reg_defs.get(&CALL_RESOURCE) == Some(&su)
            && self.sus[su].nodes.iter().any(|&n| self.is_call_frame(n, false))
        {
            self.live_reg_defs.remove(&CALL_RESOURCE);
            self.live_reg_gens.remove(&CALL_RESOURCE);
            self.release_interferences(Some(CALL_RESOURCE), queue);
        }
        self.sus[su].is_scheduled = true;
    }

    /// `releaseInterferences`: the held units that waited for `reg` (any, for
    /// none) are available again.
    fn release_interferences(
        &mut self,
        reg: Option<u16>,
        queue: &mut Queue,
    ) {
        let held = std::mem::take(&mut self.interferences);
        for (su, live) in held {
            if reg.is_some_and(|reg| !live.contains(&reg)) {
                self.interferences.push((su, live));
            } else if self.sus[su].is_available && queue_free(&self.sus, su) {
                queue.push(&mut self.sus, su);
            }
        }
    }

    /// `ReleasePred`.
    fn release_pred(
        &mut self,
        su: usize,
        edge: Dep,
        queue: &mut Queue,
    ) {
        let pred = edge.su;
        assert!(self.sus[pred].num_succs_left != 0, "a unit was released too many times");
        self.sus[pred].num_succs_left -= 1;
        // `!forceUnitLatencies()`: when the predecessor can go without a stall
        if preference(self.info) == Preference::Ilp {
            let ready = self.height(su) + edge.latency;
            self.set_height_to_at_least(pred, ready);
        }
        if self.sus[pred].num_succs_left == 0 {
            self.sus[pred].is_available = true;
            queue.push(&mut self.sus, pred);
        }
    }

    /// Whether `n` is the target's `CALLSEQ_BEGIN` (`destroy` false) or
    /// `CALLSEQ_END` machine node.
    fn is_call_frame(
        &self,
        n: u32,
        destroy: bool,
    ) -> bool {
        let wanted = if destroy { self.info.call_frame_destroy() } else { self.info.call_frame_setup() };
        wanted.is_some() && self.machine(n) == wanted
    }

    /// The first chain operand of `n`, unless it is the entry token. (A
    /// `RegisterMask` has the type `OTHER` here; the pin's is untyped.)
    fn chain_above(
        &self,
        n: u32,
    ) -> Option<u32> {
        let op = self
            .ops(n)
            .iter()
            .find(|op| self.dag.value_type(**op) == EVT::OTHER && self.opcode(op.node) != ISD::RegisterMask)?;
        (self.opcode(op.node) != ISD::EntryToken).then_some(op.node)
    }

    /// `FindCallSeqStart`: from a call sequence's end, its beginning.
    fn find_call_seq_start(
        &self,
        mut n: u32,
        nest: &mut u32,
        max_nest: &mut u32,
    ) -> Option<u32> {
        loop {
            // the path with the most nesting finds the matching start
            if self.opcode(n) == ISD::TokenFactor {
                let mut best = None;
                let mut best_max = *max_nest;
                for op in self.ops(n) {
                    let (mut my_nest, mut my_max) = (*nest, *max_nest);
                    if let Some(new) = self.find_call_seq_start(op.node, &mut my_nest, &mut my_max)
                        && (best.is_none() || my_max > best_max)
                    {
                        best = Some(new);
                        best_max = my_max;
                    }
                }
                *max_nest = best_max;
                return best;
            }
            if self.is_call_frame(n, true) {
                *nest += 1;
                *max_nest = (*max_nest).max(*nest);
            } else if self.is_call_frame(n, false) {
                debug_assert!(*nest != 0, "a call sequence starts before it ends");
                *nest -= 1;
                if *nest == 0 {
                    return Some(n);
                }
            }
            n = self.chain_above(n)?;
        }
    }

    /// `IsChainDependent`: whether `outer` reaches `inner` through chain
    /// operands.
    fn is_chain_dependent(
        &self,
        outer: u32,
        inner: u32,
        mut nest: u32,
    ) -> bool {
        let mut n = outer;
        loop {
            if n == inner {
                return true;
            }
            if self.opcode(n) == ISD::TokenFactor {
                return self.ops(n).iter().any(|op| self.is_chain_dependent(op.node, inner, nest));
            }
            if self.is_call_frame(n, true) {
                nest += 1;
            } else if self.is_call_frame(n, false) {
                if nest == 0 {
                    return false;
                }
                nest -= 1;
            }
            match self.chain_above(n) {
                Some(above) => n = above,
                None => return false,
            }
        }
    }

    /// `RegPressureDiff` without the live-use count (that priority is off):
    /// the definitions of the predecessors that are not all used yet, less
    /// this unit's own, in the register classes at their limit.
    ///
    /// DEVIATE SD32: the pressure per register class is not tracked. Reason:
    /// the one target on the ILP scheduler, M68k, keeps the default
    /// `getRegPressureLimit` of 0, so every class is always at its limit and
    /// only the counts of definitions matter. Exit: a target that sets a
    /// limit and asks for `Preference::Ilp`. Owner: m68k-spike.
    fn reg_pressure_diff(
        &self,
        su: usize,
    ) -> i32 {
        let mut diff = 0;
        for pred in self.sus[su].preds.iter().filter(|p| !p.is_ctrl()) {
            // zero when enough uses of the node are scheduled to cover its
            // definitions: they are all live
            let left = self.sus[pred.su].num_reg_defs_left;
            if left != 0 {
                diff += self.sus[pred.su].reg_defs as i32;
            }
        }
        let unit = &self.sus[su];
        let node = unit.node();
        if let Some(op) = self.machine(node)
            && unit.num_succs != 0
        {
            let defs = self.values(node).len().min(self.info.num_defs(op));
            diff -= (0..defs).filter(|&i| self.has_any_use_of_value(node, i)).count() as i32;
        }
        diff
    }

    /// `scheduledNode` of the register-pressure queue: the uses of the
    /// predecessors' definitions one fewer. (The pressure per class is not
    /// kept: see [`Graph::reg_pressure_diff`].)
    fn scheduled_node(
        &mut self,
        su: usize,
    ) {
        for pred in self.sus[su].preds.clone().iter().filter(|p| !p.is_ctrl()) {
            let left = &mut self.sus[pred.su].num_reg_defs_left;
            *left = left.saturating_sub(1);
        }
    }

    /// `closestSucc`.
    fn closest_succ(
        &mut self,
        su: usize,
    ) -> u32 {
        let mut max = 0;
        for succ in self.sus[su].succs.clone() {
            if succ.is_ctrl() {
                continue;
            }
            let mut height = self.height(succ.su);
            if self.opcode(self.sus[succ.su].node()) == ISD::CopyToReg {
                height = self.closest_succ(succ.su) + 1;
            }
            max = max.max(height);
        }
        max
    }
}

/// The slot `LiveRegDefs` keeps for the call sequence in progress
/// (`TRI->getNumRegs()`): no register has this number.
const CALL_RESOURCE: u16 = u16::MAX;

/// Whether a held unit may go back to the queue: it is not in it.
fn queue_free(
    sus: &[SUnit],
    su: usize,
) -> bool {
    sus[su].queue_id == 0
}

/// `ScheduleDAGTopologicalSort`.
struct Topo {
    node2index: Vec<i32>,
    index2node: Vec<usize>,
    dirty: bool,
    updates: Vec<(usize, usize)>,
    reachable: HashMap<(usize, usize), bool>,
}

impl Topo {
    fn new(sus: &[SUnit]) -> Topo {
        let mut topo = Topo {
            node2index: vec![0; sus.len()],
            index2node: vec![0; sus.len()],
            dirty: false,
            updates: Vec::new(),
            reachable: HashMap::default(),
        };
        topo.init(sus);
        topo
    }

    /// `InitDAGTopologicalSorting`.
    fn init(
        &mut self,
        sus: &[SUnit],
    ) {
        self.dirty = false;
        self.updates.clear();
        self.reachable.clear();
        let size = sus.len();
        let mut work = Vec::new();
        for (num, su) in sus.iter().enumerate() {
            self.node2index[num] = su.succs.len() as i32;
            if su.succs.is_empty() {
                work.push(num);
            }
        }
        let mut id = size as i32;
        while let Some(su) = work.pop() {
            id -= 1;
            self.allocate(su, id);
            for pred in &sus[su].preds {
                self.node2index[pred.su] -= 1;
                if self.node2index[pred.su] == 0 {
                    work.push(pred.su);
                }
            }
        }
    }

    fn allocate(
        &mut self,
        n: usize,
        index: i32,
    ) {
        self.node2index[n] = index;
        self.index2node[index as usize] = n;
    }

    /// `FixOrder`.
    fn fix_order(
        &mut self,
        sus: &[SUnit],
    ) {
        if self.dirty {
            self.init(sus);
            return;
        }
        for (y, x) in std::mem::take(&mut self.updates) {
            self.add_pred(sus, y, x);
        }
    }

    /// `AddPredQueued`: `y` now depends on `x`.
    fn add_pred_queued(
        &mut self,
        y: usize,
        x: usize,
    ) {
        self.dirty = self.dirty || self.updates.len() > 10;
        if !self.dirty {
            self.updates.push((y, x));
        }
    }

    fn add_pred(
        &mut self,
        sus: &[SUnit],
        y: usize,
        x: usize,
    ) {
        let lower = self.node2index[y];
        let upper = self.node2index[x];
        if lower < upper {
            let mut visited = vec![false; sus.len()];
            let has_loop = self.dfs(sus, y, upper, &mut visited);
            assert!(!has_loop, "an inserted edge creates a loop");
            self.shift(&mut visited, lower, upper);
        }
        self.reachable.clear();
    }

    /// `DFS`: whether `su` reaches the node at index `upper`; marks the nodes
    /// seen.
    fn dfs(
        &self,
        sus: &[SUnit],
        su: usize,
        upper: i32,
        visited: &mut [bool],
    ) -> bool {
        let mut work = vec![su];
        while let Some(su) = work.pop() {
            visited[su] = true;
            for succ in sus[su].succs.iter().rev() {
                let s = succ.su;
                if self.node2index[s] == upper {
                    return true;
                }
                if !visited[s] && self.node2index[s] < upper {
                    work.push(s);
                }
            }
        }
        false
    }

    fn shift(
        &mut self,
        visited: &mut [bool],
        lower: i32,
        upper: i32,
    ) {
        let mut moved = Vec::new();
        let mut shift = 0;
        let mut i = lower;
        while i <= upper {
            let w = self.index2node[i as usize];
            if visited[w] {
                visited[w] = false;
                moved.push(w);
                shift += 1;
            } else {
                self.allocate(w, i - shift);
            }
            i += 1;
        }
        for w in moved {
            self.allocate(w, i - shift);
            i += 1;
        }
    }

    /// `IsReachable`: whether `target` reaches `su` through successors.
    fn is_reachable(
        &mut self,
        sus: &[SUnit],
        su: usize,
        target: usize,
    ) -> bool {
        self.fix_order(sus);
        let lower = self.node2index[target];
        let upper = self.node2index[su];
        if lower >= upper {
            return false;
        }
        if let Some(&known) = self.reachable.get(&(target, su)) {
            return known;
        }
        let mut visited = vec![false; sus.len()];
        let has_loop = self.dfs(sus, target, upper, &mut visited);
        if !has_loop {
            self.reachable.insert((target, su), false);
        }
        has_loop
    }
}

thread_local! {
    /// Source order on for this thread: what `LLRM_SRC_ORDER` does for the
    /// process.
    pub(super) static SRC_ORDER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `createDefaultScheduler`: X86 enables the machine scheduler and its default
/// scheduler role (`enableMachineScheduler() &&
/// enableMachineSchedDefaultSched()`), so the pin's DAG scheduler is
/// `createSourceListDAGScheduler`: a `SrcRegReductionPriorityQueue`
/// (`srcorder`), IR order first. On at every width, as the pin has it: off at
/// m16 a select could fall inside a call frame and `jumps` grew 28 B.
fn src_order(info: &dyn SchedInfo) -> bool {
    preference(info) == Preference::Source
}

/// The scheduler this run builds: the target's, or source order where the
/// thread or the environment forces it.
fn preference(info: &dyn SchedInfo) -> Preference {
    if SRC_ORDER.get() || llrm_support::env_flag!("LLRM_SRC_ORDER") {
        return Preference::Source;
    }
    info.preference()
}

/// `RegReductionPQBase` with `SrcRegReductionPriorityQueue`: Sethi-Ullman
/// numbers and `src_ls_rr_sort`.
#[derive(Default)]
struct Queue {
    queue: Vec<usize>,
    cur_queue_id: u32,
    sethi_ullman: Vec<u32>,
    /// The work stack of `calc_sethi_ullman`, kept between calls.
    walk: Vec<(usize, usize)>,
}

impl Queue {
    /// `initNodes`: `PrescheduleNodesWithMultipleUses` (`!TracksRegPressure`),
    /// then `CalculateSethiUllmanNumbers`. `AddPseudoTwoAddrDeps` stays off,
    /// as `-disable-2addr-hack` defaults to true.
    fn init(
        &mut self,
        graph: &mut Graph<'_>,
        topo: &mut Topo,
    ) {
        // `!TracksRegPressure && !SrcOrder`: the source-order queue X86 uses
        // and the register-pressure one of the ILP scheduler leave the edges
        // alone.
        if preference(graph.info) == Preference::Burr {
            self.preschedule_nodes_with_multiple_uses(graph, topo);
        }
        self.sethi_ullman = vec![0; graph.sus.len()];
        for su in 0..graph.sus.len() {
            self.calc_sethi_ullman(&graph.sus, su);
        }
    }

    /// `PrescheduleNodesWithMultipleUses`.
    fn preschedule_nodes_with_multiple_uses(
        &mut self,
        graph: &mut Graph<'_>,
        topo: &mut Topo,
    ) {
        'unit: for su in 0..graph.sus.len() {
            if graph.sus[su].num_succs != 0 || graph.sus[su].num_preds != 1 {
                continue;
            }
            let node = graph.sus[su].node();
            if graph.opcode(node) == ISD::CopyToReg && virtual_register(graph, graph.ops(node)[1]) {
                continue;
            }
            // A call-frame setup among the chain predecessors keeps the unit
            // where it is. The setup opcode is the target's (SD30).
            let Some(pred) = graph.sus[su].preds.iter().find(|p| !p.is_ctrl()).map(|p| p.su) else { continue };
            if graph.sus[pred].has_phys_reg_defs || graph.sus[pred].num_succs == 1 {
                continue;
            }
            if graph.opcode(node) == ISD::CopyFromReg && virtual_register(graph, graph.ops(node)[1]) {
                continue;
            }
            for pred_succ in graph.sus[pred].succs.clone() {
                let other = pred_succ.su;
                if other == su {
                    continue;
                }
                if graph.sus[other].num_succs == 0 {
                    continue 'unit;
                }
                if graph.sus[su].has_phys_reg_clobbers
                    && graph.sus[other].has_phys_reg_defs
                    && graph.can_clobber_phys_reg_defs(other, su)
                {
                    continue 'unit;
                }
                if topo.is_reachable(&graph.sus, su, other) {
                    continue 'unit;
                }
            }
            let mut i = 0;
            while i < graph.sus[pred].succs.len() {
                let edge = graph.sus[pred].succs[i];
                let succ = edge.su;
                if succ != su {
                    // `succ` stops depending on `pred` and depends on `su`,
                    // which takes `pred`'s place beside it.
                    graph.remove_pred(succ, Dep { su: pred, ..edge });
                    topo.add_pred_queued(su, pred);
                    graph.add_pred(su, Dep { su: pred, ..edge });
                    topo.add_pred_queued(succ, su);
                    graph.add_pred(succ, Dep { su, ..edge });
                    continue;
                }
                i += 1;
            }
        }
    }

    /// `CalcNodeSethiUllmanNumber`, iteratively.
    fn calc_sethi_ullman(
        &mut self,
        sus: &[SUnit],
        su: usize,
    ) {
        if self.sethi_ullman[su] != 0 {
            return;
        }
        let mut work = std::mem::take(&mut self.walk);
        work.push((su, 0usize));
        while let Some(&(cur, processed)) = work.last() {
            let mut known = true;
            for p in processed..sus[cur].preds.len() {
                let pred = sus[cur].preds[p];
                if pred.is_ctrl() {
                    continue;
                }
                if self.sethi_ullman[pred.su] == 0 {
                    work.last_mut().expect("nonempty").1 = p + 1;
                    work.push((pred.su, 0));
                    known = false;
                    break;
                }
            }
            if !known {
                continue;
            }
            let (mut number, mut extra) = (0, 0);
            for pred in sus[cur].preds.iter().filter(|p| !p.is_ctrl()) {
                let theirs = self.sethi_ullman[pred.su];
                if theirs > number {
                    number = theirs;
                    extra = 0;
                } else if theirs == number {
                    extra += 1;
                }
            }
            self.sethi_ullman[cur] = (number + extra).max(1);
            work.pop();
        }
        self.walk = work;
    }

    /// `getNodePriority`.
    fn node_priority(
        &self,
        graph: &Graph<'_>,
        su: usize,
    ) -> u32 {
        let unit = &graph.sus[su];
        if matches!(graph.opcode(unit.node()), ISD::TokenFactor | ISD::CopyToReg) {
            return 0;
        }
        if unit.num_succs == 0 && unit.num_preds != 0 {
            return 0xffff;
        }
        if unit.num_preds == 0 && unit.num_succs != 0 {
            return 0;
        }
        self.sethi_ullman[su]
    }

    fn push(
        &mut self,
        sus: &mut [SUnit],
        su: usize,
    ) {
        debug_assert_eq!(sus[su].queue_id, 0, "a unit is in the queue already");
        self.cur_queue_id += 1;
        sus[su].queue_id = self.cur_queue_id;
        self.queue.push(su);
    }

    /// `popFromQueue`: the best of the first 1000 by `bu_ls_rr_sort`.
    fn pop(
        &mut self,
        graph: &mut Graph<'_>,
    ) -> Option<usize> {
        if self.queue.is_empty() {
            return None;
        }
        let mut best = 0;
        for i in 1..self.queue.len().min(1000) {
            if self.bu_ls_rr_sort(graph, self.queue[best], self.queue[i]) {
                best = i;
            }
        }
        let su = self.queue.swap_remove(best);
        graph.sus[su].queue_id = 0;
        Some(su)
    }

    /// `getNodeOrdering`: the IR order of the unit's node under `SrcOrder`.
    fn node_ordering(
        graph: &Graph<'_>,
        su: usize,
    ) -> u32 {
        if preference(graph.info) == Preference::Burr {
            return 0;
        }
        graph.dag.node(SDValue { node: graph.sus[su].node(), res: 0 }).order
    }

    /// `bu_ls_rr_sort`, `src_ls_rr_sort` or `ilp_ls_rr_sort`: whether `right`
    /// goes before `left`. Source order puts the lower non-zero IR order
    /// first; `BURRSort` breaks ties.
    fn bu_ls_rr_sort(
        &self,
        graph: &mut Graph<'_>,
        left: usize,
        right: usize,
    ) -> bool {
        // checkSpecialNodes
        let (l_low, r_low) = (graph.sus[left].is_schedule_low, graph.sus[right].is_schedule_low);
        if l_low != r_low {
            return l_low < r_low;
        }
        match preference(graph.info) {
            Preference::Burr => {}
            Preference::Source => {
                let (l_order, r_order) = (Self::node_ordering(graph, left), Self::node_ordering(graph, right));
                if (l_order != 0 || r_order != 0) && l_order != r_order {
                    return l_order != 0 && (l_order < r_order || r_order == 0);
                }
            }
            Preference::Ilp => {
                if let Some(first) = self.ilp_ls_rr_sort(graph, left, right) {
                    return first;
                }
            }
        }
        self.burr_sort(graph, left, right)
    }

    /// `ilp_ls_rr_sort` less `checkSpecialNodes` and the closing `BURRSort`,
    /// which the caller runs: `Some` where a rule before it decides.
    ///
    /// The pin's defaults are in: register-pressure difference, critical
    /// path and height on; live uses and stalls off.
    fn ilp_ls_rr_sort(
        &self,
        graph: &mut Graph<'_>,
        left: usize,
        right: usize,
    ) -> Option<bool> {
        // No way to compute latency of calls.
        if graph.sus[left].is_call || graph.sus[right].is_call {
            return None;
        }
        let (l_diff, r_diff) = (graph.reg_pressure_diff(left), graph.reg_pressure_diff(right));
        if l_diff != r_diff {
            return Some(l_diff > r_diff);
        }
        if l_diff > 0 || r_diff > 0 {
            let (l_reduce, r_reduce) = (can_enable_coalescing(graph, left), can_enable_coalescing(graph, right));
            if l_reduce && !r_reduce {
                return Some(false);
            }
            if r_reduce && !l_reduce {
                return Some(true);
            }
        }
        let (l_depth, r_depth) = (graph.depth(left), graph.depth(right));
        if l_depth.abs_diff(r_depth) > MAX_REORDER_WINDOW {
            return Some(l_depth < r_depth);
        }
        let (l_height, r_height) = (graph.height(left), graph.height(right));
        if l_height != r_height && l_height.abs_diff(r_height) > MAX_REORDER_WINDOW {
            return Some(l_height > r_height);
        }
        None
    }

    /// `BURRSort`.
    fn burr_sort(
        &self,
        graph: &mut Graph<'_>,
        left: usize,
        right: usize,
    ) -> bool {
        let (l_phys, r_phys) = (graph.sus[left].has_phys_reg_defs, graph.sus[right].has_phys_reg_defs);
        if l_phys != r_phys {
            return l_phys < r_phys;
        }
        let mut l_priority = self.node_priority(graph, left);
        let mut r_priority = self.node_priority(graph, right);
        if graph.sus[left].is_call && graph.sus[right].is_call_op {
            let values = graph.values(graph.sus[right].node()).len() as u32;
            r_priority = r_priority.saturating_sub(values);
        }
        if graph.sus[right].is_call && graph.sus[left].is_call_op {
            let values = graph.values(graph.sus[left].node()).len() as u32;
            l_priority = l_priority.saturating_sub(values);
        }
        if l_priority != r_priority {
            return l_priority > r_priority;
        }
        if graph.sus[left].is_call || graph.sus[right].is_call {
            let (l_order, r_order) = (Self::node_ordering(graph, left), Self::node_ordering(graph, right));
            if (l_order != 0 || r_order != 0) && l_order != r_order {
                return l_order != 0 && (l_order < r_order || r_order == 0);
            }
        }
        let (l_dist, r_dist) = (graph.closest_succ(left), graph.closest_succ(right));
        if l_dist != r_dist {
            return l_dist < r_dist;
        }
        let scratches = |g: &Graph<'_>, su: usize| g.sus[su].preds.iter().filter(|p| !p.is_ctrl()).count();
        let (l_scratch, r_scratch) = (scratches(graph, left), scratches(graph, right));
        if l_scratch != r_scratch {
            return l_scratch > r_scratch;
        }
        if (graph.sus[left].is_call && r_priority > 0) || (graph.sus[right].is_call && l_priority > 0) {
            return graph.sus[left].queue_id > graph.sus[right].queue_id;
        }
        if !(graph.sus[left].is_call || graph.sus[right].is_call) {
            // BUCompareLatency(checkPref = false), without a virtual-register
            // cycle (SD30) or a hazard recognizer.
            let (l_height, r_height) = (graph.height(left), graph.height(right));
            let cycle = graph.cur_cycle;
            let (l_stall, r_stall) = (cycle < l_height, cycle < r_height);
            if l_stall {
                if !r_stall {
                    return true;
                }
                if l_height != r_height {
                    return l_height > r_height;
                }
            } else if r_stall {
                return false;
            }
            if l_height != r_height {
                return l_height > r_height;
            }
            let (l_depth, r_depth) = (graph.depth(left), graph.depth(right));
            if l_depth != r_depth {
                return l_depth < r_depth;
            }
            let (l_latency, r_latency) = (graph.sus[left].latency, graph.sus[right].latency);
            if l_latency != r_latency {
                return l_latency > r_latency;
            }
        } else {
            let (l_height, r_height) = (graph.height(left), graph.height(right));
            if l_height != r_height {
                return l_height > r_height;
            }
            let (l_depth, r_depth) = (graph.depth(left), graph.depth(right));
            if l_depth != r_depth {
                return l_depth < r_depth;
            }
        }
        graph.sus[left].queue_id > graph.sus[right].queue_id
    }
}

/// `-max-sched-reorder`: the instructions the ILP scheduler allows ahead of
/// the critical path.
const MAX_REORDER_WINDOW: u32 = 6;

/// `canEnableCoalescing`: a unit best kept close to its uses.
fn can_enable_coalescing(
    graph: &Graph<'_>,
    su: usize,
) -> bool {
    let unit = &graph.sus[su];
    // `EXTRACT_SUBREG`, `INSERT_SUBREG` and `SUBREG_TO_REG` are machine nodes
    // here: their opcodes never equal the generic ones (SD30).
    matches!(graph.opcode(unit.node()), ISD::TokenFactor | ISD::CopyToReg)
        // no register def: it lengthens no live range
        || (unit.num_preds == 0 && unit.num_succs != 0)
}

/// `cast<RegisterSDNode>(reg)->getReg().isVirtual()`: a `Register` node
/// holds a virtual register, a `PhysReg` node a physical one.
fn virtual_register(
    graph: &Graph<'_>,
    reg: SDValue,
) -> bool {
    matches!(graph.dag.node(reg).data, NodeData::Register(_))
}
