//! Early if-conversion: a conditional branch around a few cheap operations
//! becomes the operations, run on both paths, and a `csel` for each phi at the
//! join (`SSAIfConv` of the pin's `EarlyIfConversion.cpp`).
//!
//! # How it works
//! Runs on the selected body while it is still SSA (before [`lower_phis`]).
//! A *head* ends in a conditional branch; its successors are a triangle
//! (`head -> then -> tail`, `head -> tail`) or a diamond. When every
//! operation of the arms can be speculated and the join's phis can be
//! selects, the arms' operations move into the head, ahead of the compare
//! whose flags the branch reads, and each phi of the join becomes a `csel`
//! (`fcsel` for floats) on the branch's own condition.
//!
//! # Example
//! ```text
//! head:  cmp; b.hi tail      head:  <then ops>; cmp
//! then:  <ops>               tail:  x = csel a, b, hi
//! tail:  x = phi [a,head],[b,then]
//! ```
//!
//! # Not translated
//! - IC1: the data-dependent branch analysis (`-enable-early-ifcvt-data-
//!   dependent`, off by default in the pin).
//! - IC2: a speculated operation that names a machine register (other than
//!   the flags and the zero register) is refused; the pin tracks register
//!   units and may move it.
//! - IC3: the passes the pin runs ahead of it in `addILPOpts` (the condition
//!   optimizer, conditional compares, `MachineCombiner`, `CondBrTuning`) and
//!   the SSA passes before them (CSE, sinking, peepholes) are not run on this
//!   body, so a candidate can look different.
//!
//! # Used by
//! [`crate::driver`], before `lower_phis`.
//!
//! [`lower_phis`]: crate::mfunc::lower_phis

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::facts;
use crate::mc::inst::{AddSub, Cond, FpSize, Gpr, Inst, Logic, Select, Target, Vreg, Width};
use crate::mc::visit::{Reg, Role};
use crate::mfunc::{MFunc, Op, successors_of};
use crate::tracemetrics;

/// `early-ifcvt-limit`: the most operations of one speculated block. Applies
/// even to the stress mode of the pin's own tests, and does not here.
const BLOCK_INSTR_LIMIT: usize = 30;

/// A register unit the speculation can clobber and a reader can need: the
/// flags, or a machine register by class and number. The zero register is
/// not one.
type Unit = u32;
const FLAGS: Unit = 1 << 20;

fn unit_of(reg: Reg) -> Option<Unit> {
    match reg {
        Reg::Gpr(r) if !r.is_virtual() && r.0 != 31 => Some(r.0),
        Reg::Fpr(r) if !r.is_virtual() => Some(64 + r.0),
        _ => None,
    }
}

/// The condition `analyzeBranch` reports: what the head's branch tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Test {
    /// `b.cc`: the flags.
    Flags(Cond),
    /// `cbz`/`cbnz`.
    Compare { nonzero: bool, width: Width, rt: Gpr },
    /// `tbz`/`tbnz`.
    Bit { nonzero: bool, rt: Gpr, bit: u8 },
}

impl Test {
    /// `Cond.size() != 1`: the branch needs a compare of its own before a
    /// `csel` can read it.
    pub fn needs_compare(self) -> bool {
        !matches!(self, Test::Flags(_))
    }
}

/// The blocks and edges of the function as the pass changes them: a block's
/// number is its place when the pass began, so a number survives the removal
/// of the blocks around it.
struct Cfg {
    succ: Vec<Vec<usize>>,
    pred: Vec<Vec<usize>>,
    /// The blocks in layout order, removed ones out.
    order: Vec<usize>,
    labels: Vec<String>,
}

impl Cfg {
    fn of(function: &MFunc) -> Cfg {
        let count = function.blocks.len();
        let by_label: HashMap<&str, usize> =
            function.blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
        let mut succ: Vec<Vec<usize>> = Vec::with_capacity(count);
        for (at, block) in function.blocks.iter().enumerate() {
            let mut out = successors_of(block, at, count, &by_label);
            let mut seen = HashSet::default();
            out.retain(|to| seen.insert(*to));
            succ.push(out);
        }
        let mut pred = vec![Vec::new(); count];
        for (from, targets) in succ.iter().enumerate() {
            for &to in targets {
                pred[to].push(from);
            }
        }
        Cfg {
            succ,
            pred,
            order: (0..count).collect(),
            labels: function.blocks.iter().map(|block| block.label.clone()).collect(),
        }
    }

    fn layout_successor(
        &self,
        block: usize,
    ) -> Option<usize> {
        let at = self.order.iter().position(|&one| one == block)?;
        self.order.get(at + 1).copied()
    }
}

fn is_terminator(op: &Op) -> bool {
    matches!(
        op,
        Op::I(
            Inst::Branch { link: false, .. }
                | Inst::BranchCond { .. }
                | Inst::CompareBranch { .. }
                | Inst::TestBranch { .. }
                | Inst::BranchReg { link: false, .. }
                | Inst::Ret { .. }
                | Inst::Brk { .. }
        ) | Op::Return { .. }
            | Op::JumpTable { .. }
    )
}

/// `getFirstTerminator`: where the block's trailing branches begin.
fn first_terminator(ops: &[Op]) -> usize {
    let mut at = ops.len();
    while at > 0 && is_terminator(&ops[at - 1]) {
        at -= 1;
    }
    at
}

/// `analyzeBranch`: the head's conditional branch, the block it goes to when
/// the condition holds, and the other block if the head names it. `None`: not
/// a block that ends in one conditional branch (and at most one jump).
fn analyze_branch(
    ops: &[Op],
    labels: &HashMap<&str, usize>,
) -> Option<(Test, usize, Option<usize>, usize)> {
    let first = first_terminator(ops);
    let terms = &ops[first..];
    let target = |t: &Target| match t {
        Target::Label(label) => labels.get(label.as_str()).copied(),
        _ => None,
    };
    let test = |op: &Op| -> Option<(Test, usize)> {
        match op {
            Op::I(Inst::BranchCond { cond, target: t }) => Some((Test::Flags(*cond), target(t)?)),
            Op::I(Inst::CompareBranch { nonzero, width, rt, target: t }) => {
                Some((Test::Compare { nonzero: *nonzero, width: *width, rt: *rt }, target(t)?))
            }
            Op::I(Inst::TestBranch { nonzero, rt, bit, target: t }) => {
                Some((Test::Bit { nonzero: *nonzero, rt: *rt, bit: *bit }, target(t)?))
            }
            _ => None,
        }
    };
    match terms {
        [cond] => {
            let (test, taken) = test(cond)?;
            Some((test, taken, None, first))
        }
        [cond, Op::I(Inst::Branch { link: false, target: t })] => {
            let (test, taken) = test(cond)?;
            Some((test, taken, Some(target(t)?), first))
        }
        _ => None,
    }
}

/// What a phi of the join takes from each side.
#[derive(Clone, Debug)]
struct PhiInfo {
    /// Its place among the join's operations.
    at: usize,
    dst: Reg,
    t: Reg,
    f: Reg,
    cond_cycles: i32,
    t_cycles: i32,
    f_cycles: i32,
}

/// `SSAIfConv`: one candidate, found by [`IfConv::can_convert`] and changed by
/// [`IfConv::convert`].
pub struct IfConv {
    head: usize,
    tail: usize,
    tbb: usize,
    fbb: usize,
    cond: Test,
    phis: Vec<PhiInfo>,
    /// Where in the head the arms' operations go.
    insertion: usize,
    /// Copies the arms' reads look through.
    forward: HashMap<Reg, Reg>,
}

impl IfConv {
    fn is_triangle(&self) -> bool {
        self.tbb == self.tail || self.fbb == self.tail
    }

    /// The join's predecessor on the true side.
    fn t_pred(&self) -> usize {
        if self.tbb == self.tail { self.head } else { self.tbb }
    }

    fn f_pred(&self) -> usize {
        if self.fbb == self.tail { self.head } else { self.fbb }
    }
}

/// The values a phi takes: its block `label` named in its `incoming` list.
fn incoming_from(
    incoming: &[(Reg, String)],
    label: &str,
) -> Option<Reg> {
    incoming.iter().find(|(_, from)| from == label).map(|(value, _)| *value)
}

/// `canInsertSelect` of `AArch64InstrInfo`: whether `dst = cond ? t : f` can
/// be one select, and the cycles the condition and each value take to reach
/// it. `(cond, t, f)`; `None` where it cannot.
pub fn can_insert_select(
    test: Test,
    dst: Reg,
    t: Reg,
    f: Reg,
    defs: &Defs,
) -> Option<(i32, i32, i32)> {
    let same_class = |a: Reg, b: Reg| matches!((a, b), (Reg::Gpr(_), Reg::Gpr(_)) | (Reg::Fpr(_), Reg::Fpr(_)));
    if !same_class(t, f) || !same_class(dst, t) {
        return None;
    }
    let extra = i32::from(test.needs_compare());
    match t {
        Reg::Gpr(_) => {
            let mut cycles = (1 + extra, 1, 1);
            if fold_into_select(defs, t).is_some() {
                cycles.1 = 0;
            } else if fold_into_select(defs, f).is_some() {
                cycles.2 = 0;
            }
            Some(cycles)
        }
        // Scalar floats are `fcsel`. A 128-bit vector has no single select;
        // the width of a float register is not kept here, so vectors stay
        // with the branch (IC4).
        Reg::Fpr(_) => Some((5 + extra, 2, 2)),
    }
}

/// How a value that is the register `x + 1`, `-x` or `~x`, or the constant 1,
/// folds into a select (`canFoldIntoCSel`): the select kind and the register
/// it then takes in the value's place (`None`: the zero register).
fn fold_into_select(
    defs: &Defs,
    value: Reg,
) -> Option<(Select, Option<Reg>)> {
    // Only a virtual register's unique definition can fold.
    let value = remove_copies(defs, value);
    if !value.is_virtual() {
        return None;
    }
    let def = defs.get(&value)?;
    let zero = |reg: Gpr| reg.0 == 31;
    match def {
        Op::Imm { value: 1, .. } => Some((Select::Csinc, None)),
        Op::I(Inst::AddSubImm { op: AddSub { subtract: false, set_flags: false }, rn, imm12: 1, shift12: false, .. }) => {
            Some((Select::Csinc, Some(Reg::Gpr(*rn))))
        }
        Op::I(Inst::AddSubShifted { op: AddSub { subtract: true, set_flags: false }, rn, rm, shift, amount: 0, .. })
            if zero(*rn) && matches!(shift, crate::mc::inst::Shift::Lsl) =>
        {
            Some((Select::Csneg, Some(Reg::Gpr(*rm))))
        }
        Op::I(Inst::LogicShifted { op: Logic::Orr, invert: true, rn, rm, shift: crate::mc::inst::Shift::Lsl, amount: 0, .. })
            if zero(*rn) =>
        {
            Some((Select::Csinv, Some(Reg::Gpr(*rm))))
        }
        _ => None,
    }
}

fn remove_copies(
    defs: &Defs,
    mut value: Reg,
) -> Reg {
    while let Some(Op::Copy { src, .. }) = defs.get(&value) {
        if src.is_virtual() {
            value = *src;
        } else {
            return value;
        }
    }
    value
}

/// Zeros the registers `op` defines, so two operations compare equal when only
/// their results differ (`isIdenticalTo(IgnoreVRegDefs)`).
fn without_defs(op: &Op) -> Op {
    let mut op = op.clone();
    op.visit(&mut |reg, role| {
        if role == Role::Def {
            match reg {
                crate::mc::visit::RegRef::Gpr(r) => *r = Gpr(0),
                crate::mc::visit::RegRef::Fpr(r) => *r = Vreg(0),
            }
        }
    });
    op
}

/// `hasSameValue`: the two phi inputs are one value.
fn has_same_value(
    defs: &Defs,
    t: Reg,
    f: Reg,
) -> bool {
    if t == f {
        return true;
    }
    if !t.is_virtual() || !f.is_virtual() {
        return false;
    }
    let (Some(t_def), Some(f_def)) = (defs.get(&t), defs.get(&f)) else { return false };
    let facts = facts::of(t_def);
    if facts.side_effects || facts.may_load || facts.may_store || facts.call {
        return false;
    }
    let (t_defs, t_uses) = t_def.defs_and_uses();
    if t_uses.iter().any(|reg| unit_of(*reg).is_some()) || facts.reads_flags {
        return false;
    }
    // The same defined operand.
    let (f_defs, _) = f_def.defs_and_uses();
    if t_defs.iter().position(|r| *r == t) != f_defs.iter().position(|r| *r == f) {
        return false;
    }
    without_defs(t_def) == without_defs(f_def)
}

/// The operations of `ops` before the first terminator that are not debug
/// values, and whether each can be speculated; what they read from the head.
struct Speculation {
    /// Head operations (by place) an arm reads a value of.
    insert_after: HashSet<usize>,
    /// A value an arm reads that is a plain copy of another: the arm reads the
    /// source instead (IC3: the pin's CSE has forwarded such copies by now).
    forward: HashMap<Reg, Reg>,
    /// Units some speculated operation writes.
    clobbered: HashSet<Unit>,
}

impl Speculation {
    /// `canSpeculateInstrs`: all operations of the arm that is `ops` can run
    /// on both paths. `head_defs`: the place in the head of each value it
    /// defines.
    fn can_speculate(
        &mut self,
        ops: &[Op],
        head_defs: &HashMap<Reg, usize>,
        head_ops: &[Op],
        known: &Defs,
    ) -> bool {
        let first = first_terminator(ops);
        let mut count = 0;
        for op in &ops[..first] {
            if matches!(op, Op::DbgValue { .. }) {
                continue;
            }
            count += 1;
            if count > BLOCK_INSTR_LIMIT {
                return false;
            }
            if matches!(op, Op::Phi { .. }) {
                return false;
            }
            let facts = facts::of(op);
            // No loads (not even of a constant), nothing with an effect.
            if facts.may_load || facts.may_store || facts.call || facts.side_effects {
                return false;
            }
            if matches!(op, Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. }) {
                return false;
            }
            if facts.sets_flags {
                self.clobbered.insert(FLAGS);
            }
            let (defs, uses) = op.defs_and_uses();
            for reg in defs.iter() {
                if let Some(unit) = unit_of(*reg) {
                    self.clobbered.insert(unit);
                }
            }
            // IC2: a machine register named by an operation stays in its block.
            if uses.iter().any(|reg| unit_of(*reg).is_some()) {
                return false;
            }
            for reg in uses.iter() {
                if !reg.is_virtual() {
                    continue;
                }
                let source = remove_copies(known, *reg);
                if source != *reg {
                    self.forward.insert(*reg, source);
                }
                if let Some(&at) = head_defs.get(&source) {
                    if is_terminator(&head_ops[at]) {
                        return false;
                    }
                    self.insert_after.insert(at);
                }
            }
        }
        true
    }
}

/// `findInsertionPoint`: the last place in the head, below every operation an
/// arm depends on, where no register an arm clobbers is live (the flags the
/// branch reads, above all).
fn find_insertion_point(
    head: &[Op],
    speculation: &Speculation,
) -> Option<usize> {
    let first_term = first_terminator(head);
    let mut live: HashSet<Unit> = HashSet::default();
    for at in (0..head.len()).rev() {
        if speculation.insert_after.contains(&at) {
            return None;
        }
        let op = &head[at];
        let facts = facts::of(op);
        let (defs, uses) = op.defs_and_uses();
        // A definition ends the unit's liveness going up.
        if facts.sets_flags {
            live.remove(&FLAGS);
        }
        for reg in defs.iter() {
            if let Some(unit) = unit_of(*reg) {
                live.remove(&unit);
            }
        }
        let mut reads: Vec<Unit> = uses.iter().filter_map(|reg| unit_of(*reg)).collect();
        if facts.reads_flags {
            reads.push(FLAGS);
        }
        for unit in reads {
            if speculation.clobbered.contains(&unit) {
                live.insert(unit);
            }
        }
        if at != first_term && is_terminator(op) {
            continue;
        }
        if !live.is_empty() {
            continue;
        }
        return Some(at);
    }
    None
}

/// The one definition of each virtual register: the operation, and the block
/// it is in.
#[derive(Default)]
pub struct Defs {
    op: HashMap<Reg, Op>,
    block: HashMap<Reg, usize>,
}

impl Defs {
    fn of(function: &MFunc) -> Defs {
        let mut out = Defs::default();
        for (b, block) in function.blocks.iter().enumerate() {
            for op in block.ops.iter() {
                out.add(op, b);
            }
        }
        out
    }

    fn add(
        &mut self,
        op: &Op,
        block: usize,
    ) {
        for reg in op.defs_and_uses().0.iter() {
            if reg.is_virtual() {
                self.op.insert(*reg, op.clone());
                self.block.insert(*reg, block);
            }
        }
    }

    fn get(
        &self,
        reg: &Reg,
    ) -> Option<&Op> {
        self.op.get(reg)
    }
}

impl IfConv {
    /// `canConvertIf`: whether the sub-CFG headed by `head` can be converted.
    pub fn can_convert(
        function: &MFunc,
        cfg: &Cfg,
        head: usize,
        defs: &Defs,
    ) -> Option<IfConv> {
        if cfg.succ[head].len() != 2 {
            return None;
        }
        let (mut s0, mut s1) = (cfg.succ[head][0], cfg.succ[head][1]);
        if cfg.pred[s0].len() != 1 {
            std::mem::swap(&mut s0, &mut s1);
        }
        if cfg.pred[s0].len() != 1 || cfg.succ[s0].len() != 1 {
            return None;
        }
        let tail = cfg.succ[s0][0];
        if tail != s1 && (cfg.pred[s1].len() != 1 || cfg.succ[s1].len() != 1 || cfg.succ[s1][0] != tail) {
            return None;
        }
        let tail_ops = &function.blocks[tail].ops;
        if !matches!(tail_ops.first(), Some(Op::Phi { .. })) {
            return None;
        }
        let labels: HashMap<&str, usize> =
            cfg.labels.iter().enumerate().filter(|(n, _)| cfg.order.contains(n)).map(|(n, l)| (l.as_str(), n)).collect();
        let head_ops = &function.blocks[head].ops;
        let (cond, tbb, _, _) = analyze_branch(head_ops, &labels)?;
        let fbb = if tbb == s0 { s1 } else { s0 };
        let mut conv = IfConv { head, tail, tbb, fbb, cond, phis: Vec::new(), insertion: 0, forward: HashMap::default() };
        let (t_pred, f_pred) = (conv.t_pred(), conv.f_pred());
        let (t_label, f_label) = (&cfg.labels[t_pred], &cfg.labels[f_pred]);
        for (at, op) in tail_ops.iter().enumerate() {
            let Op::Phi { dst, incoming } = op else { break };
            let t = incoming_from(incoming, t_label)?;
            let f = incoming_from(incoming, f_label)?;
            let (cond_cycles, t_cycles, f_cycles) = can_insert_select(cond, *dst, t, f, defs)?;
            conv.phis.push(PhiInfo { at, dst: *dst, t, f, cond_cycles, t_cycles, f_cycles });
        }
        let mut head_defs: HashMap<Reg, usize> = HashMap::default();
        for (at, op) in head_ops.iter().enumerate() {
            for reg in op.defs_and_uses().0.iter().filter(|reg| reg.is_virtual()) {
                head_defs.insert(*reg, at);
            }
        }
        let mut speculation = Speculation { insert_after: HashSet::default(), forward: HashMap::default(), clobbered: HashSet::default() };
        if conv.tbb != tail && !speculation.can_speculate(&function.blocks[conv.tbb].ops, &head_defs, head_ops, defs) {
            return None;
        }
        if conv.fbb != tail && !speculation.can_speculate(&function.blocks[conv.fbb].ops, &head_defs, head_ops, defs) {
            return None;
        }
        conv.insertion = find_insertion_point(head_ops, &speculation)?;
        conv.forward = speculation.forward;
        Some(conv)
    }
}

/// The width an operation defines its result at, where it says.
fn width_of(op: &Op) -> Option<Width> {
    match op {
        Op::Imm { bits, .. } => Some(if *bits == 32 { Width::W } else { Width::X }),
        Op::I(
            Inst::AddSubImm { width, .. }
            | Inst::AddSubShifted { width, .. }
            | Inst::AddSubExtended { width, .. }
            | Inst::LogicImm { width, .. }
            | Inst::LogicShifted { width, .. }
            | Inst::MoveWide { width, .. }
            | Inst::BitfieldMove { width, .. }
            | Inst::Extract { width, .. }
            | Inst::Divide { width, .. }
            | Inst::ThreeSource { width, .. }
            | Inst::CondSelect { width, .. }
            | Inst::AddCarry { width, .. },
        ) => Some(*width),
        _ => None,
    }
}

/// The size an operation defines a float at, where it says.
fn fp_size_of(op: &Op) -> Option<FpSize> {
    match op {
        Op::I(
            Inst::FpUnary { size, .. }
            | Inst::FpBinary { size, .. }
            | Inst::FpFused { size, .. }
            | Inst::FpSelect { size, .. }
            | Inst::FpMoveImm { size, .. },
        ) => Some(*size),
        Op::I(Inst::FpToFp { to, .. }) => Some(*to),
        _ => None,
    }
}

/// The class of a select over `t` and `f`, from what defines them: the pin
/// reads it from the phi's register class, a virtual register here has none
/// (IC5). Copies and phis are followed.
fn value_width(
    defs: &Defs,
    reg: Reg,
) -> Option<Width> {
    let mut reg = reg;
    for _ in 0..8 {
        match defs.get(&reg)? {
            Op::Copy { src, .. } if src.is_virtual() => reg = *src,
            op => return width_of(op),
        }
    }
    None
}

fn value_fp_size(
    defs: &Defs,
    reg: Reg,
) -> Option<FpSize> {
    let mut reg = reg;
    for _ in 0..8 {
        match defs.get(&reg)? {
            Op::Copy { src, .. } if src.is_virtual() => reg = *src,
            op => return fp_size_of(op),
        }
    }
    None
}

/// `insertCmpForCondBr` and `insertSelect` of `AArch64InstrInfo`: the
/// operations that make `dst = test ? t : f`, the compare of a `cbz` or `tbz`
/// first. `None` where the class cannot be told.
fn select_ops(
    dst: Reg,
    test: Test,
    t: Reg,
    f: Reg,
    defs: &Defs,
) -> Option<Vec<Op>> {
    let mut out = Vec::new();
    let cc = match test {
        Test::Flags(cond) => cond,
        Test::Compare { nonzero, width, rt } => {
            let op = AddSub { subtract: true, set_flags: true };
            out.push(Op::I(Inst::AddSubImm { op, width, rd: Gpr(31), rn: rt, imm12: 0, shift12: false }));
            if nonzero { Cond::Ne } else { Cond::Eq }
        }
        Test::Bit { nonzero, rt, bit } => {
            let width = if bit < 32 { Width::W } else { Width::X };
            out.push(Op::I(Inst::LogicImm { op: Logic::Ands, width, rd: Gpr(31), rn: rt, imm: 1u64 << bit }));
            if nonzero { Cond::Ne } else { Cond::Eq }
        }
    };
    match (dst, t, f) {
        (Reg::Gpr(rd), Reg::Gpr(mut rn), Reg::Gpr(mut rm)) => {
            let width = value_width(defs, t).or_else(|| value_width(defs, f)).unwrap_or(Width::X);
            let (mut op, mut cond) = (Select::Csel, cc);
            if let Some((kind, src)) = fold_into_select(defs, t) {
                cond = cc.inverted();
                rn = rm;
                rm = src.map_or(Gpr(31), |r| if let Reg::Gpr(g) = r { g } else { Gpr(31) });
                op = kind;
            } else if let Some((kind, src)) = fold_into_select(defs, f) {
                rm = src.map_or(Gpr(31), |r| if let Reg::Gpr(g) = r { g } else { Gpr(31) });
                op = kind;
            }
            out.push(Op::I(Inst::CondSelect { op, width, rd, rn, rm, cond }));
        }
        (Reg::Fpr(rd), Reg::Fpr(rn), Reg::Fpr(rm)) => {
            let size = value_fp_size(defs, t).or_else(|| value_fp_size(defs, f))?;
            out.push(Op::I(Inst::FpSelect { size, rd, rn, rm, cond: cc }));
        }
        _ => return None,
    }
    Some(out)
}

/// Replaces the incoming block named `from` by `to` in every phi.
fn rename_incoming(
    function: &mut MFunc,
    from: &str,
    to: &str,
) {
    for block in &mut function.blocks {
        for op in &mut block.ops {
            match op {
                Op::Phi { incoming, .. } => {
                    for (_, label) in incoming.iter_mut().filter(|(_, label)| label == from) {
                        *label = to.to_owned();
                    }
                }
                _ => break,
            }
        }
    }
}

impl IfConv {
    /// `convertIf`: the arms' operations go into the head, each phi of the
    /// join becomes a select, the arms (and the join, if it is the head's
    /// layout successor and has no other predecessor) are removed. The
    /// numbers of the blocks removed.
    pub fn convert(
        &self,
        function: &mut MFunc,
        cfg: &mut Cfg,
        defs: &mut Defs,
    ) -> Vec<usize> {
        let mut removed = Vec::new();
        // The operations of an arm: everything before its terminators.
        let mut moved: Vec<(Op, Option<llrm_codegen::model::lir::SrcLoc>)> = Vec::new();
        for arm in [self.tbb, self.fbb] {
            if arm == self.tail {
                continue;
            }
            let ops = function.blocks[arm].ops.take();
            let first = first_terminator(&ops);
            for (at, (mut op, loc)) in ops.into_iter().enumerate() {
                if at < first {
                    if !self.forward.is_empty() {
                        op.visit(&mut |reg, role| {
                            if role.reads() && !role.writes() {
                                match reg {
                                    crate::mc::visit::RegRef::Gpr(r) => {
                                        if let Some(Reg::Gpr(to)) = self.forward.get(&Reg::Gpr(*r)) {
                                            *r = *to;
                                        }
                                    }
                                    crate::mc::visit::RegRef::Fpr(r) => {
                                        if let Some(Reg::Fpr(to)) = self.forward.get(&Reg::Fpr(*r)) {
                                            *r = *to;
                                        }
                                    }
                                }
                            }
                        });
                    }
                    moved.push((op, loc));
                }
            }
            removed.push(arm);
        }
        for (offset, (op, loc)) in moved.into_iter().enumerate() {
            defs.add(&op, self.head);
            function.blocks[self.head].ops.insert_at(self.insertion + offset, op, loc);
        }
        let extra_preds = cfg.pred[self.tail].len() != 2;
        // Phis: selects at the head's first terminator.
        let mut selects: Vec<(Vec<Op>, Reg)> = Vec::new();
        let mut kept: Vec<(Reg, Reg, Reg, Reg)> = Vec::new();
        for info in &self.phis {
            let same = has_same_value(defs, info.t, info.f);
            if !extra_preds {
                if same {
                    selects.push((vec![Op::Copy { dst: info.dst, src: info.t }], info.dst));
                } else {
                    let ops = select_ops(info.dst, self.cond, info.t, info.f, defs)
                        .expect("the select was found possible");
                    selects.push((ops, info.dst));
                }
            } else if same {
                kept.push((info.dst, info.t, info.f, info.t));
            } else {
                let fresh = match info.dst {
                    Reg::Gpr(_) => Reg::Gpr(function.gpr()),
                    Reg::Fpr(_) => Reg::Fpr(function.fpr()),
                };
                let ops =
                    select_ops(fresh, self.cond, info.t, info.f, defs).expect("the select was found possible");
                selects.push((ops, fresh));
                kept.push((info.dst, info.t, info.f, fresh));
            }
        }
        let head = self.head;
        for (ops, _) in selects {
            let at = first_terminator(&function.blocks[head].ops);
            for (offset, op) in ops.into_iter().enumerate() {
                defs.add(&op, head);
                function.blocks[head].ops.insert(at + offset, op);
            }
        }
        // The join's phis.
        let head_label = cfg.labels[head].clone();
        let (t_label, f_label) = (cfg.labels[self.t_pred()].clone(), cfg.labels[self.f_pred()].clone());
        if extra_preds {
            for (info, (_, _, _, value)) in self.phis.iter().zip(&kept) {
                let Op::Phi { incoming, .. } = function.blocks[self.tail].ops.op_mut(info.at) else { unreachable!() };
                let mut rebuilt = Vec::new();
                for (reg, label) in incoming.drain(..) {
                    if label == t_label {
                        rebuilt.push((*value, head_label.clone()));
                    } else if label != f_label {
                        rebuilt.push((reg, label));
                    }
                }
                *incoming = rebuilt;
            }
        } else {
            let phis = self.phis.len();
            let tail_ops = &mut function.blocks[self.tail].ops;
            for _ in 0..phis {
                tail_ops.remove(0);
            }
        }
        // The head's branches go.
        let first = first_terminator(&function.blocks[head].ops);
        function.blocks[head].ops.truncate(first);
        // The edges.
        for arm in [self.tbb, self.fbb] {
            if arm != self.tail {
                cfg.pred[self.tail].retain(|&p| p != arm);
            }
        }
        cfg.pred[self.tail].retain(|&p| p != head);
        cfg.succ[head].clear();
        cfg.order.retain(|n| !removed.contains(n));
        if !extra_preds && cfg.layout_successor(head) == Some(self.tail) {
            let tail_ops = function.blocks[self.tail].ops.take();
            for (op, loc) in tail_ops {
                function.blocks[head].ops.push_at(op, loc);
            }
            cfg.succ[head] = std::mem::take(&mut cfg.succ[self.tail]);
            let tail_label = cfg.labels[self.tail].clone();
            for &s in &cfg.succ[head] {
                for p in cfg.pred[s].iter_mut().filter(|p| **p == self.tail) {
                    *p = head;
                }
            }
            rename_incoming(function, &tail_label, &head_label);
            cfg.pred[self.tail].clear();
            cfg.order.retain(|&n| n != self.tail);
            removed.push(self.tail);
        } else {
            function.blocks[head].ops.push(crate::mfunc::branch_to(&cfg.labels[self.tail]));
            cfg.succ[head] = vec![self.tail];
            cfg.pred[self.tail].push(head);
        }
        removed
    }
}

/// The body as the trace metrics read it.
fn build_func(
    function: &MFunc,
    cfg: &Cfg,
    model: &crate::schedmodel::Arm64Model,
) -> tracemetrics::Func {
    use llrm_codegen::backend::schedmodel::SchedulingModel;
    let machine = model.machine();
    let factors: Vec<u32> = (0..machine.resources.len()).map(|k| machine.resource_factor(k)).collect();
    let number: HashMap<&str, usize> = cfg.labels.iter().enumerate().map(|(n, l)| (l.as_str(), n)).collect();
    let count = function.blocks.len();
    let mut blocks = vec![tracemetrics::Blk::default(); count];
    let mut def: HashMap<u32, (usize, usize)> = HashMap::default();
    for &b in &cfg.order {
        blocks[b].pred = cfg.pred[b].clone();
        blocks[b].succ = cfg.succ[b].clone();
        for (at, op) in function.blocks[b].ops.iter().enumerate() {
            let (defs, uses) = op.defs_and_uses();
            let mut ins = tracemetrics::Ins { latency: 1, ..Default::default() };
            for reg in defs.iter() {
                match reg {
                    r if r.is_virtual() => {
                        let id = reg_id(*r);
                        ins.defs.push(id);
                        def.insert(id, (b, at));
                    }
                    r => ins.unit_defs.extend(unit_of(*r)),
                }
            }
            let f = facts::of(op);
            if f.sets_flags {
                ins.unit_defs.push(FLAGS);
            }
            match op {
                Op::Phi { incoming, .. } => {
                    ins.transient = true;
                    ins.phi = Some(
                        incoming
                            .iter()
                            .filter_map(|(value, from)| Some((*number.get(from.as_str())?, reg_id(*value))))
                            .collect(),
                    );
                }
                _ => {
                    for reg in uses.iter() {
                        if reg.is_virtual() {
                            ins.uses.push(reg_id(*reg));
                        } else {
                            ins.unit_uses.extend(unit_of(*reg));
                        }
                    }
                    if f.reads_flags {
                        ins.unit_uses.push(FLAGS);
                    }
                    ins.call = f.call;
                    ins.transient = matches!(op, Op::Copy { .. } | Op::DbgValue { .. } | Op::I(Inst::Label(_) | Inst::Cfi(_) | Inst::Mark(_)));
                    if !ins.transient {
                        if let Some(insn) = crate::lir_body::insn_of(function, op) {
                            ins.latency = model.def_latency(&insn);
                            ins.writes = model.unit(&insn).writes.iter().map(|w| (w.idx, w.release_at_cycle)).collect();
                        }
                    }
                }
            }
            blocks[b].ins.push(ins);
        }
    }
    // Each block is its own number among the loop analysis's blocks; a block
    // the pass removed is no node (it has no edges).
    struct N {
        at: i64,
        succ: Vec<i64>,
    }
    impl llrm_codegen::analysis::loops::Node for N {
        fn at(&self) -> i64 {
            self.at
        }

        fn succ(&self) -> &[i64] {
            &self.succ
        }
    }
    let nodes: Vec<N> = (0..count)
        .map(|b| N { at: b as i64, succ: blocks[b].succ.iter().map(|&s| s as i64).collect() })
        .collect();
    let forest = llrm_codegen::analysis::loops::forest_of(&nodes, Some(0));
    let mut loops = tracemetrics::Loops { of: vec![None; count], ..Default::default() };
    for (i, one) in forest.loops.iter().enumerate() {
        loops.headers.push(one.header as usize);
        loops.parent.push(one.parent);
        loops.blocks.push(forest.blocks(i).map(|b| b as usize).collect());
    }
    for b in 0..count {
        loops.of[b] = forest.innermost(b as i64);
    }
    tracemetrics::Func {
        blocks,
        def,
        loops,
        model: tracemetrics::Model {
            issue_width: machine.issue_width,
            latency_factor: machine.latency_factor(),
            factors,
        },
    }
}

fn reg_id(reg: Reg) -> u32 {
    match reg {
        Reg::Gpr(r) => r.0,
        Reg::Fpr(r) => r.0,
    }
}

/// `adjCycles`.
fn adj_cycles(
    cycles: u32,
    delta: i32,
) -> u32 {
    if delta < 0 && cycles.wrapping_add_signed(delta) > cycles { 0 } else { cycles.wrapping_add_signed(delta) }
}

/// Whether `op`, defined in block `block`, is the same on every iteration of
/// the loop `looped` (`MachineLoop::isLoopInvariant`).
fn loop_invariant(
    op: &Op,
    looped: usize,
    func: &tracemetrics::Func,
    defs: &Defs,
) -> bool {
    let (d, u) = op.defs_and_uses();
    if d.iter().any(|r| unit_of(*r).is_some()) || u.iter().any(|r| unit_of(*r).is_some()) {
        return false;
    }
    u.iter().filter(|r| r.is_virtual()).all(|r| {
        defs.block.get(r).is_none_or(|b| !func.loops.blocks[looped].contains(b))
    })
}

/// `EarlyIfConverter::shouldConvertIf`.
fn should_convert(
    conv: &IfConv,
    traces: &mut tracemetrics::Traces,
    func: &tracemetrics::Func,
    function: &MFunc,
    defs: &Defs,
    penalty: u32,
) -> bool {
    // A branch on a loop-invariant condition predicts well.
    if let Some(looped) = func.loops.of[conv.head] {
        let reg = match conv.cond {
            Test::Flags(_) => None,
            Test::Compare { rt, .. } | Test::Bit { rt, .. } => Some(Reg::Gpr(rt)),
        };
        if let Some(reg) = reg.filter(|r| r.is_virtual())
            && let Some(def) = defs.get(&reg)
        {
            let ok = |op: &Op| loop_invariant(op, looped, func, defs);
            let all_inputs = def.defs_and_uses().1.iter().filter(|r| r.is_virtual()).all(|r| defs.get(r).is_some_and(ok));
            if ok(def) || all_inputs {
                return false;
            }
        }
    }
    let t_trace = traces.trace(conv.t_pred());
    let f_trace = traces.trace(conv.f_pred());
    let min_crit = t_trace.critical_path(traces).min(f_trace.critical_path(traces));
    let crit_limit = penalty / 2;
    let extra: Vec<usize> = if conv.tbb != conv.tail { vec![conv.tbb] } else { Vec::new() };
    let res_length = f_trace.resource_length(traces, &extra);
    if res_length > min_crit + crit_limit {
        return false;
    }
    let head_trace = traces.trace(conv.head);
    let first = first_terminator(&function.blocks[conv.head].ops);
    let branch_depth = head_trace.instr_depth(traces, first);
    let tail_trace = traces.trace(conv.tail);
    let _ = func;
    let mut should = true;
    for info in &conv.phis {
        let slack = tail_trace.instr_slack(traces, info.at);
        let max_depth = slack + tail_trace.instr_depth(traces, info.at);
        let cond_depth = adj_cycles(branch_depth, info.cond_cycles);
        if cond_depth > max_depth && cond_depth - max_depth > crit_limit {
            should = false;
        }
        let t_depth = adj_cycles(t_trace.phi_depth(traces, conv.tail, info.at), info.t_cycles);
        if t_depth > max_depth && t_depth - max_depth > crit_limit {
            should = false;
        }
        let f_depth = adj_cycles(f_trace.phi_depth(traces, conv.tail, info.at), info.f_cycles);
        if f_depth > max_depth && f_depth - max_depth > crit_limit {
            should = false;
        }
    }
    should
}

/// `LLRM_ARM64_IFCVT`: `stress` converts whenever legal (the pin's
/// `-stress-early-ifcvt`); any other value runs the model.
fn stress() -> bool {
    std::env::var("LLRM_ARM64_IFCVT").is_ok_and(|mode| mode == "stress")
}

/// The dominator tree's post order (`post_order(DomTree)`).
fn dominator_post_order(cfg: &Cfg) -> Vec<usize> {
    struct N<'a> {
        at: i64,
        succ: &'a [i64],
    }
    impl llrm_codegen::analysis::loops::Node for N<'_> {
        fn at(&self) -> i64 {
            self.at
        }

        fn succ(&self) -> &[i64] {
            self.succ
        }
    }
    let count = cfg.succ.len();
    let succ: Vec<Vec<i64>> = cfg.succ.iter().map(|out| out.iter().map(|&b| b as i64).collect()).collect();
    let nodes: Vec<N> = (0..count).map(|at| N { at: at as i64, succ: &succ[at] }).collect();
    let idom = llrm_codegen::analysis::loops::immediate_dominators(&nodes, Some(0));
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (&block, &parent) in &idom {
        if let Some(parent) = parent.filter(|&parent| parent != block) {
            children[parent as usize].push(block as usize);
        }
    }
    for list in &mut children {
        list.sort_unstable();
    }
    let mut out = Vec::new();
    let mut stack = vec![(0usize, 0usize)];
    while let Some((node, child)) = stack.pop() {
        if child < children[node].len() {
            stack.push((node, child + 1));
            stack.push((children[node][child], 0));
        } else {
            out.push(node);
        }
    }
    out
}

/// `EarlyIfConverter::run`: converts every candidate the model accepts, the
/// innermost first. The number of conversions.
pub fn run(function: &mut MFunc) -> usize {
    if function.blocks.len() < 3 {
        return 0;
    }
    let model = crate::schedmodel::Arm64Model::generic();
    let penalty = model.mispredict_penalty();
    let mut cfg = Cfg::of(function);
    let mut defs = Defs::of(function);
    let order = dominator_post_order(&cfg);
    let mut converted = 0;
    let mut position = 0;
    let log = std::env::var_os("LLRM_ARM64_IFCVT_LOG").is_some();
    // The traces live until a conversion changes the body they read.
    'epoch: loop {
        let func = if stress() { None } else { Some(build_func(function, &cfg, &model)) };
        let mut traces = func.as_ref().map(tracemetrics::Traces::new);
        while position < order.len() {
            let head = order[position];
            if !cfg.order.contains(&head) {
                position += 1;
                continue;
            }
            let Some(candidate) = IfConv::can_convert(function, &cfg, head, &defs) else {
                position += 1;
                continue;
            };
            let accept = match (&mut traces, &func) {
                (Some(traces), Some(func)) => should_convert(&candidate, traces, func, function, &defs, penalty),
                _ => true,
            };
            if !accept {
                position += 1;
                continue;
            }
            if log {
                eprintln!("ifcvt @{}: head {} tail {} phis {}", function.name, cfg.labels[head], cfg.labels[candidate.tail], candidate.phis.len());
            }
            candidate.convert(function, &mut cfg, &mut defs);
            converted += 1;
            continue 'epoch;
        }
        break;
    }
    if converted > 0 {
        let alive: HashSet<usize> = cfg.order.iter().copied().collect();
        let mut at = 0;
        function.blocks.retain(|_| {
            at += 1;
            alive.contains(&(at - 1))
        });
    }
    converted
}
