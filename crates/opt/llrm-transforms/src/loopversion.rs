//! Loop versioning: a loop is copied for what is true of it only at run time,
//! and the copy is entered behind a test of that.
//!
//! # How it works
//! A candidate is an innermost loop in canonical form
//! ([`llrm_analysis::cfg::Canonical`]) that has something to assume, each a
//! reason that implements `Assumption`: the test it makes in the loop's entry
//! and what it tells the copy's code.
//!
//! - [`crate::versionstride`]: an index steps by an unknown stride `s` (`a[i *
//!   s]`): the test is `s == 1`, and the copy reads 1 for `s` (gcc's
//!   `-fversion-loops-for-strides`).
//! - [`crate::versionalias`]: pointers the loop reads and writes may be one
//!   object: the test is that their bytes do not meet over the loop's trips,
//!   and the copy's pointers are restrict (LLVM's `LoopVersioning` with
//!   `RuntimePointerChecking`).
//!
//! The loop is copied ([`crate::loopclone`]'s `versioned`): the entry makes the
//! tests, goes to the copy where they all hold (a branch weighed `LIKELY` to
//! 1) or to the loop as it was, and both leave to the loop's exit, whose phis
//! take the copy's values.
//!
//! ```text
//!   for i: a[i * s] = 0              if s == 1: for i: a[i] = 0      <- a fill
//!                                    else:      for i: a[i * s] = 0
//! ```
//!
//! The copy is a candidate, as in [`crate::unswitch`]: [`crate::candidate`]
//! runs the pipeline over it, with the loop's header copied and what it loads
//! unchanged hoisted ([`crate::pipeline::header_copied`]) and the same done to
//! the loop it is judged against, so what the assumption allows is done by
//! then (a fill, one counter, a load read once). It is kept only when its
//! price, spills included, is 5 percent smaller (`MIN_PERCENT`): the loop twice
//! and the test are code for what they save. At most `MAX_TRIALS` loop is tried
//! per body, of at most `MAX_LOOP_OPS` operations in a body of at most
//! `MAX_BODY_OPS`.
//!
//! `LLRM_DEBUG=version` prints each loop versioned and why each other was not.
//!
//! # Example
//! Before:
//! ```mir
//! b2:
//!   %o = mul i16 %i, %s
//!   %p = getelementptr i16, ptr %a, i16 %o
//!   store i16 0, ptr %p
//!   %i.next = add i16 %i, 1
//!   br label %b1
//! ```
//! After, in the entry `%one = icmp eq i16 %s, 1` and `br i1 %one, label %copy,
//! label %b1`, and in the copy:
//! ```mir
//! 3:
//!   %5 = mul i16 %4, 1
//!   %6 = getelementptr i16, ptr %a, i16 %5
//!   store i16 0, ptr %6
//! ```
//! which the pipeline makes a fill of `%n` words.
//!
//! From unit test `a_loop_that_steps_by_a_stride_is_copied_for_one` in
//! `loopversion_tests.rs`.
//!
//! # Runs
//! Once per body, after `unswitch`, `split`, `fuse` and `distribute` and
//! before unroll-jam, at -O3 and -Omax (`-fversion-loops`, both
//! reasons: gcc's flag is the stride's, LLVM's has none). Not run where one
//! pass is selected by name. The module declares `llrm.noalias.p0` first
//! (`DeclareRestrict`), so that every pass reads a copy's restrict pointers;
//! `dead::markers_dropped` makes them their operands before selection.
//!
//! # Needs
//! [`crate::candidate`], [`crate::loopclone`] (`versioned`), [`crate::profit`],
//! `induction`, [`llrm_analysis::access`] (the reasons').
//!
//! # Keeps
//! Nothing when it versions: the function is replaced by the candidate.
//!
//! # See also
//! gcc `gimple-loop-versioning.cc` (`class loop_versioning`); LLVM
//! `LoopVersioning.cpp` (`LoopVersioning::versionLoop`).

use std::collections::BTreeSet;

use llrm_analysis::cfg::{self, Canonical};
use llrm_analysis::manager::Registers;
use llrm_analysis::memory;
use llrm_mir::context::Context;
use llrm_mir::context::GlobalId;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Linkage, MetadataNode, MetadataOperand, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, ModuleAnalyses, ModulePass, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

use crate::candidate::{self, Baseline, Gain, Pricing};
use crate::lcssa::operations;
use crate::loopclone::{self, LoopCopy};
use crate::profit::{self, OperationCosts};
use crate::versionalias::Disjoint;
use crate::versionstride::Stride;

/// Operations of the loop that is copied.
const MAX_LOOP_OPS: usize = 128;
/// Loops tried per body, the most nested first.
const MAX_TRIALS: usize = 1;
/// Operations of a body that is tried at all: a trial runs the pipeline over
/// the body twice (the copy, and the loop it is judged against), and in
/// QCport's `d_faces` (1,038 operations) that made -O3 52% dearer to compile
/// for loops none of which it kept.
const MAX_BODY_OPS: usize = 600;
/// How often the copy is run for each time the loop is: a test that holds
/// for the loop as gcc versions it (`profile_probability::likely`).
const LIKELY: i64 = 15;
/// What the copy must save of the price, in percent: the price is a model, and
/// the loop twice with its test is code.
const MIN_PERCENT: i64 = 5;

pub struct Version {
    /// What a copy goes through before it is judged.
    pub passes: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Version {
    fn name(&self) -> &'static str {
        "version"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let costs = profit::costs(&outer);
        let passes = &mut self.passes;
        let module = std::rc::Rc::clone(&outer);
        let mut reoptimize = |candidate: &mut Unit| {
            for pass in passes.iter_mut() {
                pass.run(candidate, &mut Analyses::new(std::rc::Rc::clone(&module)));
            }
        };
        match optimized(unit, analyses, &outer, &costs, &mut reoptimize) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("version: {error}"),
        }
    }
}

/// Every loop this can copy and the price likes, up to the trial budget;
/// whether any was.
pub fn optimized(
    unit: &mut Unit,
    analyses: &mut Analyses,
    outer: &Outer,
    costs: &OperationCosts,
    reoptimize: &mut dyn FnMut(&mut Unit),
) -> Result<bool, String> {
    let mut refused = BTreeSet::new();
    let mut changed = false;
    for _ in 0..MAX_TRIALS {
        let Some(mut wanted) = found(unit, analyses, &refused) else { break };
        let header = wanted.shape.loop_.header;
        let mut copy = unit.function.clone();
        if let Err(error) = versioned(unit.context, unit.declared, &mut copy, &mut wanted) {
            llrm_support::debug!("version", "b{header}: not versioned, {error}");
            refused.insert(header);
            continue;
        }
        if candidate::adopted(
            unit,
            outer,
            costs,
            copy,
            reoptimize,
            Pricing::WithSpills,
            Gain::CheaperBy(MIN_PERCENT),
            Baseline::Reoptimized,
        ) {
            llrm_support::debug!(
                "version",
                "versioned the loop with header b{header} for {}",
                wanted.assumptions.iter().map(|one| one.name()).collect::<Vec<_>>().join(" and ")
            );
            changed = true;
            refused.clear();
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        } else {
            llrm_support::debug!(
                "version",
                "b{header}: not versioned for {}, the copy is not cheaper",
                wanted.assumptions.iter().map(|one| one.name()).collect::<Vec<_>>().join(" and ")
            );
            refused.insert(header);
        }
    }
    Ok(changed)
}

/// What a pass that tests pointers makes them restrict with: `llrm.noalias` of
/// the flat space, which the module has declared before any pass is run over
/// a body, so that a copy is judged by the passes that read what it says.
pub const RESTRICT: &str = "llrm.noalias.p0";

/// The declaration of [`RESTRICT`], made in the module where it has none.
pub struct DeclareRestrict;

impl ModulePass for DeclareRestrict {
    fn name(&self) -> &'static str {
        "declare-restrict"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        if module.named(RESTRICT).is_none() {
            let pointer = module.context.types.ptr(0);
            let ty = module
                .context
                .types
                .intern(
                    Type::Function {
                        returns: pointer,
                        parameters: vec![pointer],
                        variadic: false,
                    },
                );
            module.add_function(RESTRICT, ty, Linkage::External).expect("a name no global has");
        }
        Vec::new()
    }
}

/// What a loop's copy takes for granted: found in a loop, the test of it that
/// the entry makes, and what the copy's code is told. Each reason to version
/// a loop is one, as gcc's `loop_versioning` has its strides and LLVM's
/// `LoopVersioning` its pointer checks.
pub trait Assumption {
    /// What it assumes, for `LLRM_DEBUG=version`.
    fn name(&self) -> &'static str;

    /// The test, made before `before` in the loop's entry, that holds where
    /// the copy may run.
    fn condition(
        &mut self,
        context: &mut Context,
        declared: &mut Declared,
        function: &mut Function,
        before: InstId,
    ) -> Result<Operand, String>;

    /// `copy` made to take it for granted.
    fn assumed(
        &self,
        context: &Context,
        function: &mut Function,
        copy: &LoopCopy,
    ) -> Result<(), String>;
}

/// What the passes of this file ask of the body to find an assumption in a
/// loop of it.
pub struct Within<'a> {
    pub function: &'a Function,
    pub context: &'a Context,
    pub outer: &'a Outer,
    pub within: &'a memory::Unit<'a>,
}

/// A loop and what its copy takes for granted.
struct Wanted {
    shape: Canonical,
    assumptions: Vec<Box<dyn Assumption>>,
}

/// The first innermost loop not in `refused` that has something to assume,
/// if any.
fn found(
    unit: &mut Unit,
    analyses: &mut Analyses,
    refused: &BTreeSet<i64>,
) -> Option<Wanted> {
    let function: &Function = unit.function;
    if llrm_analysis::occurrence::operations(function).count() > MAX_BODY_OPS {
        return None;
    }
    let shape = cfg::Shape::of(function);
    let mut candidates = shape
        .loops
        .iter()
        .filter(|loop_| !refused.contains(&loop_.header))
        .filter(|loop_| {
            !shape.loops.iter().any(|inner| inner.header != loop_.header && loop_.body.is_superset(&inner.body))
        })
        .filter_map(|loop_| Canonical::of(function, loop_))
        .collect::<Vec<_>>();
    let ops =
        |one: &Canonical| one.loop_.body.iter().map(|&at| operations(function, cfg::block(at)).len()).sum::<usize>();
    candidates.retain(|one| ops(one) <= MAX_LOOP_OPS);
    if candidates.is_empty() {
        return None;
    }
    let facts = analyses.get::<Registers>(unit.context, unit.layout, function);
    let outer = std::rc::Rc::clone(analyses.outer());
    let within =
        memory::Unit::within(unit.context, unit.layout, function, &outer).with_registers(&facts).with_shape(&shape);
    let view = Within { function, context: unit.context, outer: &outer, within: &within };
    candidates
        .into_iter()
        .find_map(
            |one| {
                let assumptions = [
                    Stride::found(&view, &one).map(|one| Box::new(one) as Box<dyn Assumption>),
                    Disjoint::found(&view, &one).map(|one| Box::new(one) as Box<dyn Assumption>),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
                if assumptions.is_empty() {
                    llrm_support::debug!("version", "b{}: not versioned, nothing to assume", one.loop_.header);
                    return None;
                }
                Some(Wanted { shape: one, assumptions })
            },
        )
}

/// `function` with the loop of `wanted` copied for what it assumes, entered
/// where every test holds.
fn versioned(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    wanted: &mut Wanted,
) -> Result<(), String> {
    let bit = context.types.int(1);
    let assumptions = &mut wanted.assumptions;
    let (copy, dispatch) = loopclone::versioned(function, &wanted.shape.loop_, &mut |function, before| {
        let mut holds: Option<Operand> = None;
        for one in assumptions.iter_mut() {
            let test = one.condition(context, declared, function, before)?;
            holds = Some(match holds {
                None => test,
                Some(both) => {
                    let and = function.create_instruction(
                        Opcode::Binary(BinaryOp::And),
                        bit,
                        vec![both, test],
                        Flags::default(),
                        None,
                    );
                    function.insert(and, Position::Before(before))?;
                    Operand::Value(function.instruction(and).result.expect("an and's value"))
                }
            });
        }
        holds.ok_or_else(|| "nothing assumed".to_owned())
    })?
    .ok_or("the loop is not copied")?;
    for one in assumptions.iter() {
        one.assumed(context, function, &copy)?;
    }
    // What a loop is versioned for is what it is run with, nearly always.
    let word = context.types.int(32);
    let weight = |context: &mut Context, n: i64| MetadataOperand::Constant(context.int(word, i128::from(n)));
    let operands =
        vec![MetadataOperand::String("branch_weights".to_owned()), weight(context, LIKELY), weight(context, 1)];
    function.annotate(dispatch, "prof", declared.node(MetadataNode { distinct: false, operands }));
    Ok(())
}

#[cfg(test)]
#[path = "loopversion_tests.rs"]
mod tests;
