//! A function as the arm64 backend holds it between instruction selection and
//! encoding: blocks of machine instructions over virtual registers
//! (`MachineFunction`), the frame objects it needs, and the pseudo
//! instructions only the backend's own phases understand.

use std::collections::HashMap;

use llrm_codegen::model::lir::SrcLoc;

use crate::mc::inst::{Gpr, Inst, Target, Vreg};
use crate::mc::visit::{Reg, RegRef, Role};

/// A machine instruction: a real one, or a pseudo that a later phase turns into
/// real ones (LLVM's pseudo instructions and `COPY`).
#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        }
    }

    pub fn defs_and_uses(&self) -> (Vec<Reg>, Vec<Reg>) {
        let (mut defs, mut uses) = (Vec::new(), Vec::new());
        self.clone().visit(&mut |register, role| {
            let key = register.key();
            if role.writes() {
                defs.push(key);
            }
            if role.reads() {
                uses.push(key);
            }
        });
        (defs, uses)
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
    /// The frame object each source variable lives in (`llvm.dbg.declare` of
    /// an `alloca`), by the variable's metadata node.
    pub declared: Vec<(llrm_mir::MetadataId, u32)>,
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

/// The blocks `block` (the `at`th of `count`) can go to, from the labels its
/// branches name and its fall-through.
pub fn successors_of(
    block: &MBlock,
    at: usize,
    count: usize,
    labels: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut out = Vec::new();
    let mut falls_through = true;
    for op in &block.ops {
        match op {
            Op::I(Inst::Branch { link: false, target: Target::Label(label) }) => {
                out.extend(labels.get(label.as_str()).copied());
                falls_through = false;
            }
            Op::I(Inst::BranchCond { target: Target::Label(label), .. })
            | Op::I(Inst::CompareBranch { target: Target::Label(label), .. })
            | Op::I(Inst::TestBranch { target: Target::Label(label), .. }) => {
                out.extend(labels.get(label.as_str()).copied());
            }
            Op::Return { .. } => falls_through = false,
            Op::I(Inst::Brk { .. }) => falls_through = false,
            _ => {}
        }
    }
    if falls_through && at + 1 < count {
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

/// The virtual registers some path reads before writing: a phi copy that
/// missed an edge leaves one (`LLRM_ARM64_VERIFY`).
pub fn undefined_reads(function: &MFunc) -> Vec<Reg> {
    use std::collections::HashSet;
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
                HashSet::new()
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
            HashSet::new()
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
