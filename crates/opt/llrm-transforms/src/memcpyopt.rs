//! Loads and stores that copy adjacent bytes become one `llvm.memcpy` (LLVM's
//! MemCpyOpt, which makes a call of the stores that copy a struct; gcc has no
//! such pass and leaves them to the by-pieces expansion).
//!
//! # How it works
//! In each block, a pair is a non-volatile integer load whose only user is a
//! non-volatile store of it in the same block, each at a constant offset from a
//! root pointer. A run is the pairs of one source root and one destination root
//! that tile adjacent bytes of both, the destination at the same distance from
//! the source throughout. Pairs are taken in one walk of the block: any other
//! instruction that touches memory, or a pair of other roots, ends the pairs
//! open so far, so nothing stands between a run's first load and last store but
//! work that touches no memory.
//!
//! The run is one `llvm.memcpy` of its bytes where its last store stood, when
//! two things hold. The alias owner (`regions::overlapping`, over the
//! references `Unit::annotated` gives) says no source cell and destination cell
//! of the run can meet, so the order of its loads and stores does not matter.
//! And it is cheaper: the pairs' loads and stores against what the same
//! `length` costs (`loopidioms::copy_price`: the loads and stores isel makes of
//! a short copy, else `rep movs`). Four byte pairs are one dword pair, which is
//! a gain; two dword pairs are what isel makes of the copy, which is none.
//!
//! ```text
//!   %a = load i32, ptr %s            call memcpy(%d, %s, 40)
//!   store i32 %a, ptr %d      ==>    (nine more pairs gone)
//!   ... nine more pairs
//! ```
//!
//! # Example
//! Ten dword pairs, `d[i] = s[i]`, two globals:
//!
//! ```mir
//! %a0 = load i32, ptr @s
//! store i32 %a0, ptr @d
//! %p1 = getelementptr i32, ptr @s, i32 1
//! %a1 = load i32, ptr %p1
//! ...
//! ```
//!
//! becomes `call void @llvm.memcpy.p0.p0.i32(ptr @d, ptr @s, i32 40, i1
//! false)`.
//!
//! From unit test `ten_dword_pairs_are_one_memcpy` in `memcpyopt_tests.rs`.
//!
//! # Runs
//! After the function fixed point, beside `Merge` (no pass asks a memcpy what
//! its destination holds, and `splitcopy` would split the memcpy of a local
//! again). On and off with `-ftree-loop-distribute-patterns`, as `LoopIdioms`.
//!
//! # Needs
//! `regions::overlapping` through `Unit::annotated`; the target's costs.
//!
//! # Keeps
//! The blocks and edges: only instructions change.
//!
//! # See also
//! [`crate::loopidioms`] owns `copy_price` and the loop that copies;
//! [`crate::splitcopy`] is the opposite for a local.

use llrm_analysis::memory::{MemRef, Unit};
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::regions;
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Effects};
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, Declared, FunctionPass, PreservedAnalyses};

use crate::counting;
use crate::loopidioms::{_copy, How, copy_price};

/// Pairs one run may take: each is checked against every other's.
const MAX_PAIRS: usize = 64;

pub struct MemCpyOpt {
    pub size: bool,
}

impl FunctionPass for MemCpyOpt {
    fn name(&self) -> &'static str {
        "memcpyopt"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if merged(unit, analyses, self.size) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// A load and the store of its value.
#[derive(Clone)]
struct Pair {
    load: InstId,
    store: InstId,
    from: MemRef,
    to: MemRef,
    bytes: i64,
}

/// Pairs of one source root and one destination root.
fn same_roots(
    one: &Pair,
    other: &Pair,
) -> bool {
    one.from.root == other.from.root
        && one.to.root == other.to.root
        && one.from.space == other.from.space
        && one.to.space == other.to.space
}

/// `function` with each run of pairs made one memcpy where it pays; whether
/// any was.
pub fn merged(
    body: &mut passes::Unit,
    analyses: &mut Analyses,
    size: bool,
) -> bool {
    let registers = analyses.get::<llrm_analysis::manager::Registers>(body.context, body.layout, body.function);
    let outer = analyses.outer();
    let callees = outer.callees();
    let (context, layout, function, declared) =
        (&mut *body.context, body.layout, &mut *body.function, &mut *body.declared);
    let shape = llrm_analysis::cfg::Shape::of(function);
    let runs = {
        let unit = Unit::within(context, layout, function, outer).with_shape(&shape).with_registers(&registers);
        let costs = if size { outer.target().size_costs() } else { outer.target().costs() };
        let mut found: Vec<Vec<Pair>> = Vec::new();
        let mut close = |open: &mut Vec<Pair>| {
            let taken = std::mem::take(open);
            if taken.len() >= 2 {
                found.extend(_runs(taken));
            }
        };
        for block in function.layout() {
            let mut pending: Vec<(InstId, MemRef, i64)> = Vec::new();
            let mut open: Vec<Pair> = Vec::new();
            for &inst in function.block(*block).instructions() {
                if let Some(pending_load) = _load(&unit, inst) {
                    pending.push(pending_load);
                } else if let Some((load, from, bytes)) = _paired(&unit, inst, &pending) {
                    let to = MemRef::at(&unit, function.instruction(inst).operands[1], bytes as u32);
                    let pair = Pair { load, store: inst, from, to, bytes };
                    // A pair of other roots is memory work between the run's
                    // loads and stores.
                    if open.first().is_some_and(|first| !same_roots(first, &pair)) {
                        close(&mut open);
                        pending.clear();
                    } else {
                        open.push(pair);
                    }
                } else if memory::of(unit.context, callees, function, inst) != Effects::NONE {
                    close(&mut open);
                    pending.clear();
                }
            }
            close(&mut open);
        }
        let mut kept = Vec::new();
        let mut references = None;
        for run in found {
            if !_pays(&costs, &run) {
                continue;
            }
            let all = references.get_or_insert_with(|| unit.annotated().ok());
            let apart = all
                .as_ref()
                .is_some_and(
                    |all| run.iter().all(|reader| {
                        run.iter().all(|writer| match (all.get(&reader.load), all.get(&writer.store)) {
                            (Some(read), Some(written)) => {
                                !regions::overlapping(read, written, None, None, unit.program).unwrap_or(true)
                            }
                            _ => false,
                        })
                    }),
                );
            if apart {
                llrm_support::debug!(
                    "memcpyopt",
                    "{} pairs of {} bytes made one memcpy",
                    run.len(),
                    run.iter().map(|one| one.bytes).sum::<i64>()
                );
                kept.push(run);
            }
        }
        kept
    };
    for run in &runs {
        _replaced(context, layout, function, declared, run);
    }
    !runs.is_empty()
}

/// The load `inst` is, where its value has one user, and that a store in its
/// block; its reference and bytes.
fn _load(
    unit: &Unit,
    inst: InstId,
) -> Option<(InstId, MemRef, i64)> {
    let op = unit.function.instruction(inst);
    let Opcode::Load { volatile: false, .. } = op.opcode else { return None };
    let bits = unit.context.types.int_bits(op.ty)?;
    let value = op.result?;
    let [only] = unit.function.users(value) else { return None };
    let user = unit.function.instruction(only.user);
    let stored = matches!(user.opcode, Opcode::Store { volatile: false, .. })
        && only.index == 0
        && unit.function.parent(only.user) == unit.function.parent(inst);
    let from = MemRef::at(unit, op.operands[0], bits / 8);
    (stored && bits % 8 == 0 && _plain(&from)).then_some((inst, from, i64::from(bits / 8)))
}

/// The pair the store `inst` completes, if the load it stores is one of the
/// `pending`.
fn _paired(
    unit: &Unit,
    inst: InstId,
    pending: &[(InstId, MemRef, i64)],
) -> Option<(InstId, MemRef, i64)> {
    let op = unit.function.instruction(inst);
    let Opcode::Store { volatile: false, .. } = op.opcode else { return None };
    let Operand::Value(value) = op.operands[0] else { return None };
    let ValueDef::Instruction(load) = unit.function.value(value).def else { return None };
    let found = pending.iter().find(|one| one.0 == load)?;
    let to = MemRef::at(unit, op.operands[1], found.2 as u32);
    _plain(&to).then(|| found.clone())
}

/// A constant offset from a root, in a space a string move reaches.
fn _plain(reference: &MemRef) -> bool {
    reference.root.is_some() && reference.base.is_none() && reference.segment.is_none()
}

/// `open`'s maximal runs of two or more pairs that tile adjacent bytes of both
/// sides at one distance, at most `MAX_PAIRS` each.
fn _runs(mut open: Vec<Pair>) -> Vec<Vec<Pair>> {
    open.sort_by_key(|one| one.from.disp);
    let mut runs: Vec<Vec<Pair>> = Vec::new();
    for pair in open {
        let joins = runs
            .last()
            .and_then(|run| run.last())
            .is_some_and(
                |last| last.from.disp + last.bytes == pair.from.disp
                    && last.to.disp + last.bytes == pair.to.disp
                    && runs.last().is_some_and(|run| run.len() < MAX_PAIRS),
            );
        if joins {
            runs.last_mut().expect("a run").push(pair);
        } else {
            runs.push(vec![pair]);
        }
    }
    runs.retain(|run| run.len() >= 2);
    runs
}

/// Whether one memcpy of `run`'s bytes costs less than its loads and stores.
fn _pays(
    costs: &crate::profit::OperationCosts,
    run: &[Pair],
) -> bool {
    let bytes: i64 = run.iter().map(|one| one.bytes).sum();
    run.len() as i64 * (costs.load + costs.store) > copy_price(costs, bytes, false)
}

/// `run` made one memcpy where its last store stood.
fn _replaced(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    run: &[Pair],
) {
    let bytes: i64 = run.iter().map(|one| one.bytes).sum();
    let first = &run[0];
    let (to_space, from_space) = (first.to.space, first.from.space);
    let width = layout.pointer(to_space).index_bits;
    let (callee, function_type) = _copy(context, declared, How::Apart, to_space, from_space, width);
    let to = function.instruction(first.store).operands[1];
    let from = function.instruction(first.load).operands[0];
    let count = counting::constant(context, &BigInt::from(bytes), width);
    let off = counting::constant(context, &BigInt::from(0), 1);
    let pointer = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
    let void = context.types.void();
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); 4],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let last = function
        .block(function.parent(first.store).expect("a placed store"))
        .instructions()
        .iter()
        .copied()
        .rfind(|inst| run.iter().any(|one| one.store == *inst))
        .expect("a run has stores");
    let call = function.create_instruction(
        Opcode::Call(Box::new(info)),
        void,
        vec![to, from, count, off, callee],
        Flags::default(),
        None,
    );
    function.insert(call, Position::Before(last)).expect("a placed store");
    for one in run {
        function.erase(one.store).expect("a store has no result");
        function.erase(one.load).expect("a load with no user");
    }
}

#[cfg(test)]
#[path = "memcpyopt_tests.rs"]
mod tests;
