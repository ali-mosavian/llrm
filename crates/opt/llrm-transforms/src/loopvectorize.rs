//! `LoopVectorizePass`: widens the loops the legality check accepts and the
//! planner prices, by the factors it chooses.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopVectorize.cpp`
//! (`processLoop`,
//! [L7714](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L7714))
//! over the translated parts: [`can_vectorize`] (`LoopVectorizationLegality`),
//! [`plan`] (`LoopVectorizationPlanner`) and the execution of the plan
//! (`VPlan::execute`, `InnerLoopVectorizer`). For each innermost loop of the
//! function the pass asks legality, extracts the recipes (the loop's memory
//! accesses and control), asks the planner for VF and IC and, if both agree,
//! rewrites the loop: each load becomes `IC` wide loads of `VF` lanes, each
//! store `IC` wide stores of the loaded values (the part `k` at
//! `k * VF` elements past the first), and the induction steps by `VF * IC`.
//!
//! # Example
//! ```text
//! loop: %i = phi [0, %ph], [%n, %loop]       loop: %i = phi [0, %ph], [%n, %loop]
//!       %v = load i16, gep %a, %i      =>          %wide.load = load <8 x i16>, gep %a, %i
//!       store i16 %v, gep %b, %i                   %g1 = gep (gep %a, %i), 8
//!       %n = add nuw nsw %i, 1                     %wide.load1 = load <8 x i16>, %g1 ...
//!       br (%n == 1920) ...                        %n = add nuw nsw %i, 16
//! ```
//!
//! # DEVIATE rows
//! - LVP1 the vector loop is made in place: the scalar induction is the vector
//!   loop's `index` (the pin makes `vector.ph`, `vector.body`, `middle.block`
//!   and keeps the scalar loop as the remainder). With no remainder (VP3) and a
//!   trip count VF x IC divides, the pin's output after SimplifyCFG is this
//!   loop. Exit: the epilogue (VP3).
//! - LVP2 a loop whose induction value is used after the loop, or whose exit
//!   test is not `icmp eq`/`ne` of the stepped induction against the trip
//!   count, is left alone. Exit: the vector trip count and the middle block.
//! - LVP3 recipes are loads and stores of consecutive addresses and the loop
//!   control (VP2 of the planner): a stored value is a loaded value. Exit:
//!   splat (the DAG's `BUILD_VECTOR`), binary operations, reverse accesses.
//! - LVP4 the new instructions carry no metadata (`!tbaa`, `!alias.scope`).
//!
//! # Runs
//! At `loop-vectorize` (l.1354 of `PassBuilderPipelines.cpp`), only where the
//! target takes vector values (`Machine::has_vector_mir`).

use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::ivdescriptors::InductionKind;
use llrm_analysis::loopaccess::{SymbolicStrideMap, get_ptr_stride};
use llrm_analysis::loopaccessinfo::LoopAccessInfo;
use llrm_analysis::memory;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_analysis::typebasedaa::TypeBasedAA;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::tti::Ty;
use llrm_mir::types::{Type, TypeId};

use crate::loopvectorizelegality::{Legality, can_vectorize, const_int_step};
use crate::loopvectorizeplan::{LoopFacts, Plan, Recipe, plan};

/// `LoopVectorizePass`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopVectorize {
    /// Vectorize whatever the target says of vector values (the unit tests').
    pub always: bool,
}

/// One load or store of the loop, by the instruction it widens.
#[derive(Clone, Copy, Debug)]
struct Access {
    inst: InstId,
    is_load: bool,
    /// The scalar element type.
    element: TypeId,
    align: u64,
    /// For a store, the load whose value it stores.
    stored_load: Option<InstId>,
    /// For a store of a loop-invariant value, that value (a splat).
    splat: Option<Operand>,
}

/// What the transform needs to rewrite a planned loop.
struct Widening {
    accesses: Vec<Access>,
    /// The `add` that steps the induction, and which of its operands is the
    /// constant step.
    step: InstId,
    step_operand: usize,
    index_ty: TypeId,
    /// Where a splat of an invariant value is made.
    preheader: BlockId,
}

/// Why a loop is left alone (LV: Not vectorizing: ...).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Skipped(pub &'static str);

fn element_ty(
    cx: &Cx,
    ty: TypeId,
) -> Option<Ty> {
    match cx.context.types.get(ty) {
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some(Ty::Int(*bits)),
        _ => None,
    }
}

/// The recipes of `header`'s loop, and what the transform rewrites (LVP3), or
/// why not.
fn extract(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    legality: &Legality,
) -> Result<(Vec<Recipe>, Widening), Skipped> {
    let function = cx.function;
    let header = the_loop.get_header();
    // The pin's VPlan makes its own canonical IV, so the original induction may
    // start anywhere: an integer one stepping by one is all the widening needs.
    let primary = legality
        .inductions
        .iter()
        .find(|(_, descriptor)| {
            descriptor.kind == InductionKind::Int && const_int_step(se, descriptor).is_some_and(|step| step.is_one())
        })
        .map(|(phi, _)| *phi)
        .ok_or(Skipped("No integer induction that steps by one"))?;
    if !legality.reductions.is_empty() {
        return Err(Skipped("Reductions are not widened yet"));
    }
    let primary_phi = Operand::Value(function.instruction(primary).result.expect("a phi"));
    let step = legality
        .inductions
        .iter()
        .find(|(phi, _)| *phi == primary)
        .and_then(|(_, descriptor)| descriptor.induction_bin_op)
        .ok_or(Skipped("The induction has no step instruction"))?;
    let index_ty = function.instruction(primary).ty;
    let strides = SymbolicStrideMap::default();
    let preheader = the_loop.get_loop_preheader(function).ok_or(Skipped("The loop has no preheader"))?;

    let used_outside = |inst: InstId| -> bool {
        function
            .instruction(inst)
            .result
            .is_some_and(|value| function.users(value).iter().any(|one| function.parent(one.user) != Some(header)))
    };

    // Defined before the loop (a constant, an argument, an outer value).
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => function.parent(def) != Some(header),
            _ => true,
        },
        _ => true,
    };
    let mut recipes = Vec::new();
    let mut accesses: Vec<Access> = Vec::new();
    let mut step_operand = None;
    let mut saw_exit_compare = false;
    for inst in function.iter_block(header).collect::<Vec<_>>() {
        let data = function.instruction(inst);
        match data.opcode {
            Opcode::Phi => {
                if inst != primary {
                    return Err(Skipped("A header phi other than the induction"));
                }
            }
            Opcode::GetElementPtr { .. } => {
                // An address: every use is the pointer of a load or store of
                // this loop, or the base of another address.
                let value = data.result.expect("a gep");
                let only_addresses = function
                    .users(value)
                    .iter()
                    .all(
                        |one| {
                            let user = function.instruction(one.user);
                            function.parent(one.user) == Some(header)
                                && match user.opcode {
                                    Opcode::Load { .. } => one.index == 0,
                                    Opcode::Store { .. } => one.index == 1,
                                    // An address built on this one (the base
                                    // operand).
                                    Opcode::GetElementPtr { .. } => one.index == 0,
                                    _ => false,
                                }
                        },
                    );
                if !only_addresses {
                    return Err(Skipped("An address used other than by a load or store"));
                }
            }
            Opcode::Load { align, volatile } | Opcode::Store { align, volatile } => {
                let is_load = matches!(data.opcode, Opcode::Load { .. });
                if volatile {
                    return Err(Skipped("A volatile access"));
                }
                let (value_ty, ptr) = if is_load {
                    (data.ty, data.operands[0])
                } else {
                    let value = data.operands[0];
                    (function.operand_type(cx.context, value).ok_or(Skipped("An untyped store"))?, data.operands[1])
                };
                let lane = element_ty(cx, value_ty).ok_or(Skipped("An element type the target has no vector of"))?;
                if get_ptr_stride(se, cx, value_ty, ptr, header, &strides, true) != Some(1) {
                    return Err(Skipped("An access that is not consecutive"));
                }
                let size = cx.layout.size_bits(&cx.context.types, value_ty) / 8;
                let align = align.unwrap_or(size);
                let mut stored_load = None;
                let mut splat = None;
                if is_load {
                    // Every use of the loaded value is a store of this loop
                    // (LVP3).
                    let value = data.result.expect("a load");
                    let stored = function
                        .users(value)
                        .iter()
                        .all(
                            |one| function.parent(one.user) == Some(header)
                                && matches!(function.instruction(one.user).opcode, Opcode::Store { .. })
                                && one.index == 0,
                        );
                    if !stored {
                        return Err(Skipped("A loaded value used other than by a store"));
                    }
                    recipes.push(Recipe::Load { lane, align });
                } else {
                    let operand = data.operands[0];
                    let loaded = match operand {
                        Operand::Value(value) => match function.value(value).def {
                            llrm_mir::module::ValueDef::Instruction(def)
                                if matches!(function.instruction(def).opcode, Opcode::Load { .. })
                                    && function.parent(def) == Some(header) =>
                            {
                                Some(def)
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(def) = loaded {
                        stored_load = Some(def);
                    } else if invariant(operand) {
                        // A fill: the pin broadcasts a live-in value.
                        if let Operand::Constant(c) = operand
                            && !matches!(cx.context.get(c).kind, ConstantKind::Int(_) | ConstantKind::Zero)
                        {
                            return Err(Skipped("A stored constant that is not an integer"));
                        }
                        splat = Some(operand);
                    } else {
                        return Err(Skipped("A stored value that is neither loaded nor loop-invariant"));
                    }
                    recipes.push(Recipe::Store { lane, align });
                }
                accesses.push(Access { inst, is_load, element: value_ty, align, stored_load, splat });
            }
            Opcode::Binary(BinaryOp::Add) if inst != step => {
                // An index offset: only addresses use it (the accesses use the
                // gep, not the sum).
                let value = data.result.expect("an add");
                let offsets_address = matches!(data.operands[1], Operand::Constant(_))
                    && function.users(value).iter().all(|one| {
                        function.parent(one.user) == Some(header)
                            && matches!(
                                function.instruction(one.user).opcode,
                                Opcode::GetElementPtr { .. }
                            )
                    });
                if !offsets_address {
                    return Err(Skipped("An instruction the vectorizer does not widen yet"));
                }
            }
            Opcode::Binary(BinaryOp::Add) if inst == step => {
                if used_outside(inst) {
                    return Err(Skipped("The induction is used after the loop"));
                }
                // `%i + 1`: the phi and the constant step, in either order.
                let (a, b) = (data.operands[0], data.operands[1]);
                step_operand = if a == primary_phi && matches!(b, Operand::Constant(_)) {
                    Some(1)
                } else if b == primary_phi && matches!(a, Operand::Constant(_)) {
                    Some(0)
                } else {
                    None
                };
                if step_operand.is_none() {
                    return Err(Skipped("The induction step is not a constant"));
                }
            }
            Opcode::ICmp(predicate) => {
                // The exit test: the stepped induction against a loop-invariant
                // bound (LVP2). The constant trip count (VP3) fixes where the
                // widened induction meets it.
                let stepped = function.instruction(step).result.map(Operand::Value);
                let (a, b) = (data.operands[0], data.operands[1]);
                let exit_compare = (Some(a) == stepped && invariant(b)) || (Some(b) == stepped && invariant(a));
                if !exit_compare || !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
                    return Err(Skipped("The exit test is not the stepped induction against a loop-invariant bound"));
                }
                saw_exit_compare = true;
            }
            Opcode::Br => {}
            _ => return Err(Skipped("An instruction the vectorizer does not widen yet")),
        }
    }
    let step_operand = step_operand.ok_or(Skipped("The induction step was not found"))?;
    if !saw_exit_compare {
        return Err(Skipped("No exit compare"));
    }
    if accesses.is_empty() {
        return Err(Skipped("No memory access to widen"));
    }
    Ok((recipes, Widening { accesses, step, step_operand, index_ty, preheader }))
}

fn int_constant(
    context: &llrm_mir::context::Context,
    ty: TypeId,
    value: u64,
) -> Operand {
    Operand::Constant(context.int(ty, i128::from(value)))
}

/// `Builder.CreateVectorSplat(vf, scalar)` in the preheader: a constant
/// splat, or the `insertelement` + `shufflevector` the pin hoists
/// (`broadcast.splatinsert`, `broadcast.splat`).
fn broadcast(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    preheader: BlockId,
    vector_ty: TypeId,
    vf: u32,
    scalar: Operand,
) -> Operand {
    if let Operand::Constant(c) = scalar {
        let kind = match context.get(c).kind.clone() {
            ConstantKind::Int(0) | ConstantKind::Zero => ConstantKind::Zero,
            _ => ConstantKind::Aggregate(vec![c; vf as usize]),
        };
        return Operand::Constant(context.constant(Constant { ty: vector_ty, kind }));
    }
    let before = Position::Before(function.terminator(preheader).expect("a terminated preheader"));
    let poison = Operand::Constant(context.constant(Constant { ty: vector_ty, kind: ConstantKind::Poison }));
    let zero = Operand::Constant(context.int(context.types.intern(Type::Int(64)), 0));
    let inserted =
        function.create_instruction(Opcode::InsertElement, vector_ty, vec![poison, scalar, zero], Flags::default(), Some("broadcast.splatinsert"));
    function.insert(inserted, before).expect("the preheader");
    let inserted = Operand::Value(function.instruction(inserted).result.expect("an insertelement"));
    let shuffled = function.create_instruction(
        Opcode::ShuffleVector(vec![0; vf as usize]),
        vector_ty,
        vec![inserted, poison],
        Flags::default(),
        Some("broadcast.splat"),
    );
    function.insert(shuffled, before).expect("the preheader");
    Operand::Value(function.instruction(shuffled).result.expect("a shufflevector"))
}

/// `VPlan::execute` for the plan: IC parts of each access, the induction
/// stepped by VF x IC.
fn execute(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    widening: &Widening,
    plan: Plan,
) {
    let vector_of = |context: &llrm_mir::context::Context, element: TypeId| {
        context.types.intern(Type::Vector { element, count: plan.vf })
    };
    let mut wide: Vec<(InstId, Vec<ValueId>)> = Vec::new();
    let mut erased: Vec<InstId> = Vec::new();
    let mut splats: Vec<(Operand, Operand)> = Vec::new();
    for access in &widening.accesses {
        let before = Position::Before(access.inst);
        let void_ty = function.instruction(widening.accesses.iter().find(|one| !one.is_load).expect("a store").inst).ty;
        let vector_ty = vector_of(context, access.element);
        let pointer = if access.is_load {
            function.instruction(access.inst).operands[0]
        } else {
            function.instruction(access.inst).operands[1]
        };
        let Operand::Value(pointer_value) = pointer else { unreachable!("an address is a gep") };
        let gep_flags = function
            .instruction(match function.value(pointer_value).def {
                llrm_mir::module::ValueDef::Instruction(def) => def,
                _ => unreachable!("an address is a gep"),
            })
            .flags;
        let ptr_ty = function.value(pointer_value).ty;
        let mut parts = Vec::new();
        for part in 0..plan.ic {
            // Part 0 uses the scalar address; part k is k x VF elements on from
            // it.
            let address = if part == 0 {
                pointer
            } else {
                let offset = int_constant(context, widening.index_ty, u64::from(part) * u64::from(plan.vf));
                let gep = function.create_instruction(
                    Opcode::GetElementPtr { source: access.element },
                    ptr_ty,
                    vec![pointer, offset],
                    gep_flags,
                    Some("gep.part"),
                );
                function.insert(gep, before).expect("the access's block");
                Operand::Value(function.instruction(gep).result.expect("a gep"))
            };
            if access.is_load {
                let load = function.create_instruction(
                    Opcode::Load { align: Some(access.align), volatile: false },
                    vector_ty,
                    vec![address],
                    Flags::default(),
                    Some("wide.load"),
                );
                function.insert(load, before).expect("the access's block");
                parts.push(function.instruction(load).result.expect("a load"));
            } else {
                let value = match (access.stored_load, access.splat) {
                    (Some(source), _) => Operand::Value(
                        wide.iter().find(|(old, _)| *old == source).expect("the load comes first").1[part as usize],
                    ),
                    (None, Some(scalar)) => match splats.iter().find(|(one, _)| *one == scalar) {
                        Some((_, made)) => *made,
                        None => {
                            let made = broadcast(context, function, widening.preheader, vector_ty, plan.vf, scalar);
                            splats.push((scalar, made));
                            made
                        }
                    },
                    (None, None) => unreachable!("a store stores a load or a splat"),
                };
                let store = function.create_instruction(
                    Opcode::Store { align: Some(access.align), volatile: false },
                    void_ty,
                    vec![value, address],
                    Flags::default(),
                    None,
                );
                function.insert(store, before).expect("the access's block");
            }
        }
        if access.is_load {
            wide.push((access.inst, parts));
        }
        erased.push(access.inst);
    }
    // The stores first (they use the loads), then the loads.
    for inst in erased.iter().rev() {
        function.erase(*inst).expect("an access nothing uses");
    }
    // The induction steps by VF x IC.
    let step = int_constant(context, widening.index_ty, u64::from(plan.vf) * u64::from(plan.ic));
    function.set_operand(widening.step, widening.step_operand, step);
}

impl FunctionPass for LoopVectorize {
    fn name(&self) -> &'static str {
        "loop-vectorize"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-vectorize")
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let target = outer.target();
        if !self.always && !target.has_vector_mir() {
            return PreservedAnalyses::all();
        }
        let Some(tti) = target.tti() else { return PreservedAnalyses::all() };
        if tti.vector_register_bits() == 0 {
            return PreservedAnalyses::all();
        }
        let opt_size = has_opt_size(unit.function);
        let mut changed = crate::loopsimplify::simplified(unit.function);
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let worklist: Vec<BlockId> = {
            let (_, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
            env.loops
                .get_loops_in_preorder()
                .into_iter()
                .filter(|one| one.is_innermost())
                .map(|one| one.get_header())
                .collect()
        };
        for header in worklist {
            let planned = {
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let cx = env.cx(&*unit.context, &*unit.function);
                let Some(found) = cx.loops.get_loop_for(header) else { continue };
                let memory_unit = memory::Unit::within(&*unit.context, unit.layout, &*unit.function, &outer);
                let mut aa = AggregateAA::new();
                aa.add_aa_result(BasicAA::new(&memory_unit));
                aa.add_aa_result(llrm_analysis::scopednoaliasaa::ScopedNoAliasAA::new(&memory_unit));
                aa.add_aa_result(TypeBasedAA::new(&memory_unit));
                let max_width = tti.vector_register_bits() * 2;
                let lai = LoopAccessInfo::new(se, &cx, header, &mut aa, false, max_width);
                let legality = match can_vectorize(&cx, se, &lai, found) {
                    Ok(legality) => legality,
                    Err(refusal) => {
                        why(unit, &outer, header, &format!("legality: {refusal:?}"));
                        continue;
                    }
                };
                let trip_count = u64::from(se.get_small_constant_trip_count(&cx, header));
                let (recipes, widening) = match extract(&cx, se, found, &legality) {
                    Ok(found) => found,
                    Err(Skipped(reason)) => {
                        why(unit, &outer, header, &format!("recipes: {reason}"));
                        continue;
                    }
                };
                let dep = lai.dep_checker();
                let facts = LoopFacts {
                    recipes,
                    trip_count,
                    max_safe_width_in_bits: dep.max_safe_vector_width_in_bits(),
                    safe_for_any_vector_width: dep.is_safe_for_any_vector_width(),
                    store_load_forward_bits: (!dep.is_safe_for_any_store_load_forward_distances())
                        .then(|| dep.store_load_forward_safe_distance_in_bits()),
                    widest_induction_bits: legality.widest_induction_bits.unwrap_or(64),
                    opt_size,
                };
                match plan(&*tti, &facts) {
                    Ok(chosen) => Some((chosen, widening)),
                    Err(not) => {
                        why(unit, &outer, header, &format!("plan: {not:?}"));
                        None
                    }
                }
            };
            let Some((chosen, widening)) = planned else { continue };
            why(unit, &outer, header, &format!("vectorized VF {} IC {}", chosen.vf, chosen.ic));
            execute(unit.context, unit.function, &widening, chosen);
            changed = true;
            // The loop's body is new: the expressions are stale.
            session = ScevSession::of(unit.context, unit.function, &outer);
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `LLVM_DEBUG` of LoopVectorize (D3), on `LLRM_DEBUG=loop-vectorize`: the
/// verdict on the loop.
fn why(
    unit: &passes::Unit,
    outer: &passes::Outer,
    header: BlockId,
    verdict: &str,
) {
    if llrm_support::debug::enabled("loop-vectorize") {
        let owner = unit.id.and_then(|id| (*outer.globals[id.0 as usize].name).clone()).unwrap_or_default();
        let labels = llrm_mir::print::block_labels(unit.function);
        eprintln!("LV: @{owner} %{}: {verdict}", labels[&header]);
    }
}

/// `Function::hasOptSize`.
fn has_opt_size(function: &Function) -> bool {
    use llrm_mir::opcode::Attribute;
    function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                Attribute::Flag(name) if name == "optsize" || name == "minsize"
            ),
        )
}

#[cfg(test)]
#[path = "loopvectorize_tests.rs"]
mod tests;
