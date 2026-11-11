//! A function as the arm64 backend holds it between instruction selection and
//! encoding: blocks of machine instructions over virtual registers
//! (`MachineFunction`), the frame objects it needs, and the pseudo
//! instructions only the backend's own phases understand.

use llrm_codegen::model::lir::SrcLoc;
use rustc_hash::FxHashMap as HashMap;

use crate::mc::inst::{Gpr, Inst, Target, Vreg};
use crate::mc::visit::{Reg, RegList, RegRef, Role};

/// A machine instruction: a real one, or a pseudo that a later phase turns into
/// real ones (LLVM's pseudo instructions and `COPY`).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Op {
    I(Inst),
    /// `dst = src`, registers of one class (LLVM's `COPY`): `mov` or `fmov`
    /// once registers are assigned, nothing if they are the same.
    Copy {
        dst: Reg,
        src: Reg,
    },
    /// `dst = value` in a `bits`-wide (32 or 64) register: `MOVi32imm` and
    /// `MOVi64imm`, expanded to the shortest `movz`/`movk`/`orr` sequence.
    Imm {
        dst: Gpr,
        bits: u32,
        value: u64,
    },
    /// A call: `uses` are the argument registers it reads (physical), `defs`
    /// the result registers it writes; every caller-saved register is
    /// clobbered.
    Call {
        callee: Callee,
        uses: Vec<Reg>,
        defs: Vec<Reg>,
        /// The IR call is marked `tail` and takes no stack arguments: a
        /// candidate for [`crate::sibcall`].
        tail: bool,
    },
    /// A return reading the result registers; or, with `tail`, the jump to
    /// that callee, reading the argument registers (`uses`).
    Return {
        uses: Vec<Reg>,
        tail: Option<Callee>,
    },
    /// `dst` = the address of a frame object plus `offset`, once the frame is
    /// laid out (`ADDXri` on a frame index).
    FrameAddress {
        dst: Gpr,
        slot: u32,
        offset: i64,
    },
    /// The jump to entry `index` of jump table `id` (`JumpTableDest32`): `dst`
    /// is the entry's address, `scratch` holds the loaded offset. `table` is
    /// the table's address. Both results are written before the inputs are
    /// dead, so neither may share a register with them.
    JumpTable {
        dst: Gpr,
        scratch: Gpr,
        table: Gpr,
        index: Gpr,
        id: u32,
        /// The labels of the blocks the entries go to; the table's successors.
        targets: Vec<String>,
    },
    /// `DBG_VALUE`: from here on the source variable `variable` is the value
    /// in `reg` (`llvm.dbg.value`); none once nothing keeps the value. It is
    /// no code and no use: liveness, allocation and every pass that looks at
    /// neighbouring operations step over it, and the allocator renames it.
    DbgValue {
        variable: llrm_mir::MetadataId,
        reg: Option<Reg>,
    },
    /// `dst = PHI [value, predecessor]..`, first in its block: the value it
    /// takes from each predecessor, named by that block's label. Selection
    /// makes it (as the pin's ISel does) and [`lower_phis`] turns it into
    /// copies (`PHIElimination`); no later phase sees one.
    Phi {
        dst: Reg,
        incoming: Vec<(Reg, String)>,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Callee {
    Symbol(String),
    Register(Gpr),
}

impl Op {
    /// `f` on each register operand with its role.
    pub fn visit(
        &mut self,
        f: &mut dyn FnMut(RegRef<'_>, Role),
    ) {
        match self {
            Op::I(inst) => inst.visit(f),
            Op::Copy { dst, src } => {
                visit_reg(dst, Role::Def, f);
                visit_reg(src, Role::Use, f);
            }
            Op::Imm { dst, .. } | Op::FrameAddress { dst, .. } => f(RegRef::Gpr(dst), Role::Def),
            Op::JumpTable { dst, scratch, table, index, .. } => {
                f(RegRef::Gpr(table), Role::Use);
                f(RegRef::Gpr(index), Role::Use);
                f(RegRef::Gpr(dst), Role::Def);
                f(RegRef::Gpr(scratch), Role::Def);
            }
            Op::Call { callee, uses, defs, .. } => {
                if let Callee::Register(register) = callee {
                    f(RegRef::Gpr(register), Role::Use);
                }
                for register in uses {
                    visit_reg(register, Role::Use, f);
                }
                for register in defs {
                    visit_reg(register, Role::Def, f);
                }
            }
            Op::Return { uses, tail } => {
                if let Some(Callee::Register(register)) = tail {
                    f(RegRef::Gpr(register), Role::Use);
                }
                for register in uses {
                    visit_reg(register, Role::Use, f);
                }
            }
            Op::Phi { dst, incoming } => {
                visit_reg(dst, Role::Def, f);
                for (value, _) in incoming {
                    visit_reg(value, Role::Use, f);
                }
            }
            // Not a use: the allocator renames it by hand
            // (`regalloc::rewrite`).
            Op::DbgValue { .. } => {}
        }
    }

    /// Whether the operation is debug information only
    /// (`MachineInstr::isDebugInstr`).
    pub fn is_debug(&self) -> bool {
        matches!(self, Op::DbgValue { .. })
    }

    pub fn defs_and_uses(&self) -> (RegList, RegList) {
        let (mut defs, mut uses) = (RegList::new(), RegList::new());
        self.for_each_register(&mut |key, role| {
            if role.writes() {
                defs.push(key);
            }
            if role.reads() {
                uses.push(key);
            }
        });
        (defs, uses)
    }

    /// `f` on each register operand with its role, without collecting them:
    /// the passes that walk every operation of a function many times ask this.
    pub fn for_each_register(
        &self,
        f: &mut dyn FnMut(Reg, Role),
    ) {
        self.clone().visit(&mut |register, role| f(register.key(), role));
    }
}

fn visit_reg(
    register: &mut Reg,
    role: Role,
    f: &mut dyn FnMut(RegRef<'_>, Role),
) {
    match register {
        Reg::Gpr(inner) => {
            let mut copy = *inner;
            f(RegRef::Gpr(&mut copy), role);
            *inner = copy;
        }
        Reg::Fpr(inner) => {
            let mut copy = *inner;
            f(RegRef::Fpr(&mut copy), role);
            *inner = copy;
        }
    }
}

/// A basic block: a label and its instructions, the last of which is a branch
/// or a return.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MBlock {
    pub label: String,
    pub ops: OpList,
}

/// A block's operations, each with the source place it was selected from
/// (LLVM's `DebugLoc` on a `MachineInstr`). Reading is the slice's; what adds,
/// drops or moves an operation says what becomes of its place, so the two never
/// part.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpList {
    ops: Vec<Op>,
    locs: Vec<Option<SrcLoc>>,
}

impl std::ops::Deref for OpList {
    type Target = [Op];

    fn deref(&self) -> &[Op] {
        &self.ops
    }
}

impl From<Vec<Op>> for OpList {
    fn from(ops: Vec<Op>) -> Self {
        let locs = vec![None; ops.len()];
        Self { ops, locs }
    }
}

impl<'a> IntoIterator for &'a OpList {
    type IntoIter = std::slice::Iter<'a, Op>;
    type Item = &'a Op;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.iter()
    }
}

impl<'a> IntoIterator for &'a mut OpList {
    type IntoIter = std::slice::IterMut<'a, Op>;
    type Item = &'a mut Op;

    /// Each operation to change in place: its place stays.
    fn into_iter(self) -> Self::IntoIter {
        self.ops.iter_mut()
    }
}

impl IntoIterator for OpList {
    type IntoIter = std::iter::Zip<std::vec::IntoIter<Op>, std::vec::IntoIter<Option<SrcLoc>>>;
    type Item = (Op, Option<SrcLoc>);

    fn into_iter(self) -> Self::IntoIter {
        self.ops.into_iter().zip(self.locs)
    }
}

impl FromIterator<(Op, Option<SrcLoc>)> for OpList {
    fn from_iter<I: IntoIterator<Item = (Op, Option<SrcLoc>)>>(iter: I) -> Self {
        let (ops, locs) = iter.into_iter().unzip();
        Self { ops, locs }
    }
}

impl OpList {
    /// `op`, from no place.
    pub fn push(
        &mut self,
        op: Op,
    ) {
        self.push_at(op, None);
    }

    pub fn push_at(
        &mut self,
        op: Op,
        loc: Option<SrcLoc>,
    ) {
        self.ops.push(op);
        self.locs.push(loc);
    }

    /// `op` before the operation at `at`, from the place that operation is from
    /// (code the allocator or a pass adds stands where its neighbour does).
    pub fn insert(
        &mut self,
        at: usize,
        op: Op,
    ) {
        let loc = self.locs.get(at).copied().or_else(|| self.locs.last().copied()).flatten();
        self.insert_at(at, op, loc);
    }

    pub fn insert_at(
        &mut self,
        at: usize,
        op: Op,
        loc: Option<SrcLoc>,
    ) {
        self.ops.insert(at, op);
        self.locs.insert(at, loc);
    }

    /// The operation at `at`, to change in place: its place stays.
    pub fn op_mut(
        &mut self,
        at: usize,
    ) -> &mut Op {
        &mut self.ops[at]
    }

    pub fn remove(
        &mut self,
        at: usize,
    ) -> (Op, Option<SrcLoc>) {
        (self.ops.remove(at), self.locs.remove(at))
    }

    pub fn truncate(
        &mut self,
        len: usize,
    ) {
        self.ops.truncate(len);
        self.locs.truncate(len);
    }

    pub fn retain(
        &mut self,
        mut keep: impl FnMut(&Op) -> bool,
    ) {
        let mut at = 0;
        let locs = &mut self.locs;
        self.ops.retain(|op| {
            let kept = keep(op);
            if !kept {
                locs.remove(at);
            } else {
                at += 1;
            }
            kept
        });
    }

    /// The operations in `range` in the order of `key` (stable), each keeping
    /// its place.
    pub fn sort_by_key<K: Ord>(
        &mut self,
        range: std::ops::Range<usize>,
        mut key: impl FnMut(&Op) -> K,
    ) {
        let mut pairs: Vec<(Op, Option<SrcLoc>)> =
            self.ops[range.clone()].iter().cloned().zip(self.locs[range.clone()].iter().copied()).collect();
        pairs.sort_by_key(|(op, _)| key(op));
        for (offset, (op, loc)) in pairs.into_iter().enumerate() {
            self.ops[range.start + offset] = op;
            self.locs[range.start + offset] = loc;
        }
    }

    /// The place the operation at `at` is from.
    pub fn loc(
        &self,
        at: usize,
    ) -> Option<SrcLoc> {
        self.locs.get(at).copied().flatten()
    }

    /// Every operation with its place, leaving the list empty.
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    /// The operations as a plain list, the places left behind.
    pub fn into_ops(self) -> Vec<Op> {
        self.ops
    }
}

/// A frame object: a local the function addresses, or a spill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot {
    pub size: u64,
    pub align: u64,
}

/// Where each value of a function lives before registers are assigned.
#[derive(Clone, Debug, Default)]
pub struct MFunc {
    pub name: String,
    pub blocks: Vec<MBlock>,
    pub slots: Vec<Slot>,
    /// The next virtual register number, of either class.
    pub next_register: u32,
    /// Float registers the selector knows hold 128 bits (`f128` values), for
    /// the allocator to spill and copy whole.
    pub wide: Vec<Reg>,
    /// The most bytes of outgoing arguments any call passes on the stack.
    pub outgoing: u64,
    /// Whether the function calls another (it saves the link register).
    pub calls: bool,
    /// The bytes of incoming arguments the caller put on the stack are above
    /// the frame at this offset from the frame pointer.
    pub exported: bool,
    /// The stack pointer moves in the body (`alloca` of a run-time size): frame
    /// objects are addressed from the frame pointer and outgoing arguments are
    /// made room for around each call.
    pub dynamic: bool,
    /// The function's `"frame-pointer"` attribute (`all`, `non-leaf`, ...).
    pub frame_pointer: Option<String>,
    /// The unwind table the function gets.
    pub unwind: crate::cfi::Unwind,
    /// The frame object each source variable lives in (`llvm.dbg.declare` of
    /// an `alloca`), by the variable's metadata node.
    pub declared: Vec<(llrm_mir::MetadataId, u32)>,
    /// The constants the function loads from memory (`MachineConstantPool`),
    /// each under the label its address is taken from.
    pub pool: Vec<PoolEntry>,
    /// The selector's block id of each block it made, by label (a block a
    /// later pass made has none; a label, unlike a position, survives the
    /// passes that remove and insert blocks), and the branch probabilities it
    /// kept for the edges it split (`(from, to, numerator / 2^31)`); empty
    /// when the selector kept none.
    pub block_ids: HashMap<String, u32>,
    pub edge_probs: Vec<(u32, u32, u32)>,
    /// The critical edges `lower_phis` split: `(from, to, the block made)`, by
    /// label. The block odds of the edge it replaced are the new block's.
    pub split_edges: Vec<(String, String, String)>,
    /// The IR's edge odds the allocator reads (`MachineBranchProbabilityInfo`);
    /// set just before allocation.
    pub alloc_odds: Option<llrm_spike_mf::placement::Odds>,
}

/// A constant pool entry: `bytes` at `align`, under `label`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolEntry {
    pub label: String,
    pub bytes: Vec<u8>,
    pub align: usize,
}

impl MFunc {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_owned(), next_register: Gpr::FIRST_VIRTUAL, ..Self::default() }
    }

    pub fn gpr(&mut self) -> Gpr {
        let number = self.next_register;
        self.next_register += 1;
        Gpr(number)
    }

    pub fn fpr(&mut self) -> Vreg {
        let number = self.next_register;
        self.next_register += 1;
        Vreg(number)
    }

    pub fn slot(
        &mut self,
        size: u64,
        align: u64,
    ) -> u32 {
        self.slots.push(Slot { size, align });
        (self.slots.len() - 1) as u32
    }
}

/// The frame object that stands for the caller's outgoing argument area: an
/// `Addr::Slot` of it is `offset` bytes above the frame record.
pub const INCOMING_ARGUMENTS: u32 = u32::MAX;

/// The label of the `n`th block of `function`: local to the object.
pub fn block_label(
    function: &str,
    n: usize,
) -> String {
    format!(".L{function}.{n}")
}

pub fn branch_to(label: &str) -> Op {
    Op::I(Inst::Branch { link: false, target: Target::Label(label.to_owned()) })
}

/// Whether control leaves `block` by running into the block after it: nothing
/// in it jumps unconditionally, returns, indexes a table or traps.
pub fn falls_through(block: &MBlock) -> bool {
    !block
        .ops
        .iter()
        .any(
            |op| matches!(
                op,
                Op::I(Inst::Branch { link: false, target: Target::Label(_) })
                    | Op::Return { .. }
                    | Op::JumpTable { .. }
                    | Op::I(Inst::Brk { .. })
            ),
        )
}

/// The blocks `block` (the `at`th of `count`) can go to, from the labels its
/// branches name and its fall-through.
pub fn successors_of(
    block: &MBlock,
    at: usize,
    count: usize,
    labels: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut out = Vec::new();
    for op in &block.ops {
        match op {
            Op::I(Inst::Branch { link: false, target: Target::Label(label) }) => {
                out.extend(labels.get(label.as_str()).copied());
            }
            Op::I(Inst::BranchCond { target: Target::Label(label), .. })
            | Op::I(Inst::CompareBranch { target: Target::Label(label), .. })
            | Op::I(Inst::TestBranch { target: Target::Label(label), .. }) => {
                out.extend(labels.get(label.as_str()).copied());
            }
            Op::JumpTable { targets, .. } => {
                out.extend(targets.iter().filter_map(|label| labels.get(label.as_str()).copied()));
            }
            _ => {}
        }
    }
    if falls_through(block) && at + 1 < count {
        out.push(at + 1);
    }
    out
}

/// The copies `moves` (`dst, src`) as a parallel copy: no copy reads a register
/// an earlier one wrote, and a cycle is broken through a register made by
/// `temporary`.
pub fn sequence_copies(
    mut moves: Vec<(Reg, Reg)>,
    mut temporary: impl FnMut(Reg) -> Reg,
) -> Vec<Op> {
    let mut out = Vec::new();
    while !moves.is_empty() {
        // A move whose destination no other move reads.
        match moves.iter().position(|(dst, _)| !moves.iter().any(|(_, src)| src == dst)) {
            Some(at) => {
                let (dst, src) = moves.remove(at);
                out.push(Op::Copy { dst, src });
            }
            None => {
                // A cycle: park one source in a new register.
                let (_, src) = moves[0];
                let parked = temporary(src);
                out.push(Op::Copy { dst: parked, src });
                for (_, reader) in moves.iter_mut() {
                    if *reader == src {
                        *reader = parked;
                    }
                }
            }
        }
    }
    out
}

/// `UnreachableMachineBlockElim`: the blocks no path from the entry reaches are deleted, and the phi operands that
/// came from them. Selection makes one for every IR block, such as the `unreachable` default of a `switch`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/UnreachableBlockElim.cpp>
pub fn remove_unreachable_blocks(function: &mut MFunc) {
    let count = function.blocks.len();
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let mut reached = vec![false; count];
    let mut pending = vec![0usize];
    while let Some(at) = pending.pop() {
        if at >= count || std::mem::replace(&mut reached[at], true) {
            continue;
        }
        pending.extend(successors_of(&function.blocks[at], at, count, &labels));
    }
    if reached.iter().all(|&one| one) {
        return;
    }
    let dead: rustc_hash::FxHashSet<String> = function
        .blocks
        .iter()
        .zip(&reached)
        .filter(|(_, reached)| !**reached)
        .map(|(block, _)| block.label.clone())
        .collect();
    let mut at = 0;
    function.blocks.retain(|_| {
        at += 1;
        reached[at - 1]
    });
    for block in &mut function.blocks {
        for op in &mut block.ops {
            if let Op::Phi { incoming, .. } = op {
                incoming.retain(|(_, from)| !dead.contains(from));
            }
        }
    }
}

/// Phi elimination (`PHIElimination::LowerPHINode`,
/// llvm/lib/CodeGen/PHIElimination.cpp): each phi `d = phi [v, p]..` gets a
/// register `i` of its own; `i = COPY v` goes at the end of each predecessor
/// `p`, before its branches, and `d = COPY i` at the top of the phi's block. A
/// predecessor with several successors gets a block on the edge for its copies
/// (laid out right after its source, as `SplitCriticalEdge` does), so a copy
/// never runs on a path that does not take the phi.
///
/// DEVIATE ARM64-PHI-SPLIT (owner arm64-phi, exit 2026-11-24, gap 3 of
/// docs/architecture/arm64-spike.md): every such edge is split,
/// where the pin splits by liveness (`SplitPHIEdges`). Selection used to split
/// the same edges, so the code is the same. The registers of the phis' edges
/// are numbered here, after selection, in the pin's order (`LowerPHINode`:
/// [L372](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L372),
/// [L415](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L415),
/// [L620](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L620)),
/// so an exact priority tie in the greedy allocator (lower vreg index first)
/// can fall the other way than before: jpeg_single -O3 is +36 B and +0.054%
/// executed for that reason (2026-11-09, closed as pin-faithful tie-order).
pub fn lower_phis(function: &mut MFunc) -> Result<(), String> {
    let count = function.blocks.len();
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    // The edges' copies `(via, value)` by `(predecessor, block)`, in phi order.
    let mut edge_copies: HashMap<(usize, usize), Vec<(Reg, Reg)>> = HashMap::default();
    let mut heads: Vec<Vec<(Reg, Reg)>> = vec![Vec::new(); count];
    let mut phis: Vec<Vec<(Reg, Vec<(Reg, String)>)>> = vec![Vec::new(); count];
    for (at, block) in function.blocks.iter().enumerate() {
        for op in block.ops.iter().take_while(|op| matches!(op, Op::Phi { .. })) {
            let Op::Phi { dst, incoming } = op else { unreachable!("filtered to phis") };
            phis[at].push((*dst, incoming.clone()));
        }
    }
    let labels: HashMap<String, usize> = labels.into_iter().map(|(label, at)| (label.to_owned(), at)).collect();
    for (at, block_phis) in phis.iter().enumerate() {
        for (dst, incoming) in block_phis {
            let via = match dst {
                Reg::Gpr(_) => Reg::Gpr(function.gpr()),
                Reg::Fpr(_) => Reg::Fpr(function.fpr()),
            };
            heads[at].push((*dst, via));
            for (value, from) in incoming {
                let from = *labels.get(from).ok_or_else(|| format!("a phi names the unknown block {from}"))?;
                edge_copies.entry((from, at)).or_default().push((via, *value));
            }
        }
    }
    for (at, block) in function.blocks.iter_mut().enumerate() {
        if phis[at].is_empty() {
            continue;
        }
        let skip = phis[at].len();
        let rest = std::mem::take(&mut block.ops).into_iter().skip(skip);
        block.ops = heads[at].iter().map(|&(dst, via)| (Op::Copy { dst, src: via }, None)).chain(rest).collect();
    }
    if edge_copies.is_empty() {
        return Ok(());
    }
    let by_label: HashMap<&str, usize> = labels.iter().map(|(label, &at)| (label.as_str(), at)).collect();
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(at, block)| {
            let mut out = successors_of(block, at, count, &by_label);
            let mut seen = Vec::new();
            out.retain(|to| {
                let fresh = !seen.contains(to);
                seen.push(*to);
                fresh
            });
            out
        })
        .collect();
    let is_branch = |op: &Op| {
        matches!(
            op,
            Op::I(Inst::Branch { .. } | Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. })
        )
    };
    let mut edges: Vec<(usize, MBlock)> = Vec::new();
    for (from, targets) in successors.iter().enumerate() {
        let carrying: Vec<usize> =
            targets.iter().copied().filter(|to| edge_copies.contains_key(&(from, *to))).collect();
        if carrying.is_empty() {
            continue;
        }
        let copies = |to: usize| -> Vec<Op> {
            edge_copies[&(from, to)].iter().map(|&(via, value)| Op::Copy { dst: via, src: value }).collect()
        };
        if targets.len() == 1 {
            let ops = &mut function.blocks[from].ops;
            let first_branch = ops.iter().rposition(|op| !is_branch(op)).map_or(0, |last| last + 1);
            for (offset, copy) in copies(carrying[0]).into_iter().enumerate() {
                ops.insert(first_branch + offset, copy);
            }
            continue;
        }
        for to in carrying {
            let label = function.blocks[to].label.clone();
            let edge = format!(".L{}.e{}", function.name, edges.len());
            let mut body = copies(to);
            body.push(branch_to(&label));
            edges.push((from, MBlock { label: edge.clone(), ops: body.into() }));
            function.split_edges.push((function.blocks[from].label.clone(), label.clone(), edge.clone()));
            // Retarget the branches to `to`; a fall-through becomes a branch.
            let mut retargeted = false;
            for op in &mut function.blocks[from].ops {
                match op {
                    Op::I(
                        Inst::Branch { target, .. }
                        | Inst::BranchCond { target, .. }
                        | Inst::CompareBranch { target, .. }
                        | Inst::TestBranch { target, .. },
                    ) if *target == Target::Label(label.clone()) => {
                        *target = Target::Label(edge.clone());
                        retargeted = true;
                    }
                    Op::JumpTable { targets, .. } => {
                        for entry in targets.iter_mut().filter(|entry| **entry == label) {
                            *entry = edge.clone();
                            retargeted = true;
                        }
                    }
                    _ => {}
                }
            }
            if !retargeted {
                function.blocks[from].ops.push(branch_to(&edge));
            }
        }
    }
    // Each block lies right after its source (`SplitCriticalEdge` inserts it
    // at `std::next(this)`, MachineBasicBlock.cpp#L1217): the later split of a
    // source is nearer to it.
    let originals = std::mem::take(&mut function.blocks);
    let mut placed = Vec::with_capacity(originals.len() + edges.len());
    let next_labels: Vec<Option<String>> =
        (0..originals.len()).map(|at| originals.get(at + 1).map(|next| next.label.clone())).collect();
    for (at, mut block) in originals.into_iter().enumerate() {
        // The blocks put after it come between it and the block it ran into.
        if edges.iter().any(|(from, _)| *from == at) && falls_through(&block) {
            if let Some(next) = &next_labels[at] {
                block.ops.push(branch_to(next));
            }
        }
        placed.push(block);
        for (_, edge) in edges.iter().filter(|(from, _)| *from == at).rev() {
            placed.push(edge.clone());
        }
    }
    function.blocks = placed;
    Ok(())
}

/// The virtual registers some path reads before writing: a phi copy that
/// missed an edge leaves one (`LLRM_ARM64_VERIFY`).
pub fn undefined_reads(function: &MFunc) -> Vec<Reg> {
    use llrm_support::hash::HashSet;
    let labels: HashMap<&str, usize> =
        function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let count = function.blocks.len();
    let successors: Vec<Vec<usize>> =
        function.blocks.iter().enumerate().map(|(at, block)| successors_of(block, at, count, &labels)).collect();
    let mut predecessors = vec![Vec::new(); count];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    let all: HashSet<Reg> = function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| {
            let (defs, _) = op.defs_and_uses();
            defs
        })
        .filter(|register| register.is_virtual())
        .collect();
    let mut out: Vec<HashSet<Reg>> = vec![all.clone(); count];
    let walk = |block: &MBlock, mut defined: HashSet<Reg>, report: &mut Vec<Reg>| {
        for op in &block.ops {
            let (defs, uses) = op.defs_and_uses();
            report.extend(uses.into_iter().filter(|register| register.is_virtual() && !defined.contains(register)));
            defined.extend(defs);
        }
        defined
    };
    let mut changed = true;
    while changed {
        changed = false;
        for at in 0..count {
            let entering: HashSet<Reg> = if at == 0 {
                HashSet::default()
            } else {
                let mut from = predecessors[at].iter().map(|&pred| &out[pred]);
                match from.next() {
                    Some(first) => from.fold(first.clone(), |acc, next| acc.intersection(next).copied().collect()),
                    None => all.clone(),
                }
            };
            let leaving = walk(&function.blocks[at], entering, &mut Vec::new());
            if leaving != out[at] {
                out[at] = leaving;
                changed = true;
            }
        }
    }
    let mut report = Vec::new();
    for (at, block) in function.blocks.iter().enumerate() {
        let entering: HashSet<Reg> = if at == 0 {
            HashSet::default()
        } else {
            let mut from = predecessors[at].iter().map(|&pred| &out[pred]);
            match from.next() {
                Some(first) => from.fold(first.clone(), |acc, next| acc.intersection(next).copied().collect()),
                None => all.clone(),
            }
        };
        walk(block, entering, &mut report);
    }
    report.sort();
    report.dedup();
    report
}

#[cfg(test)]
mod verify_tests {
    use super::*;

    fn block(
        label: &str,
        ops: Vec<Op>,
    ) -> MBlock {
        MBlock { label: label.into(), ops: ops.into() }
    }

    /// A register written on one path only is a read before a write: the
    /// instrument that found `place`'s missing phi copies.
    #[test]
    fn a_register_written_on_one_path_is_reported() {
        let mut function = MFunc::new("f");
        let (a, b) = (function.gpr(), function.gpr());
        let branch = Op::I(Inst::BranchCond { cond: crate::mc::inst::Cond::Eq, target: Target::Label("join".into()) });
        function
            .blocks
            .push(
                block(
                    "entry",
                    vec![Op::Copy { dst: Reg::Gpr(a), src: Reg::Gpr(Gpr(0)) }, branch, branch_to("then")],
                ),
            );
        function
            .blocks
            .push(block("then", vec![Op::Copy { dst: Reg::Gpr(b), src: Reg::Gpr(Gpr(1)) }, branch_to("join")]));
        function
            .blocks
            .push(
                block(
                    "join",
                    vec![Op::Copy { dst: Reg::Gpr(Gpr(0)), src: Reg::Gpr(b) }, Op::Return { uses: vec![], tail: None }],
                ),
            );
        assert_eq!(undefined_reads(&function), vec![Reg::Gpr(b)]);
        function.blocks[0].ops.insert(0, Op::Copy { dst: Reg::Gpr(b), src: Reg::Gpr(Gpr(1)) });
        assert_eq!(undefined_reads(&function), vec![]);
    }
}

/// The registers `function` leaves live: those it returns and passes to a tail
/// call.
pub fn returned_registers(function: &MFunc) -> Vec<Reg> {
    let mut out = Vec::new();
    for block in &function.blocks {
        for op in block.ops.iter() {
            if let Op::Return { uses, tail } = op {
                out.extend(uses.iter().copied());
                if let Some(Callee::Register(register)) = tail {
                    out.push(Reg::Gpr(*register));
                }
            }
        }
    }
    out
}
