//! `LoopLoadElimination`: a load of what the previous iteration stored is
//! replaced by the stored value, carried round the loop in a phi.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopLoadElimination.cpp`. For each
//! innermost loop in rotated form with one exiting block, [`LoopAccessInfo`]
//! lists the dependences between its memory accesses. A store whose value a
//! load reads one iteration later (`a[i + 1] = x; ... = a[i]`, a distance of
//! one element, the load unconditional, the store on every path to the latch)
//! is a candidate, unless the load has another unsafe dependence or several
//! stores forward to it. Stores that may overwrite the location in between
//! (the pointers written on the path from the first forwarding store to the
//! last forwarded load) need run-time checks; more than one check for each
//! candidate on average is too many, and none is made when optimizing for
//! size. The loop is then versioned ([`crate::loopversioning`]) and, in the
//! loop that is kept, each candidate load is replaced by a phi of the value
//! loaded in the preheader (`load_initial`) and the stored value.
//!
//! # Example
//! ```text
//! loop:  %x = load a[i];  ...  store %y, a[i + 1]
//! ```
//! becomes
//! ```text
//! ph:    %load_initial = load a[0]
//! loop:  %store_forwarded = phi [ %load_initial, %ph ], [ %y, %latch ]; ...
//! ```
//!
//! # DEVIATE 24 (no predicated SCEV)
//! `LAI.getPSE().getPredicate()` is always true, so its complexity check and
//! the versioning for SCEV predicates are absent.
//!
//! # DEVIATE 33 (no profile)
//! `shouldOptimizeForSize` is the function's `optsize`/`minsize`: there is no
//! `ProfileSummaryInfo` or `BlockFrequencyInfo`.
//!
//! # Runs
//! In the parity pipeline at `loop-load-elim` (l.1400 of
//! `PassBuilderPipelines.cpp`, after `infer-alignment`).

use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::loopaccess::{SymbolicStrideMap, get_ptr_stride};
use llrm_analysis::loopaccessinfo::LoopAccessInfo;
use llrm_analysis::memdepchecker::DepType;
use llrm_analysis::memory;
use llrm_analysis::runtimecheck::RuntimePointerCheck;
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_analysis::scev::expander::Expander;
use llrm_analysis::scev::expr::{Kind, Ty};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_analysis::typebasedaa::TypeBasedAA;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::HashSet;

use crate::loopversioning::{LoopVersioning, is_recursively_lcssa_form};
use crate::runtimechecks::Rewriter;

/// `-runtime-check-per-loop-load-elim`.
const CHECK_PER_ELIM: usize = 1;

/// `LoopLoadEliminationPass`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopLoadElim {
    /// Twice the target's fixed vector register width, in bits
    /// (`MaxTargetVectorWidthInBits`): 0 where there are no vector registers.
    pub max_target_vector_width_in_bits: u32,
}

/// `StoreToLoadForwardingCandidate`: a store whose value a load reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    load: InstId,
    store: InstId,
}

fn pointer_operand(
    function: &Function,
    inst: InstId,
) -> Operand {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { .. } => instruction.operands[0],
        Opcode::Store { .. } => instruction.operands[1],
        _ => unreachable!("a load or a store"),
    }
}

fn is_load(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Load { .. })
}

impl Candidate {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L91
    /// `isDependenceDistanceOfOne`: whether the dependence from the store to
    /// the load has an absolute distance of one. E.g. `A[i+1] = A[i]` (or
    /// `A[i-1] = A[i]` for descending loop).
    fn is_dependence_distance_of_one(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
    ) -> bool {
        let load_ptr = pointer_operand(cx.function, self.load);
        let store_ptr = pointer_operand(cx.function, self.store);
        let load_type = llrm_analysis::loopaccess::load_store_type(cx, self.load).expect("a load");
        let strides = SymbolicStrideMap::default();

        let stride_load = get_ptr_stride(se, cx, load_type, load_ptr, lp, &strides, true).unwrap_or(0);
        let stride_store = get_ptr_stride(se, cx, load_type, store_ptr, lp, &strides, true).unwrap_or(0);
        if stride_load == 0 || stride_store == 0 || stride_load != stride_store {
            return false;
        }

        // TODO: This check for stride values other than 1 and -1 can be
        // eliminated. However, doing so may cause the LoopAccessAnalysis to
        // overcompensate, generating numerous non-wrap runtime checks that may
        // undermine the benefits of load elimination.
        if stride_load.unsigned_abs() != 1 {
            return false;
        }

        let type_byte_size = cx.layout.alloc_size(&cx.context.types, load_type) as i64;

        let load_ptr_scev = se.get_scev(cx, load_ptr);
        let store_ptr_scev = se.get_scev(cx, store_ptr);
        debug_assert!(
            matches!(se.node(load_ptr_scev).kind, Kind::AddRec(..)),
            "Loading from something other than indvar?"
        );
        debug_assert!(
            matches!(se.node(store_ptr_scev).kind, Kind::AddRec(..)),
            "Storing to something other than indvar?"
        );

        // We don't need to check non-wrapping here because forward/backward
        // dependence wouldn't be valid if these weren't monotonic accesses.
        use llrm_analysis::scev::expr::Use;
        let dist = se.get_minus_scev(cx, Use::of(store_ptr_scev), Use::of(load_ptr_scev), 0, 0);
        let Some(value) = se.as_constant(dist) else { return false };
        let value = llrm_mir::context::signed(value.bits, value.width);
        value == i128::from(type_byte_size) * i128::from(stride_load)
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L151
/// `doesStoreDominatesAllLatches`: the store dominates all latches, so as long
/// as there is no intervening store this value will be loaded in the next
/// iteration.
fn does_store_dominate_all_latches(
    cx: &Cx,
    store_block: BlockId,
    lp: BlockId,
) -> bool {
    let found = cx.loops.get_loop_for(lp).expect("the header of a loop");
    found.get_loop_latches(cx.function).iter().all(|&latch| cx.tree.dominates(store_block, latch))
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L161
/// `isLoadConditional`: the load is not executed on all paths in the loop.
fn is_load_conditional(
    function: &Function,
    load: InstId,
    lp: BlockId,
) -> bool {
    function.parent(load) != Some(lp)
}

/// What was decided for a loop, before anything is changed.
struct Plan {
    candidates: Vec<Candidate>,
    checks: Vec<RuntimePointerCheck>,
}

/// `LoadEliminationForLoop`, up to the point of no return.
struct LoadEliminationForLoop<'a> {
    lp: BlockId,
    lai: &'a LoopAccessInfo,
    /// Maps the load/store instructions to their index according to program
    /// order.
    inst_order: IdMap<InstId, u32>,
}

impl LoadEliminationForLoop<'_> {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L186
    /// `findStoreToLoadDependences`: look through the loop-carried and
    /// loop-independent dependences in this loop and find store->load
    /// dependences. No candidate is returned if LAA has failed to analyze the
    /// loop (e.g. if it's not bottom-tested, contains volatile memops, etc.).
    fn find_store_to_load_dependences(
        &self,
        cx: &Cx,
    ) -> Vec<Candidate> {
        // `std::forward_list::emplace_front`: the last found comes first.
        let mut candidates: Vec<Candidate> = Vec::new();

        let dep_checker = self.lai.dep_checker();
        let Some(deps) = dep_checker.dependences() else { return candidates };

        // Find store->load dependences (consequently true dep). Both
        // lexically forward and backward dependences qualify. Disqualify loads
        // that have other unsafe dependences.
        let mut loads_with_unsafe_dependence = IdSet::<InstId>::new();

        for dep in deps {
            let mut source = dep_checker.source(dep);
            let mut destination = dep_checker.destination(dep);

            if matches!(
                dep.ty,
                DepType::Unknown | DepType::IndirectUnsafe | DepType::InvariantUnsafe
            ) {
                if is_load(cx.function, source) {
                    loads_with_unsafe_dependence.insert(source);
                }
                if is_load(cx.function, destination) {
                    loads_with_unsafe_dependence.insert(destination);
                }
                continue;
            }

            if dep.is_backward() {
                // Note that the designations source and destination follow
                // the program order, i.e. source is always first. (The
                // direction is given by the DepType.)
                std::mem::swap(&mut source, &mut destination);
            } else {
                debug_assert!(dep.is_forward(), "Needs to be a forward dependence");
            }

            if !matches!(cx.function.instruction(source).opcode, Opcode::Store { .. }) {
                continue;
            }
            if !is_load(cx.function, destination) {
                continue;
            }
            let (store, load) = (source, destination);

            // Only propagate if the stored values are bit/pointer castable.
            let store_ty = llrm_analysis::loopaccess::load_store_type(cx, store).expect("a store");
            let load_ty = llrm_analysis::loopaccess::load_store_type(cx, load).expect("a load");
            if !cx.layout.is_bit_or_noop_pointer_castable(&cx.context.types, store_ty, load_ty) {
                // This store may partially clobber the value from another
                // forwarding candidate.
                loads_with_unsafe_dependence.insert(load);
                continue;
            }

            candidates.insert(0, Candidate { load, store });
        }

        if !loads_with_unsafe_dependence.is_empty() {
            candidates.retain(|candidate| !loads_with_unsafe_dependence.contains(&candidate.load));
        }

        candidates
    }

    /// `getInstrIndex`: the index of the instruction according to program
    /// order.
    fn instr_index(
        &self,
        inst: InstId,
    ) -> u32 {
        self.inst_order[&inst]
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L267
    /// `removeDependencesFromMultipleStores`: if a load has multiple
    /// candidates associated (i.e. different stores), it means that it could
    /// be forwarding from multiple stores depending on control flow. Remove
    /// these candidates. Here, we rely on LAA to include the relevant
    /// loop-independent dependences.
    fn remove_dependences_from_multiple_stores(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        candidates: &mut Vec<Candidate>,
    ) {
        // If the entry is `None` it means that we have multiple stores
        // forwarding to this load.
        let mut load_to_single_cand: IdMap<InstId, Option<usize>> = IdMap::new();

        for (index, cand) in candidates.iter().enumerate() {
            let Some(&entry) = load_to_single_cand.get(&cand.load) else {
                load_to_single_cand.insert(cand.load, Some(index));
                continue;
            };
            // Already multiple stores forward to this load.
            let Some(other) = entry else { continue };
            let other_cand = candidates[other];

            // Handle the very basic case when the two stores are in the same
            // block so deciding which one forwards is easy. The later one
            // forwards as long as they both have a dependence distance of one
            // to the load.
            if cx.function.parent(cand.store) == cx.function.parent(other_cand.store)
                && cand.is_dependence_distance_of_one(se, cx, self.lp)
                && other_cand.is_dependence_distance_of_one(se, cx, self.lp)
            {
                // They are in the same block, the later one will forward to
                // the load.
                if self.instr_index(other_cand.store) < self.instr_index(cand.store) {
                    load_to_single_cand.insert(cand.load, Some(index));
                }
            } else {
                load_to_single_cand.insert(cand.load, None);
            }
        }

        let keep: Vec<bool> = candidates
            .iter()
            .enumerate()
            .map(|(index, cand)| load_to_single_cand.get(&cand.load).copied().flatten() == Some(index))
            .collect();
        let mut at = 0;
        candidates.retain(|_| {
            at += 1;
            keep[at - 1]
        });
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L356
    /// `needsChecking`: given two pointers operations by their
    /// RuntimePointerChecking indices, whether they require an alias check:
    /// if one is a pointer for a candidate load and the other is a pointer
    /// for a possibly intervening store.
    fn needs_checking(
        &self,
        ptr_idx1: usize,
        ptr_idx2: usize,
        ptrs_written_on_fwding_path: &HashSet<Operand>,
        cand_load_ptrs: &HashSet<Operand>,
    ) -> bool {
        let rtc = self.lai.runtime_pointer_checking();
        let ptr1 = rtc.pointers[ptr_idx1].pointer_value;
        let ptr2 = rtc.pointers[ptr_idx2].pointer_value;
        (ptrs_written_on_fwding_path.contains(&ptr1) && cand_load_ptrs.contains(&ptr2))
            || (ptrs_written_on_fwding_path.contains(&ptr2) && cand_load_ptrs.contains(&ptr1))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L371
    /// `findPointersWrittenOnForwardingPath`: pointers that are possibly
    /// written to on the path from a forwarding store to a load. These
    /// pointers need to be alias-checked against the forwarding candidates.
    fn find_pointers_written_on_forwarding_path(
        &self,
        cx: &Cx,
        candidates: &[Candidate],
    ) -> HashSet<Operand> {
        // From FirstStore to LastLoad neither of the elimination candidate
        // loads should overlap with any of the stores.
        //
        // E.g.:
        //
        // st1 C[i]
        // ld1 B[i] <-------,
        // ld0 A[i] <----,  |              * LastLoad
        // ...           |  |
        // st2 E[i]      |  |
        // st3 B[i+1] -- | -'              * FirstStore
        // st0 A[i+1] ---'
        // st4 D[i]
        //
        // st0 forwards to ld0 if the accesses in st4 and st1 don't overlap
        // with ld0.
        //
        // `max_element` returns the first of equal elements' last;
        // `min_element` the first.
        let mut last_load = candidates[0].load;
        for cand in candidates {
            if self.instr_index(last_load) <= self.instr_index(cand.load) {
                last_load = cand.load;
            }
        }
        let mut first_store = candidates[0].store;
        for cand in candidates {
            if self.instr_index(cand.store) < self.instr_index(first_store) {
                first_store = cand.store;
            }
        }

        // We're looking for stores after the first forwarding store until the
        // end of the loop, then from the beginning of the loop until the last
        // forwarded-to load. Collect the pointer for the stores.
        let mut ptrs_written_on_fwding_path: HashSet<Operand> = HashSet::default();

        let mut insert_store_ptr = |inst: InstId| {
            if matches!(cx.function.instruction(inst).opcode, Opcode::Store { .. }) {
                ptrs_written_on_fwding_path.insert(pointer_operand(cx.function, inst));
            }
        };
        let mem_instrs = self.lai.dep_checker().memory_instructions();
        for &inst in &mem_instrs[self.instr_index(first_store) as usize + 1..] {
            insert_store_ptr(inst);
        }
        for &inst in &mem_instrs[..self.instr_index(last_load) as usize] {
            insert_store_ptr(inst);
        }

        ptrs_written_on_fwding_path
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L412
    /// `collectMemchecks`: the pointer alias checks to prove that there are
    /// no intervening stores.
    fn collect_memchecks(
        &self,
        cx: &Cx,
        candidates: &[Candidate],
    ) -> Vec<RuntimePointerCheck> {
        let ptrs_written_on_fwding_path = self.find_pointers_written_on_forwarding_path(cx, candidates);

        // Collect the pointers of the candidate loads.
        let cand_load_ptrs: HashSet<Operand> =
            candidates.iter().map(|candidate| pointer_operand(cx.function, candidate.load)).collect();

        let rtc = self.lai.runtime_pointer_checking();
        rtc.checks
            .iter()
            .filter(|&&(first, second)| {
                rtc.checking_groups[first]
                    .members
                    .iter()
                    .any(
                        |&ptr_idx1| rtc.checking_groups[second].members.iter().any(|&ptr_idx2| {
                            self.needs_checking(ptr_idx1, ptr_idx2, &ptrs_written_on_fwding_path, &cand_load_ptrs)
                        }),
                    )
            })
            .copied()
            .collect()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L521
    /// `processLoop` up to the point of no return: the candidates to forward
    /// and the checks that versioning must make, or none where the loop is not
    /// worth changing.
    fn plan(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        optimize_for_size: bool,
    ) -> Option<Plan> {
        // First start with store->load dependences.
        let mut store_to_load_dependences = self.find_store_to_load_dependences(cx);
        if store_to_load_dependences.is_empty() {
            return None;
        }

        // Generate an index for each load and store according to the original
        // program order. This will be used later.
        self.inst_order = self
            .lai
            .dep_checker()
            .memory_instructions()
            .iter()
            .enumerate()
            .map(|(index, &inst)| (inst, index as u32))
            .collect();

        // To keep things simple for now, remove those where the load is
        // potentially fed by multiple stores.
        self.remove_dependences_from_multiple_stores(se, cx, &mut store_to_load_dependences);
        if store_to_load_dependences.is_empty() {
            return None;
        }

        // Filter the candidates further.
        let mut candidates: Vec<Candidate> = Vec::new();
        for cand in &store_to_load_dependences {
            // Make sure that the stored values is available everywhere in the
            // loop in the next iteration.
            let store_block = cx.function.parent(cand.store).expect("a block");
            if !does_store_dominate_all_latches(cx, store_block, self.lp) {
                continue;
            }

            // If the load is conditional we can't hoist its 0-iteration
            // instance to the preheader because that would make it
            // unconditional. Thus we would access a memory location that the
            // original loop did not access.
            if is_load_conditional(cx.function, cand.load, self.lp) {
                continue;
            }

            // Check whether the SCEV difference is the same as the induction
            // step, thus we load the value in the next iteration.
            if !cand.is_dependence_distance_of_one(se, cx, self.lp) {
                continue;
            }

            candidates.push(*cand);
        }
        if candidates.is_empty() {
            return None;
        }

        // Check intervening may-alias stores. These need runtime checks for
        // alias disambiguation.
        let checks = self.collect_memchecks(cx, &candidates);

        // Too many checks are likely to outweigh the benefits of forwarding.
        if checks.len() > candidates.len() * CHECK_PER_ELIM {
            return None;
        }

        // `LAI.getPSE().getPredicate()` complexity: always 0 here (D24).

        let found = cx.loops.get_loop_for(self.lp).expect("the header of a loop");
        if !found.is_loop_simplify_form(cx.function) {
            return None;
        }

        if !checks.is_empty() {
            if self.lai.has_convergent_op() {
                // Versioning is needed but not allowed with convergent calls.
                return None;
            }

            if optimize_for_size {
                // Versioning is needed but not allowed when optimizing for
                // size.
                return None;
            }
        }

        Some(Plan { candidates, checks })
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopLoadElimination.cpp#L459
/// `propagateStoredValueToLoadUsers`: the value stored by the store of `cand`
/// to the users of the load; for the first iteration, the initial value of
/// the load is generated in the preheader.
fn propagate_stored_value_to_load_users(
    rw: &mut Rewriter,
    exp: &mut Expander,
    lp: BlockId,
    cand: Candidate,
) {
    // loop:
    //      %x = load %gep_i
    //         = ... %x
    //      store %y, %gep_i_plus_1
    //
    // =>
    //
    // ph:
    //      %x.initial = load %gep_0
    // loop:
    //      %x.storeforward = phi [%x.initial, %ph] [%y, %loop]
    //      %x = load %gep_i            <---- now dead
    //         = ... %x.storeforward
    //      store %y, %gep_i_plus_1
    let ptr = pointer_operand(rw.function, cand.load);
    let cx = rw.env.cx(&*rw.context, &*rw.function);
    let ptr_scev = rw.se.get_scev(&cx, ptr);
    let Kind::AddRec(operands, _) = rw.se.node(ptr_scev).kind.clone() else {
        unreachable!("Loading from something other than indvar?")
    };
    let start = operands[0].scev;
    let ph = {
        let found = rw.env.loops.get_loop_for(lp).expect("the header of a loop");
        found.get_loop_preheader(&*rw.function).expect("Preheader should exist!")
    };
    let ph_terminator = rw.function.terminator(ph).expect("a terminated preheader");
    let Ty::Ptr(space) = rw.se.type_of(start) else { unreachable!("a pointer expression") };

    exp.set_insert_point(ph_terminator);
    let initial_ptr =
        exp.expand_code_for(&rw.env, rw.context, rw.declared, rw.function, rw.se, start, Some(Ty::Ptr(space)));

    let load_inst = rw.function.instruction(cand.load);
    let load_ty = load_inst.ty;
    let Opcode::Load { align, .. } = load_inst.opcode else { unreachable!("a load") };
    let initial = rw
        .function
        .create_instruction(
            Opcode::Load { align, volatile: false },
            load_ty,
            vec![initial_ptr],
            Flags::default(),
            Some("load_initial"),
        );
    rw.function.insert(initial, Position::Before(ph_terminator)).expect("the preheader");
    let initial_value = Operand::Value(rw.function.instruction(initial).result.expect("a value"));

    let header_first = rw.function.block(lp).first().expect("a header");
    let phi =
        rw.function.create_instruction(Opcode::Phi, load_ty, Vec::new(), Flags::default(), Some("store_forwarded"));
    rw.function.insert(phi, Position::Before(header_first)).expect("the header");
    let phi_value = rw.function.instruction(phi).result.expect("a value");

    let store_value = rw.function.instruction(cand.store).operands[0];
    let store_ty = rw.function.operand_type(rw.context, store_value).expect("a typed value");
    debug_assert_eq!(
        rw.env.layout.size_bits(&rw.context.types, load_ty),
        rw.env.layout.size_bits(&rw.context.types, store_ty),
        "The type sizes should match!"
    );
    let mut forwarded = store_value;
    if load_ty != store_ty {
        let op = bit_or_pointer_cast_op(rw, store_ty, load_ty);
        let cast = rw
            .function
            .create_instruction(
                Opcode::Cast(op),
                load_ty,
                vec![store_value],
                Flags::default(),
                Some("store_forward_cast"),
            );
        rw.function.insert(cast, Position::Before(cand.store)).expect("the store");
        forwarded = Operand::Value(rw.function.instruction(cast).result.expect("a value"));
    }

    let latch = {
        let found = rw.env.loops.get_loop_for(lp).expect("the header of a loop");
        found.get_loop_latch(&*rw.function).expect("a latch")
    };
    rw.function.set_operands(phi, vec![initial_value, Operand::Block(ph), forwarded, Operand::Block(latch)]);

    let load_value = rw.function.instruction(cand.load).result.expect("a load result");
    rw.function.replace_all_uses_with(load_value, Operand::Value(phi_value));
}

/// `CastInst::CreateBitOrPointerCast`'s opcode: `ptrtoint`, `inttoptr`, or a
/// `bitcast`.
fn bit_or_pointer_cast_op(
    rw: &Rewriter,
    from: llrm_mir::types::TypeId,
    to: llrm_mir::types::TypeId,
) -> CastOp {
    use llrm_mir::types::Type;
    match (rw.context.types.get(from), rw.context.types.get(to)) {
        (Type::Pointer(_), Type::Int(_)) => CastOp::PtrToInt,
        (Type::Int(_), Type::Pointer(_)) => CastOp::IntToPtr,
        _ => CastOp::BitCast,
    }
}

/// `Function::hasOptSize` (D33).
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

impl FunctionPass for LoopLoadElim {
    fn name(&self) -> &'static str {
        "loop-load-elim"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-load-elim")
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        // `eliminateLoadsAcrossLoops`: `simplifyLoop` on every loop first.
        let mut changed = crate::loopsimplify::simplified(unit.function);

        // Build up a worklist of inner-loops to transform to avoid iterator
        // invalidation.
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        // The expressions of the passes before stand while the function is as
        // they left it.
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        if !changed {
            session.adopt(held.take());
        }
        let worklist: Vec<BlockId> = {
            let (_, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
            env.loops
                .get_loops_in_preorder()
                .into_iter()
                .filter(|one| one.is_innermost())
                .map(|one| one.get_header())
                .collect()
        };

        // Now walk the identified inner loops.
        let mut stale = false;
        for header in worklist {
            if stale {
                session = ScevSession::of(unit.context, unit.function, &outer);
                stale = false;
            }
            // The actual work is performed by LoadEliminationForLoop.
            let planned = {
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let cx = env.cx(&*unit.context, &*unit.function);
                let Some(found) = cx.loops.get_loop_for(header) else { continue };
                // Match historical behavior.
                let latch = found.get_loop_latch(cx.function);
                if !latch.is_some_and(|latch| found.is_loop_exiting(cx.function, latch))
                    || found.get_exiting_block(cx.function).is_none()
                {
                    continue;
                }
                let memory_unit = memory::Unit::within(&*unit.context, unit.layout, &*unit.function, &outer);
                let mut aa = AggregateAA::new();
                aa.add_aa_result(BasicAA::new(&memory_unit));
                aa.add_aa_result(TypeBasedAA::new(&memory_unit));
                let lai = LoopAccessInfo::new(se, &cx, header, &mut aa, false, self.max_target_vector_width_in_bits);
                let mut lel = LoadEliminationForLoop { lp: header, lai: &lai, inst_order: IdMap::new() };
                lel.plan(se, &cx, has_opt_size(cx.function)).map(|plan| (plan, lai))
            };
            let Some((plan, lai)) = planned else { continue };

            // Point of no-return, start the transformation. First, version the
            // loop if necessary.
            let mut candidates = plan.candidates;
            if !plan.checks.is_empty() {
                // Forming LCSSA is a precondition of versioning.
                let lcssa = {
                    let (_, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                    let found = env.loops.get_loop_for(header).expect("the loop");
                    is_recursively_lcssa_form(&*unit.function, env.loops, env.tree, found)
                };
                if !lcssa {
                    let _ = crate::lcssa::formed(unit.function);
                    session = ScevSession::of(unit.context, unit.function, &outer);
                }
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let mut rw = Rewriter {
                    context: unit.context,
                    declared: unit.declared,
                    function: unit.function,
                    se,
                    env,
                    callees: outer.callees(),
                };
                let mut lv = LoopVersioning::new(
                    lai.runtime_pointer_checking(),
                    &plan.checks,
                    lai.dep_checker().memory_instructions(),
                    header,
                );
                lv.version_loop(&mut rw);
                // The CFG changed: the analyses are found again.
                session = ScevSession::of(unit.context, unit.function, &outer);

                // After versioning, some of the candidates' pointers could
                // stop being SCEVAddRecs. We need to filter them out.
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let cx = env.cx(&*unit.context, &*unit.function);
                candidates.retain(|cand| {
                    [cand.load, cand.store]
                        .into_iter()
                        .all(
                            |inst| {
                                let ptr = pointer_operand(cx.function, inst);
                                let scev = se.get_scev(&cx, ptr);
                                matches!(se.node(scev).kind, Kind::AddRec(..))
                            },
                        )
                });
            }

            // Next, propagate the value stored by the store to the users of
            // the load. Also for the first iteration, generate the initial
            // value of the load.
            {
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let mut rw = Rewriter {
                    context: unit.context,
                    declared: unit.declared,
                    function: unit.function,
                    se,
                    env,
                    callees: outer.callees(),
                };
                let mut exp = Expander::new("storeforward");
                for cand in &candidates {
                    propagate_stored_value_to_load_users(&mut rw, &mut exp, header, *cand);
                }
            }
            changed = true;
            // The load is dead, the phis are new: the expressions are stale.
            session = ScevSession::of(unit.context, unit.function, &outer);
            stale = false;
        }
        if changed {
            PreservedAnalyses::none()
        } else {
            held.put(session.into_scev());
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
#[path = "looploadelim_tests.rs"]
mod tests;
