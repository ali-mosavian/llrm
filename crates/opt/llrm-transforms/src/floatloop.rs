//! Runs a counted loop of exact float work in memory as its last trip alone,
//! from the memory the earlier trips leave, which [`llrm_analysis::floatfacts`]
//! proves.
//!
//! # How it works
//! The loop is two blocks: a header that tests the counter and may store it,
//! and a latch of float work that steps the counter and branches back. The
//! counter is the header's one live phi, affine with a known start and step,
//! and the trip count is proven. The latch work must not read the counter, and
//! nothing the loop computes except the counter may be read after it. The exit
//! has the header as its only predecessor and no phis. Fewer than two trips
//! leave nothing to save.
//!
//! `floatfacts::repeated` replays the latch `count - 1` times over the cells
//! known before the loop. Each cell the latch reads before it writes
//! (`_carried`) gets a constant store at the top of the latch, holding what the
//! last trip reads. The final counter is `start + step * count`.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   b0["<b>b0</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   l["<b>L</b>"]:::mid
//!   x["<b>X</b>"]:::mid
//!   b0 --> h
//!   h --> l
//!   l --> h
//!   h --> x
//! ```
//!
//! *Before, a loop of 10 trips*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef opt fill:#ea580c,stroke:#9a3412,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   b0["<b>b0</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   l["<b>L</b><br/><small>store 438.75, @m (seed)<br/>one trip of the work</small>"]:::opt
//!   x["<b>X</b><br/><small>store i16 10, @cnt</small>"]:::out
//!   b0 --> h
//!   h --> l
//!   l --> x
//! ```
//!
//! *After, the last trip alone*
//!
//! The header branch becomes a jump to the latch and the latch's jump goes to
//! the exit. Uses of the counter after the loop, a phi's included, become the
//! final number through `SsaUpdater`. The header's phis lose the latch arm and,
//! left with one input, are that value. The header's counter stores are
//! repeated before the exit with the final number. What nothing reads is
//! [`crate::dead`]'s.
//!
//! # Example
//! Before, the counter stored in the header and 48.75 added ten times:
//!
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
//!   store i16 %i, ptr getelementptr (i8, ptr @m, i16 4)
//!   %c = icmp slt i16 %i, 10
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   %v = load float, ptr @m
//!   %w = fadd float %v, 4.875000e+01
//!   store float %w, ptr @m
//!   %n = add i16 %i, 1
//!   br label %b1
//! ```
//!
//! After:
//!
//! ```mir
//! b2:
//!   store float 4.387500e+02, ptr @m
//!   %v = load float, ptr @m
//!   %w = fadd float %v, 4.875000e+01
//!   store float %w, ptr @m
//!   %n = add i16 0, 1
//!   br label %b3
//!
//! b3:
//!   store i16 10, ptr getelementptr (i8, ptr @m, i16 4)
//!   ; ...
//! ```
//!
//! One trip runs from the 438.75 the first nine leave, and the counter is
//! stored as 10.
//!
//! From unit test `test_exact_loop_runs_its_last_trip_alone` in
//! `floatloop_tests.rs`.
//!
//! # Runs
//! Per-body pipeline, after `lcssa` and before `hoist`: LICM may move an
//! invariant preparation out of the loop afterwards. Skipped for a body with
//! no float value (`floatfacts::touches`) and for one with no loop of the
//! shape above whose latch touches one (`float_loop`, seen without the solve;
//! no gcc or LLVM pass is its counterpart); `LLRM_CHECK_FLOATSKIP=1` runs it
//! anyway and asserts it changed nothing. On at -O1 and up;
//! `Options::floatloop` turns it off for bisection. See
//! `docs/reference/levels.md`.
//!
//! # Needs
//! [`llrm_analysis::floatfacts`] (`manager::FloatFacts`), `consts`,
//! `induction`, `manager::writes`, and the loop shape that
//! [`crate::loopsimplify`] and [`crate::lcssa`] leave.
//!
//! # Keeps
//! Nothing when it changes a loop; everything otherwise.
//!
//! # See also
//! No LLVM or GCC counterpart. IndVarSimplify rewrites exit values and SCCP
//! folds the effects of a computable loop.

use llrm_analysis::consts::{self, Calls};
use llrm_analysis::floatfacts::{self, LoopExit, Solved};
use llrm_analysis::manager::{self, FloatFacts};
use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, induction, regions};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::types::TypeId;

use crate::lcssa::{arms, from_arms};
use crate::transform::live;

pub struct FloatLoop;

impl FunctionPass for FloatLoop {
    fn name(&self) -> &'static str {
        "floatloop"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        self.run_checking(unit, analyses, llrm_support::env_flag!("LLRM_CHECK_FLOATSKIP"))
    }
}

impl FloatLoop {
    /// `run`, `check` being `LLRM_CHECK_FLOATSKIP`.
    pub(crate) fn run_checking(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
        check: bool,
    ) -> PreservedAnalyses {
        // With no float in the body no loop is a float loop.
        // Nor does a float loop exist unless some loop has `_planned`'s shape,
        // which is seen without the solve: that is for nothing where none has.
        let none = !floatfacts::touches(unit.context, unit.function) || !self.float_loop(unit, analyses);
        if !none {
            return self.floated(unit, analyses);
        }
        if check {
            self.checked(unit, analyses);
        }
        PreservedAnalyses::all()
    }

    /// The pass run anyway, and what it asked of the analyses and changed
    /// said: on a copy of the body and with analyses of its own. What it asks
    /// of the manager (the integers `FloatFacts` solves under, `ThroughMemory`)
    /// is kept and read by every pass after it, a kept result knowing less than
    /// one asked later, so a check that asked changed the object (matmul bq32
    /// -O3: 18 KB of code against 34 KB).
    fn checked(
        &mut self,
        unit: &passes::Unit,
        analyses: &Analyses,
    ) {
        let mut context = unit.context.clone();
        let mut function = unit.function.clone();
        let mut declared = unit.declared.clone();
        let mut copy = passes::Unit {
            context: &mut context,
            layout: unit.layout,
            function: &mut function,
            id: unit.id,
            metadata: unit.metadata,
            declared: &mut declared,
        };
        let preserved = self.floated(&mut copy, &mut analyses.fresh());
        assert!(
            preserved.are_all_preserved()
                && llrm_mir::print::body(unit.context, unit.function) == llrm_mir::print::body(&context, &function),
            "LLRM_CHECK_FLOATSKIP: floatloop changed a body with no float in it"
        );
    }

    /// Whether some loop has the shape `_planned` starts from, which needs no
    /// solve to see: one exit that only the header reaches and no phi takes,
    /// a phi in the header, and a latch of float work, steps and a jump.
    fn float_loop(
        &self,
        unit: &passes::Unit,
        analyses: &mut Analyses,
    ) -> bool {
        let function = &*unit.function;
        let shape = analyses.get::<cfg::Shape>(unit.context, unit.layout, function);
        let memory = Unit::within(unit.context, unit.layout, function, analyses.outer()).with_shape(&shape);
        shape.loops.iter().any(|loop_| {
            let (header, Some(&latch)) = (cfg::block(loop_.header), loop_.latches.first()) else { return false };
            let latch = cfg::block(latch);
            let [exit] = function
                .successors(header)
                .into_iter()
                .filter(|one| !loop_.body.contains(&cfg::id(*one)))
                .collect::<Vec<_>>()[..]
            else {
                return false;
            };
            function.predecessors(exit) == [header]
                && !function.iter_block(exit).any(|i| function.instruction(i).opcode == Opcode::Phi)
                && function.iter_block(header).any(|i| function.instruction(i).opcode == Opcode::Phi)
                && function.iter_block(latch).all(|inst| {
                    let op = function.instruction(inst);
                    match op.opcode {
                        Opcode::Br => op.operands.len() == 1,
                        Opcode::Binary(_) => true,
                        _ => floatfacts::rule(&memory, inst).is_some(),
                    }
                })
                && floatfacts::touches_in(unit.context, function, &function.iter_block(latch).collect::<Vec<_>>())
        })
    }

    fn floated(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let calls = manager::writes(unit.context, unit.layout, unit.function, analyses);
        let solved = analyses.get::<FloatFacts>(unit.context, unit.layout, unit.function);
        if specialized(
            unit.context,
            unit.layout,
            analyses.outer().callees(),
            unit.function,
            analyses.outer(),
            &calls,
            &solved,
        ) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// A store to put before the last trip, or after the loop.
struct _Store {
    pointer: Operand,
    ty: TypeId,
    constant: ConstantKind,
}

/// What `_rewritten` does to one loop.
struct _Plan {
    header: BlockId,
    latch: BlockId,
    exit: BlockId,
    counter: ValueId,
    seeds: Vec<_Store>,
    finals: Vec<_Store>,
    last: u128,
}

/// The first counted exact float loop run as its last trip alone, given
/// `solved`, floatfacts' solve of `function` with `calls`. Whether one was.
#[allow(clippy::too_many_arguments)]
pub fn specialized(
    context: &Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
    calls: &Calls,
    solved: &Solved,
) -> bool {
    let plan = {
        let shape = llrm_analysis::cfg::Shape::of(function);
        let unit = Unit::within(context, layout, function, outer).with_shape(&shape);
        let alive = live(context, callees, function);
        floatfacts::exits(&unit, calls, solved)
            .into_iter()
            .filter(|proof| proof.count > BigInt::from(1))
            .find_map(|proof| _planned(&unit, calls, solved, &alive, &proof))
    };
    let Some(plan) = plan else {
        return false;
    };
    _rewritten(context, function, plan);
    true
}

/// Whether `reference` is read in `insts` before a store covers it.
fn _carried(
    unit: &Unit,
    reference: &MemRef,
    insts: &[InstId],
) -> bool {
    for &inst in insts {
        let Some(access) = MemRef::of(unit, inst) else {
            continue;
        };
        match unit.function.instruction(inst).opcode {
            Opcode::Load { .. }
                if regions::overlapping(reference, &access, None, None, unit.program).unwrap_or(true) =>
            {
                return true;
            }
            Opcode::Store { .. }
                if access.addr().is_some() && access.addr() == reference.addr() && access.width == reference.width =>
            {
                return false;
            }
            _ => {}
        }
    }
    false
}

fn _planned(
    unit: &Unit,
    calls: &Calls,
    solved: &Solved,
    alive: &llrm_mir::dense::IdSet<ValueId>,
    proof: &LoopExit,
) -> Option<_Plan> {
    let function = unit.function;
    let loop_ = unit.shape().loops.iter().find(|one| one.header == proof.header)?.clone();
    let (header, latch) = (cfg::block(loop_.header), cfg::block(*loop_.latches.first()?));
    let [exit] =
        function.successors(header).into_iter().filter(|one| !loop_.body.contains(&cfg::id(*one))).collect::<Vec<_>>()
            [..]
    else {
        return None;
    };
    if function.predecessors(exit) != [header]
        || function.iter_block(exit).any(|inst| function.instruction(inst).opcode == Opcode::Phi)
    {
        return None;
    }
    let header_insts = function.iter_block(header).collect::<Vec<_>>();
    let latch_insts = function.iter_block(latch).collect::<Vec<_>>();
    let phis = header_insts
        .iter()
        .copied()
        .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect::<Vec<_>>();
    let [phi] = phis
        .iter()
        .copied()
        .filter(|&inst| alive.contains(&function.instruction(inst).result.expect("a phi's value")))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    let counter = function.instruction(phi).result.expect("a phi's value");
    let affine = induction::basics(unit, &loop_).shift_remove(&counter)?;
    let width = unit.int_bits(Operand::Value(counter))?;
    let start = induction::_signed(&affine.start, &solved.integers, width)?;
    let step = induction::_signed(&affine.step, &solved.integers, width)?;
    let (update, _) = arms(function, phi).into_iter().find(|&(_, from)| from == latch)?;
    // The latch: float work that never reads the counter, the update, and its
    // branch.
    for &inst in &latch_insts {
        let op = function.instruction(inst);
        let reads_counter = op.operands.contains(&Operand::Value(counter));
        let fine = match op.opcode {
            Opcode::Br => op.operands.len() == 1,
            _ if floatfacts::rule(unit, inst).is_some() => !reads_counter,
            Opcode::Binary(_) => op.result.map(Operand::Value) == Some(update),
            _ => false,
        };
        if !fine {
            return None;
        }
    }
    // The header stores nothing but the counter.
    let mut finals = Vec::new();
    for &inst in &header_insts {
        let op = function.instruction(inst);
        if let Opcode::Store { .. } = op.opcode {
            if op.operands[0] != Operand::Value(counter) {
                return None;
            }
            finals.push((op.operands[1], function.value(counter).ty));
        }
    }
    // Nothing the loop computes but its counter is read outside it.
    let inside = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    let leaves = header_insts
        .iter()
        .chain(&latch_insts)
        .filter_map(|&inst| function.instruction(inst).result)
        .filter(|&value| value != counter);
    for value in leaves.collect::<Vec<_>>() {
        if function.users(value).iter().any(|one| !inside(one.user)) {
            return None;
        }
    }
    let [preheader] = function
        .predecessors(header)
        .into_iter()
        .filter(|one| !loop_.body.contains(&cfg::id(*one)))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    let before = function.terminator(preheader)?;
    let mut asked = consts::memory_queries(*unit, &solved.integers);
    let mut references = latch_insts.iter().filter_map(|&inst| MemRef::of(unit, inst)).collect::<Vec<_>>();
    references.extend(proof.stores.iter().map(|(reference, _)| reference.clone()));
    let initial = consts::_kills(
        floatfacts::cells_before(
            unit,
            calls,
            &solved.integers,
            &solved.facts,
            consts::ReadAt::End(cfg::id(preheader)),
            &references,
            &mut asked,
        ),
        before,
        &solved.integers,
        calls,
        None,
        None,
        false,
        &mut asked,
    );
    let before_last = floatfacts::repeated(
        unit,
        &latch_insts,
        &(&proof.count - 1),
        &initial,
        Some(&solved.integers),
        Some(&mut asked),
    )?;
    let mut seeds = Vec::new();
    for (reference, _) in &proof.stores {
        if !_carried(unit, reference, &latch_insts) {
            continue;
        }
        let fact = consts::_cell(&before_last, &asked.resolve(reference))?;
        let owner = latch_insts
            .iter()
            .copied()
            .find(
                |&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. })
                    && MemRef::of(unit, inst).as_ref() == Some(reference),
            )?;
        let (value, pointer) = (function.instruction(owner).operands[0], function.instruction(owner).operands[1]);
        let outside = match pointer {
            Operand::Value(one) => match function.value(one).def {
                ValueDef::Instruction(defining) => function.parent(defining) != Some(latch),
                ValueDef::Argument(_) => true,
            },
            _ => true,
        };
        if !outside {
            return None;
        }
        let bits = u64::try_from(&fact.n).ok()?;
        seeds.push(_Store { pointer, ty: unit.operand_type(value)?, constant: ConstantKind::Float(bits) });
    }
    let last = consts::masked(&(start + step * &proof.count), width);
    let last = u128::try_from(&last).expect("a masked number");
    let finals =
        finals.into_iter().map(|(pointer, ty)| _Store { pointer, ty, constant: ConstantKind::Int(last) }).collect();
    Some(_Plan { header, latch, exit, counter, seeds, finals, last })
}

/// A store of `store`'s constant at `position`.
fn _store(
    context: &Context,
    function: &mut Function,
    store: _Store,
    position: Position,
) {
    let value = context.constant(Constant { ty: store.ty, kind: store.constant });
    let void = context.types.void();
    let made = function.create_instruction(
        Opcode::Store { align: None, volatile: false },
        void,
        vec![Operand::Constant(value), store.pointer],
        Flags::default(),
        None,
    );
    function.insert(made, position).expect("a placed block");
}

fn _rewritten(
    context: &Context,
    function: &mut Function,
    plan: _Plan,
) {
    let _Plan { header, latch, exit, counter, seeds, finals, last } = plan;
    let first = function.block(latch).first().expect("an instruction");
    for seed in seeds {
        _store(context, function, seed, Position::Before(first));
    }
    let exit_first = function.block(exit).first().expect("an instruction");
    for store in finals {
        _store(context, function, store, Position::Before(exit_first));
    }
    // Header to the latch, the latch out.
    let branch = function.terminator(header).expect("a terminated header");
    function.set_operands(branch, vec![Operand::Block(latch)]);
    let back = function.terminator(latch).expect("a terminated latch");
    function.set_operands(back, vec![Operand::Block(exit)]);
    // After the loop, the counter is its final number.
    let ty = function.value(counter).ty;
    let final_ = Operand::Constant(context.int(ty, last as i128));
    let mut updater = SsaUpdater::new(ty, function.value(counter).name.as_deref());
    updater.add_available_value(latch, final_);
    let within = [header, latch];
    let outside = function
        .users(counter)
        .iter()
        .copied()
        .filter(|one| !function.parent(one.user).is_some_and(|block| within.contains(&block)))
        .collect::<Vec<_>>();
    for one in outside {
        updater.rewrite_use(context, function, one);
    }
    // The header's phis lose the back edge, each then its one input.
    for phi in function.iter_block(header).collect::<Vec<_>>() {
        if function.instruction(phi).opcode != Opcode::Phi {
            continue;
        }
        let kept = arms(function, phi).into_iter().filter(|&(_, from)| from != latch).collect::<Vec<_>>();
        let result = function.instruction(phi).result.expect("a phi's value");
        let [(value, _)] = kept[..] else {
            function.set_operands(phi, from_arms(&kept));
            continue;
        };
        function.replace_all_uses_with(result, value);
        function.erase(phi).expect("its uses were replaced");
    }
}

#[cfg(test)]
#[path = "floatloop_tests.rs"]
mod tests;
