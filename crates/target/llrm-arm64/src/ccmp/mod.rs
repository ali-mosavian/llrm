//! Conditional compares: a compare-and-branch chain from `&&` or `||` becomes
//! one `cmp`, a `ccmp` and one branch (`AArch64ConditionalCompares` of the
//! pin).
//!
//! # How it works
//! Runs on the selected body while it is still SSA, just before early
//! if-conversion. A *head* ends in a conditional branch; one successor
//! (*cmp block*) has the head as its only predecessor, ends in a compare and a
//! conditional branch, and shares its other successor (*tail*) with the head.
//! The cmp block's compare becomes a `ccmp` under the condition that sent the
//! head to the cmp block: it compares when that holds, and otherwise sets the
//! flags to values that satisfy the cmp block's condition for the tail. The
//! cmp block's operations move into the head, which now ends in the cmp
//! block's branch.
//!
//! A `cbz`/`cbnz` is a compare too: as the head's branch it gets a `cmp #0` of
//! its own, as the cmp block's it becomes `ccmp #0`.
//!
//! # Example
//! ```text
//! head:  cmp w0, #5          head:  cmp  w0, #5
//!        b.eq tail                  ccmp w1, #17, #4, ne
//! cmp:   cmp w1, #17                b.eq tail
//!        b.eq tail
//! ```
//!
//! # Not translated
//! - CC1: a floating-point compare (`fcmp`, `fccmp`) is not converted:
//!   `FpCondCompare` has no signalling form. Exit: `fccmpe`.
//! - CC2: the pin's `cbb`/`cbh`/`cb` compare-and-branch forms do not exist
//!   here.
//! - CC3: the condition optimizer, which the pin runs before this pass, is not
//!   run (as for [`crate::ifcvt`]).
//!
//! # Used by
//! [`crate::driver`], before early if-conversion.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::facts;
use crate::ifcvt::{
    Cfg, Defs, FLAGS, Test, analyze_branch, build_func, dominator_children, first_terminator, incoming_from,
    remove_copies, rename_incoming, unit_of,
};
use crate::mc::inst::{AddSub, CcOperand, Cond, Gpr, Inst, Target, Width};
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};
use crate::tracemetrics;

/// `aarch64-ccmp-limit`: the most operations of the speculated block.
const BLOCK_INSTR_LIMIT: usize = 30;

/// `AArch64CC::getNZCVToSatisfyCondCode`: flags (`NZCV`, as `ccmp` takes them)
/// that satisfy `cond`.
fn nzcv_to_satisfy(cond: Cond) -> u8 {
    const N: u8 = 8;
    const Z: u8 = 4;
    const C: u8 = 2;
    const V: u8 = 1;
    match cond {
        Cond::Eq | Cond::Le => Z,
        Cond::Hs | Cond::Hi => C,
        Cond::Mi | Cond::Lt => N,
        Cond::Vs => V,
        _ => 0,
    }
}

/// The condition a branch test sends control on, as `parseCond` reads it:
/// `None` for a `tbz`/`tbnz`.
fn parse_cond(test: Test) -> Option<Cond> {
    match test {
        Test::Flags(cond) => Some(cond),
        Test::Compare { nonzero, .. } => Some(if nonzero { Cond::Ne } else { Cond::Eq }),
        Test::Bit { .. } => None,
    }
}

/// The compare of the cmp block that becomes a `ccmp`.
#[derive(Clone, Copy)]
enum Compare {
    /// `subs`/`adds` with a dead result, by its place in the block.
    Flags { at: usize, negate: bool, width: Width, rn: Gpr, rm_or_imm: CcOperand },
    /// The block's `cbz`/`cbnz` terminator.
    Zero { width: Width, rt: Gpr },
}

/// `SSACCmpConv`: one candidate, found by [`CmpConv::can_convert`] and changed
/// by [`CmpConv::convert`].
pub struct CmpConv {
    head: usize,
    cmp_bb: usize,
    tail: usize,
    other: usize,
    /// Where the cmp block goes when its branch is not taken.
    cmp_falls: usize,
    head_test: Test,
    /// The condition that sends the head to the cmp block.
    head_cc: Cond,
    /// The condition that sends the cmp block to the tail.
    cmp_cc: Cond,
    compare: Compare,
}

/// The registers some operation reads: a compare whose result nobody reads is
/// a dead definition (`isDeadDef`).
fn used_registers(function: &MFunc) -> HashSet<Reg> {
    let mut used = HashSet::default();
    for block in &function.blocks {
        for op in block.ops.iter() {
            for reg in op.defs_and_uses().1.iter() {
                used.insert(*reg);
            }
        }
    }
    used
}

fn is_dead_def(
    reg: Gpr,
    used: &HashSet<Reg>,
) -> bool {
    reg.0 == 31 || (reg.is_virtual() && !used.contains(&Reg::Gpr(reg)))
}

impl CmpConv {
    /// `findConvertibleCompare`: the compare that controls the cmp block's
    /// branch, whose operands a `ccmp` can take.
    fn find_convertible_compare(
        ops: &[Op],
        test: Test,
        used: &HashSet<Reg>,
    ) -> Option<Compare> {
        let first = first_terminator(ops);
        match test {
            // `cbz`/`cbnz` is a compare against zero.
            Test::Compare { width, rt, .. } => return Some(Compare::Zero { width, rt }),
            Test::Bit { .. } => return None,
            Test::Flags(_) => {}
        }
        // Now the instruction that controls the terminator.
        for at in (0..first).rev() {
            let facts = facts::of(&ops[at]);
            match &ops[at] {
                Op::I(Inst::AddSubImm { op: AddSub { subtract, set_flags: true }, width, rd, rn, imm12, shift12 }) => {
                    if *shift12 || *imm12 >= 32 {
                        return None;
                    }
                    if !is_dead_def(*rd, used) {
                        return None;
                    }
                    return Some(Compare::Flags {
                        at,
                        negate: !*subtract,
                        width: *width,
                        rn: *rn,
                        rm_or_imm: CcOperand::Imm5(*imm12 as u8),
                    });
                }
                Op::I(Inst::AddSubShifted {
                    op: AddSub { subtract, set_flags: true },
                    width,
                    rd,
                    rn,
                    rm,
                    shift: crate::mc::inst::Shift::Lsl,
                    amount: 0,
                }) => {
                    if !is_dead_def(*rd, used) {
                        return None;
                    }
                    return Some(Compare::Flags {
                        at,
                        negate: !*subtract,
                        width: *width,
                        rn: *rn,
                        rm_or_imm: CcOperand::Reg(*rm),
                    });
                }
                _ => {}
            }
            // Flag reads and clobbers: the `ccmp` does not make exactly the
            // flags the compare did, and a compare it cannot take is no
            // compare.
            if facts.reads_flags || facts.sets_flags || facts.call {
                return None;
            }
        }
        None
    }

    /// `canSpeculateInstrs`: every operation of the cmp block can run on both
    /// paths; only the compare may write the flags.
    fn can_speculate(
        ops: &[Op],
        compare: Compare,
    ) -> bool {
        let first = first_terminator(ops);
        let mut count = 0;
        for (at, op) in ops[..first].iter().enumerate() {
            if matches!(op, Op::DbgValue { .. }) {
                continue;
            }
            count += 1;
            if count > BLOCK_INSTR_LIMIT {
                return false;
            }
            if matches!(
                op,
                Op::Phi { .. } | Op::Call { .. } | Op::Return { .. } | Op::JumpTable { .. } | Op::IndirectJump { .. }
            ) {
                return false;
            }
            let facts = facts::of(op);
            if facts.may_load || facts.may_store || facts.call || facts.side_effects || facts.early_clobber {
                return false;
            }
            let is_compare = matches!(compare, Compare::Flags { at: place, .. } if place == at);
            if facts.sets_flags && !is_compare {
                return false;
            }
            // A machine register named by an operation stays where it is.
            let (defs, uses) = op.defs_and_uses();
            if defs.iter().chain(uses.iter()).any(|reg| unit_of(*reg).is_some_and(|unit| unit != FLAGS)) {
                return false;
            }
        }
        true
    }

    /// `trivialTailPHIs`: the tail's phis take the same value from the head
    /// and from the cmp block.
    fn trivial_tail_phis(
        function: &MFunc,
        cfg: &Cfg,
        tail: usize,
        head: usize,
        cmp_bb: usize,
        defs: &Defs,
    ) -> bool {
        for op in function.blocks[tail].ops.iter() {
            let Op::Phi { incoming, .. } = op else { break };
            let (Some(from_head), Some(from_cmp)) =
                (incoming_from(incoming, &cfg.labels[head]), incoming_from(incoming, &cfg.labels[cmp_bb]))
            else {
                return false;
            };
            if remove_copies(defs, from_head) != remove_copies(defs, from_cmp) {
                return false;
            }
        }
        true
    }

    /// `canConvert`: whether the sub-CFG headed by `head` can be converted.
    pub fn can_convert(
        function: &MFunc,
        cfg: &Cfg,
        head: usize,
        defs: &Defs,
        used: &HashSet<Reg>,
    ) -> Option<CmpConv> {
        if cfg.succ[head].len() != 2 {
            return None;
        }
        let (mut s0, mut s1) = (cfg.succ[head][0], cfg.succ[head][1]);
        // The cmp block has one predecessor; the tail may have many.
        if cfg.pred[s0].len() != 1 {
            std::mem::swap(&mut s0, &mut s1);
        }
        if cfg.pred[s0].len() != 1 || cfg.succ[s0].len() != 2 {
            return None;
        }
        let (cmp_bb, tail) = (s0, s1);
        if cmp_bb == head || !cfg.succ[cmp_bb].contains(&tail) {
            return None;
        }
        let other = *cfg.succ[cmp_bb].iter().find(|&&one| one != tail)?;
        // A block whose address is taken stays (`hasAddressTaken`).
        if function.block_symbols.iter().any(|(label, _)| *label == cfg.labels[cmp_bb]) {
            return None;
        }
        if !Self::trivial_tail_phis(function, cfg, tail, head, cmp_bb, defs) {
            return None;
        }
        // The cmp block has one predecessor: a phi in it is a copy.
        if matches!(function.blocks[cmp_bb].ops.first(), Some(Op::Phi { .. })) {
            return None;
        }
        let labels: HashMap<&str, usize> = cfg
            .labels
            .iter()
            .enumerate()
            .filter(|(n, _)| cfg.order.contains(n))
            .map(|(n, l)| (l.as_str(), n))
            .collect();
        let (head_test, head_taken, _, _) = analyze_branch(&function.blocks[head].ops, &labels)?;
        let mut head_cc = parse_cond(head_test)?;
        if head_taken != cmp_bb {
            if head_taken != tail {
                return None;
            }
            head_cc = head_cc.inverted();
        }
        let cmp_ops = &function.blocks[cmp_bb].ops;
        let (cmp_test, cmp_taken, _, _) = analyze_branch(cmp_ops, &labels)?;
        let mut cmp_cc = parse_cond(cmp_test)?;
        if cmp_taken != tail {
            if cmp_taken != other {
                return None;
            }
            cmp_cc = cmp_cc.inverted();
        }
        let compare = Self::find_convertible_compare(cmp_ops, cmp_test, used)?;
        if !Self::can_speculate(cmp_ops, compare) {
            return None;
        }
        let cmp_falls = if cmp_taken == tail { other } else { tail };
        Some(CmpConv { head, cmp_bb, tail, other, cmp_falls, head_test, head_cc, cmp_cc, compare })
    }

    /// `expectedCodeSizeDelta`.
    fn expected_code_size_delta(&self) -> i32 {
        let mut delta = 0;
        // The head's `cbz` needs a compare of its own.
        if matches!(self.head_test, Test::Compare { .. }) {
            delta += 1;
        }
        // The cmp block's branch is saved, unless it was a `cbz`.
        if !matches!(self.compare, Compare::Zero { .. }) {
            delta -= 1;
        }
        delta
    }

    /// `convert`: the cmp block goes into the head. The block removed.
    pub fn convert(
        &self,
        function: &mut MFunc,
        cfg: &mut Cfg,
        defs: &mut Defs,
    ) -> usize {
        let (head, cmp_bb, tail) = (self.head, self.cmp_bb, self.tail);
        let (head_label, cmp_label) = (cfg.labels[head].clone(), cfg.labels[cmp_bb].clone());
        // `updateTailPHIs`: the cmp block's entries go; the head's are the
        // same.
        for op in &mut function.blocks[tail].ops {
            let Op::Phi { incoming, .. } = op else { break };
            incoming.retain(|(_, from)| *from != cmp_label);
        }
        // The head's branch goes; a `cbz` leaves a compare of its own.
        let first = first_terminator(&function.blocks[head].ops);
        function.blocks[head].ops.truncate(first);
        if let Test::Compare { width, rt, .. } = self.head_test {
            let compare = Op::I(Inst::AddSubImm {
                op: AddSub { subtract: true, set_flags: true },
                width,
                rd: function.gpr(),
                rn: rt,
                imm12: 0,
                shift12: false,
            });
            defs.add(&compare, head);
            function.blocks[head].ops.push(compare);
        }
        // The cmp block's operations follow, its compare now a `ccmp` that also
        // reads the head's flags.
        let nzcv = nzcv_to_satisfy(self.cmp_cc);
        let ccmp = |negate: bool, width: Width, rn: Gpr, rm_or_imm: CcOperand| {
            Op::I(Inst::CondCompare { negate, width, rn, rm_or_imm, nzcv, cond: self.head_cc })
        };
        let ops = function.blocks[cmp_bb].ops.take();
        let first = first_terminator(&ops);
        let mut terminators: Vec<(Op, Option<llrm_codegen::model::lir::SrcLoc>)> = Vec::new();
        for (at, (op, loc)) in ops.into_iter().enumerate() {
            if at >= first {
                terminators.push((op, loc));
                continue;
            }
            let op = match self.compare {
                Compare::Flags { at: place, negate, width, rn, rm_or_imm } if place == at => {
                    ccmp(negate, width, rn, rm_or_imm)
                }
                _ => op,
            };
            defs.add(&op, head);
            function.blocks[head].ops.push_at(op, loc);
        }
        // The cmp block's branch ends the head; a `cbz` is the `ccmp #0` and
        // a branch on the flags it set.
        let mut terminators = terminators.into_iter();
        if let Compare::Zero { width, rt } = self.compare {
            let (branch, loc) = terminators.next().expect("the cmp block ends in its cbz");
            let Op::I(Inst::CompareBranch { nonzero, target, .. }) = branch else { unreachable!("a cbz") };
            function.blocks[head].ops.push_at(ccmp(false, width, rt, CcOperand::Imm5(0)), loc);
            function.blocks[head]
                .ops
                .push_at(Op::I(Inst::BranchCond { cond: if nonzero { Cond::Ne } else { Cond::Eq }, target }), loc);
        } else {
            let (branch, loc) = terminators.next().expect("the cmp block ends in a branch");
            function.blocks[head].ops.push_at(branch, loc);
        }
        // The other way out is named: the cmp block may have fallen into it.
        match terminators.next() {
            Some((jump, loc)) => function.blocks[head].ops.push_at(jump, loc),
            None => function.blocks[head]
                .ops
                .push(Op::I(Inst::Branch { link: false, target: Target::Label(cfg.labels[self.cmp_falls].clone()) })),
        }
        // The edges: the head goes where the cmp block went.
        cfg.pred[tail].retain(|&p| p != cmp_bb);
        let other = self.other;
        for p in cfg.pred[other].iter_mut().filter(|p| **p == cmp_bb) {
            *p = head;
        }
        cfg.succ[head] = std::mem::take(&mut cfg.succ[cmp_bb]);
        cfg.pred[cmp_bb].clear();
        cfg.order.retain(|&n| n != cmp_bb);
        rename_incoming(function, &cmp_label, &head_label);
        function.cmp_merged.push((head_label, cmp_label));
        cmp_bb
    }
}

/// `shouldConvert`: whether the model says the `ccmp` is worth the branch it
/// takes out.
thread_local! {
    static METRICS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has modelled a whole body for the trace metrics.
pub fn metrics_built() -> usize {
    METRICS_BUILT.with(std::cell::Cell::get)
}

/// `traces` are the trace metrics of the body, made when a candidate first
/// needs them (`MachineTraceMetrics` is lazy too): the model of every
/// instruction of the function is the cost of making them, and at minimum size
/// most candidates are decided by their code size alone.
fn should_convert<'a>(
    conv: &CmpConv,
    traces: &mut Option<tracemetrics::Traces<'a>>,
    func: &'a std::cell::OnceCell<tracemetrics::Func>,
    build: &dyn Fn() -> tracemetrics::Func,
    function: &MFunc,
    min_size: bool,
    penalty: u32,
) -> bool {
    if min_size {
        let delta = conv.expected_code_size_delta();
        if delta < 0 {
            return true;
        }
        if delta > 0 {
            return false;
        }
    }
    let traces = traces.get_or_insert_with(|| tracemetrics::Traces::new(func.get_or_init(build)));
    let trace = traces.trace(conv.cmp_bb);
    // The compare delays the branch: it waits for the second compare's inputs.
    let delay_limit = penalty * 3 / 4;
    // The head's own trace: the depths of its instructions are those of any
    // trace through it.
    let head_trace = traces.trace(conv.head);
    let head_depth = head_trace.instr_depth(traces, first_terminator(&function.blocks[conv.head].ops));
    let cmp_depth = trace.instr_depth(traces, first_terminator(&function.blocks[conv.cmp_bb].ops));
    if cmp_depth > head_depth + delay_limit {
        return false;
    }
    // The speculated operations must merge into the head's critical path.
    trace.resource_depth(traces, true) <= head_depth
}

/// `AArch64ConditionalCompares::run`: converts every candidate the model
/// accepts, the head first. The number of conversions.
pub fn run(
    function: &mut MFunc,
    min_size: bool,
    stress: bool,
) -> usize {
    if function.blocks.len() < 3 {
        return 0;
    }
    let model = crate::schedmodel::Arm64Model::generic();
    let penalty = model.mispredict_penalty();
    let mut cfg = Cfg::of(function);
    let mut defs = Defs::of(function);
    // The dominator tree's pre order: a head can convert more than once.
    let children = dominator_children(&cfg);
    let mut order = Vec::new();
    let mut stack = vec![0usize];
    while let Some(node) = stack.pop() {
        order.push(node);
        stack.extend(children[node].iter().rev());
    }
    let mut converted = 0;
    let mut position = 0;
    'epoch: loop {
        let func = std::cell::OnceCell::new();
        let mut traces = None;
        let build = || {
            METRICS_BUILT.with(|built| built.set(built.get() + 1));
            build_func(function, &cfg, &model)
        };
        let used = used_registers(function);
        while position < order.len() {
            let head = order[position];
            if !cfg.order.contains(&head) {
                position += 1;
                continue;
            }
            let Some(candidate) = CmpConv::can_convert(function, &cfg, head, &defs, &used) else {
                position += 1;
                continue;
            };
            if !stress && !should_convert(&candidate, &mut traces, &func, &build, function, min_size, penalty) {
                position += 1;
                continue;
            }
            candidate.convert(function, &mut cfg, &mut defs);
            converted += 1;
            // The same head may now end in another candidate.
            continue 'epoch;
        }
        break;
    }
    if converted > 0 {
        let alive: HashSet<usize> = cfg.order.iter().copied().collect();
        let mut at = 0;
        function
            .blocks
            .retain(
                |_| {
                    at += 1;
                    alive.contains(&(at - 1))
                },
            );
    }
    converted
}

#[cfg(test)]
mod tests;
