//! The rewrites of `LoopIdiomRecognize`: a strided store becomes a memset or
//! memset.pattern in the preheader, a store of a strided load a memcpy or
//! memmove.
//!
//! # How it works
//! [`Run`] holds what the C++ class holds by reference (ScalarEvolution, alias
//! analysis, the loop, the data layout) and the function by `&mut`. The SCEV
//! queries are made through a [`Cx`] built over the function as it stands for
//! that query, never held across an edit.
//!
//! # Deviations
//! - D10 (temporary, [#2314](https://github.com/ali-mosavian/llrm/issues/2314))
//!   No `AAMDNodes` on the new call: `merge` and `extendTo` of TBAA build new
//!   metadata nodes (`TypeBasedAliasAnalysis.cpp`, not translated). MIR carries
//!   `!tbaa`, and an untagged call is may-alias to TBAA, so this errs
//!   conservative. Exit: `AAMDNodes` in MIR.
//! - No `MemorySSAUpdater`, `OptimizationRemarkEmitter` or debug location: the
//!   pass keeps no MemorySSA, emits no remarks, and MIR keeps the debug
//!   locations of statements apart from instructions.
//! - The callee of a new call is declared through [`Declared`]
//!   (`getOrInsertDeclaration`), which the pass manager adds to the module when
//!   the pass ends.

use llrm_analysis::aa::{AAResults, ModRefInfo};
use llrm_analysis::scev::expander::{Env, Expander, ExpanderCleaner};
use llrm_analysis::scev::expr::{Scev, Ty};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::bytewise::{Byte, is_bytewise_value};
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;

use super::build::{self, Copy};
use super::{LoopIdiomRecognize, equals, get_mem_set_pattern_value, get_num_bytes, get_start_for_neg_stride};
use crate::profit::OperationCosts;

/// Alias analysis over a function as it stands. It is made for one version of
/// a body and the pass edits the body between queries, so the pass asks for
/// one over the current function each time.
pub trait AaProvider {
    fn with(
        &self,
        context: &Context,
        function: &Function,
        query: &mut dyn FnMut(&mut dyn AAResults) -> bool,
    ) -> bool;
}

/// What `LoopIdiomRecognize` holds by reference besides its own state.
pub struct Run<'a> {
    /// The function's name (`getName`).
    pub name: &'a str,
    pub env: &'a Env<'a>,
    pub context: &'a mut Context,
    pub function: &'a mut Function,
    pub se: &'a mut ScalarEvolution,
    pub aa: &'a dyn AaProvider,
    pub declared: &'a mut Declared,
    pub cur_loop: Loop<'a>,
    /// The target's code-byte prices and the callees, at `-Os`.
    pub bytes: Option<(&'a OperationCosts, &'a llrm_mir::memory::Callees)>,
    /// The target's clock prices and what a byte of code is worth in them
    /// (thousandths of a clock), at `-Os`; none, bytes alone decide.
    pub clocks: Option<(&'a OperationCosts, i64)>,
    /// `TTI`: the target's costs, where it has them.
    pub tti: Option<&'a dyn llrm_mir::tti::Tti>,
}

/// The arguments of `processLoopStridedStore`.
pub struct Strided {
    pub dest_ptr: Operand,
    pub store_size_scev: Scev,
    pub store_alignment: Option<u64>,
    pub stored_val: Operand,
    pub the_store: InstId,
    pub stores: Vec<InstId>,
    pub ev: Scev,
    pub be_count: Scev,
    pub is_neg_stride: bool,
    pub is_loop_memset: bool,
}

/// The arguments of the general `processLoopStoreOfLoopLoad`.
pub struct StoreOfLoad {
    pub dest_ptr: Operand,
    pub source_ptr: Operand,
    pub store_size_scev: Scev,
    pub store_align: Option<u64>,
    pub load_align: Option<u64>,
    pub the_store: InstId,
    pub the_load: InstId,
    pub store_ev: Scev,
    pub load_ev: Scev,
    pub be_count: Scev,
}

macro_rules! cx {
    ($run:expr) => {
        $run.env.cx(&*$run.context, &*$run.function)
    };
}
pub(super) use cx;

pub(super) fn pointer_space(
    context: &Context,
    function: &Function,
    pointer: Operand,
) -> u32 {
    let ty = function.operand_type(context, pointer).expect("a typed pointer");
    match context.types.get(ty) {
        Type::Pointer(space) => *space,
        other => panic!("a pointer, not {other:?}"),
    }
}

/// `getDestAlign` and `getSourceAlign`: the `align` of a call's argument.
pub(super) fn argument_align(
    function: &Function,
    call: InstId,
    argument: usize,
) -> Option<u64> {
    let Opcode::Call(info) = &function.instruction(call).opcode else { return None };
    info.argument_attrs
        .get(argument)?
        .iter()
        .find_map(
            |attr| match attr {
                Attribute::Int(name, value) if name == "align" => Some(*value),
                _ => None,
            },
        )
}

/// `deleteDeadInstruction`: the instructions deleted here are a store and a
/// call to a void intrinsic, which have no result to replace with poison.
fn delete_dead_instruction(
    function: &mut Function,
    inst: InstId,
) {
    debug_assert!(function.instruction(inst).result.is_none(), "a store or a void call");
    function.erase(inst).expect("a dead instruction");
}

/// The `i8` constant a `Byte` stands for, as an operand.
fn byte_operand(
    context: &Context,
    byte: Byte,
) -> Operand {
    let ty = context.types.int(8);
    match byte {
        Byte::Value(value) => value,
        Byte::Constant(value) => Operand::Constant(context.int(ty, i128::from(value))),
        Byte::Undef => Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Undef })),
        Byte::Poison => Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison })),
    }
}

/// `Builder.CreateMul(TripCount, getIntN(Width, Reps))`, folded as the
/// `IRBuilder`'s constant folder does.
fn create_mul(
    context: &Context,
    function: &mut Function,
    before: InstId,
    value: Operand,
    reps: u64,
) -> Operand {
    let ty = function.operand_type(context, value).expect("a typed count");
    let width = context.types.int_bits(ty).expect("an integer count");
    let factor = context.int(ty, i128::from(reps));
    if let Operand::Constant(id) = value
        && let ConstantKind::Int(bits) = context.get(id).kind
    {
        let product = bits.wrapping_mul(u128::from(reps)) & llrm_mir::context::mask(width);
        return Operand::Constant(context.int(ty, product as i128));
    }
    let mul = function.create_instruction(
        Opcode::Binary(BinaryOp::Mul),
        ty,
        vec![value, Operand::Constant(factor)],
        Flags::default(),
        None,
    );
    function.insert(mul, llrm_mir::edit::Position::Before(before)).expect("a placed multiply");
    Operand::Value(function.instruction(mul).result.expect("a multiply has a result"))
}

/// `mayLoopAccessLocation` over the function as it stands.
fn may_access(
    run: &mut Run,
    ptr: Operand,
    access: ModRefInfo,
    be_count: Scev,
    store_size_scev: Scev,
    ignored: &[InstId],
    splat: Option<Byte>,
) -> bool {
    let cx = cx!(run);
    let (se, cur_loop) = (&mut *run.se, run.cur_loop);
    run.aa.with(&*run.context, &*run.function, &mut |aa| {
        super::may_loop_access_location(&cx, se, aa, ptr, access, cur_loop, be_count, store_size_scev, ignored, splat)
    })
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1582
    /// `avoidLIRForMultiBlockLoop`: when compiling for codesize we avoid idiom
    /// recognition for a multi-block loop unless it is a loop_memset idiom or
    /// a memset/memcpy idiom in a nested loop.
    pub fn avoid_lir_for_multi_block_loop(
        &self,
        cur_loop: Loop,
        is_memset: bool,
        is_loop_memset: bool,
    ) -> bool {
        self.apply_code_size_heuristics
            && cur_loop.get_num_blocks() > 1
            && cur_loop.is_outermost()
            && (!is_memset || !is_loop_memset)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1094
    /// `processLoopStridedStore`: a strided store of some value becomes a
    /// memset or memset_pattern in the loop preheader, if we can.
    pub fn process_loop_strided_store(
        &mut self,
        run: &mut Run,
        args: &Strided,
    ) -> bool {
        // DEVIATE: a huge pointer's step carries into its selector, which no
        // `rep stos`/`movs` does past 64K.
        if run.env.layout.carries(pointer_space(run.context, run.function, args.dest_ptr)) {
            return false;
        }
        // DEVIATE: at -Os a loop of several stores stays one when a fill for
        // each store is more bytes than the loop (lru.c: four stores a trip
        // over three trips, five fills 33 bytes over it). Exit: none.
        if let Some((costs, callees)) = run.bytes {
            let blocks = run.cur_loop.get_blocks();
            let cx = cx!(run);
            let mut groups: Vec<(Operand, Operand)> = Vec::new();
            let mut stores = 0;
            let mut kept = 0;
            for &block in blocks {
                for inst in run.function.iter_block(block) {
                    let instruction = run.function.instruction(inst);
                    if !matches!(instruction.opcode, Opcode::Store { .. }) {
                        continue;
                    }
                    stores += 1;
                    // A store of a value the loop computes stays, and with it
                    // the loop: a fill then saves that one store.
                    if let Operand::Value(value) = instruction.operands[0]
                        && matches!(
                            run.function.value(value).def,
                            llrm_mir::module::ValueDef::Instruction(def) if run.function.parent(def).is_some_and(|home| blocks.contains(&home))
                        )
                    {
                        kept += 1;
                    }
                    let object = llrm_mir::valuetracking::underlying::get_underlying_object(
                        &cx.query(Some(inst)),
                        instruction.operands[1],
                        0,
                    );
                    let group = (object, instruction.operands[0]);
                    if !groups.contains(&group) {
                        groups.push(group);
                    }
                }
            }
            // The fill of an object a memset sets already merges with it
            // (`vel_x[i] = 0` after `let vel_x = [0, ...]`): the loop's zero
            // stores are then dead, whatever else stays.
            if kept > 0 && self.fills_a_set_object(run, &cx, &blocks) {
                kept = 0;
            }
            if !crate::loopidioms::fills_pay(
                run.context,
                run.env.layout,
                run.function,
                callees,
                &blocks,
                costs,
                stores,
                groups.len() as i64,
                kept,
                run.clocks.map(|(clocks, rate)| {
                    let (be, size) = (run.se.match_apint(args.be_count), run.se.match_apint(args.store_size_scev));
                    let trips = be.filter(|be| be.active_bits() <= 32).map(|be| be.zext_value() as i64 + 1);
                    let width = size.filter(|size| size.active_bits() <= 32).map(|size| size.zext_value() as i64);
                    (clocks, rate, trips, width.map(|width| width * stores))
                }),
            ) {
                return false;
            }
        }
        // The trip count of the loop and the base pointer of the addrec SCEV
        // is guaranteed to be loop invariant, which means that it should
        // dominate the header. This allows us to insert code for it in the
        // preheader.
        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let insert_at = run.function.terminator(preheader).expect("a preheader ends");
        let mut expander = Expander::new("loop-idiom");
        expander.set_insert_point(insert_at);
        let mut cleaner = ExpanderCleaner::new(&expander);
        let (changed, used) = self.strided_store(run, &mut expander, insert_at, args);
        if used {
            cleaner.mark_result_used();
        }
        cleaner.cleanup(&mut expander, run.function);
        changed
    }

    /// The body of `processLoopStridedStore`: whether it changed the IR, and
    /// whether the expanded code is used.
    fn strided_store(
        &mut self,
        run: &mut Run,
        expander: &mut Expander,
        insert_at: InstId,
        args: &Strided,
    ) -> (bool, bool) {
        let dest_as = pointer_space(run.context, run.function, args.dest_ptr);
        let int_idx_ty = Ty::Int(run.env.layout.pointer(dest_as).index_bits);

        let (start, _, _) = run.se.match_affine_add_rec(args.ev).expect("an add recurrence");
        let mut start = start;
        // Handle negative strided loops.
        if args.is_neg_stride {
            let cx = cx!(run);
            start = get_start_for_neg_stride(&cx, run.se, start, args.be_count, int_idx_ty, args.store_size_scev);
        }

        // TODO (LLVM): ideally we should still be able to generate memset if
        // SCEV expander is taught to generate the dependencies at the latest
        // point.
        let safe = {
            let cx = cx!(run);
            expander.is_safe_to_expand(&cx, run.se, start)
        };
        if !safe {
            return (false, false);
        }

        // Okay, we have a strided store "p[i]" of a splattable value. We can
        // turn this into a memset in the loop preheader now if we want.
        // However, this would be unsafe to do if there is anything else in
        // the loop that may read or write to the aliased location. Check for
        // any overlap by generating the base pointer and checking the region.
        let base_ptr = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            start,
            Some(Ty::Ptr(dest_as)),
        );

        // From here on out, conservatively report to the pass manager that
        // we've changed the IR, even if we later clean up these added
        // instructions.
        let changed = true;

        let splat_value = {
            let cx = cx!(run);
            is_bytewise_value(&cx.query(Some(args.the_store)), args.stored_val)
        };
        if may_access(run, base_ptr, ModRefInfo::ModRef, args.be_count, args.store_size_scev, &args.stores, splat_value)
        {
            return (changed, false);
        }

        if self.avoid_lir_for_multi_block_loop(run.cur_loop, true, args.is_loop_memset) {
            return (changed, false);
        }

        // Okay, everything looks good, insert the memset.
        let pattern_value: Option<ConstantId> = if splat_value.is_none() {
            let cx = cx!(run);
            get_mem_set_pattern_value(&cx, args.stored_val)
        } else {
            None
        };

        // MemsetArg is the number of bytes for the memset libcall, and the
        // number of pattern repetitions if the memset.pattern intrinsic is
        // being used.
        let memset_arg;
        let header = run.cur_loop.get_header();
        if pattern_value.is_some() && (self.has_memset_pattern || self.options.force_memset_pattern_intrinsic) {
            let trip_count_s = {
                let cx = cx!(run);
                run.se.get_trip_count_from_exit_count(&cx, args.be_count, int_idx_ty, Some(header))
            };
            let safe = {
                let cx = cx!(run);
                expander.is_safe_to_expand(&cx, run.se, trip_count_s)
            };
            if !safe {
                return (changed, false);
            }
            let Some(const_store_size) = run.se.match_apint(args.store_size_scev) else { return (changed, false) };
            let trip_count = expander.expand_code_for(
                run.env,
                run.context,
                run.declared,
                run.function,
                run.se,
                trip_count_s,
                Some(int_idx_ty),
            );
            let pattern_bits = {
                let ty = run.context.get(pattern_value.expect("a pattern")).ty;
                run.env.layout.size_bits(&run.context.types, ty)
            };
            let pattern_reps_per_trip = (const_store_size.zext_value() as u64 * 8) / pattern_bits;
            // If ConstStoreSize is not equal to the width of PatternValue,
            // then MemsetArg is TripCount * (ConstStoreSize/PatternValueWidth).
            // Else MemSetArg is just TripCount.
            memset_arg = if pattern_reps_per_trip == 1 {
                trip_count
            } else {
                create_mul(run.context, run.function, insert_at, trip_count, pattern_reps_per_trip)
            };
        } else {
            let num_bytes_s = {
                let cx = cx!(run);
                get_num_bytes(&cx, run.se, args.be_count, int_idx_ty, args.store_size_scev, run.cur_loop)
            };

            // TODO (LLVM): ideally we should still be able to generate memset
            // if SCEV expander is taught to generate the dependencies at the
            // latest point.
            let safe = {
                let cx = cx!(run);
                expander.is_safe_to_expand(&cx, run.se, num_bytes_s)
            };
            if !safe {
                return (changed, false);
            }
            memset_arg = expander.expand_code_for(
                run.env,
                run.context,
                run.declared,
                run.function,
                run.se,
                num_bytes_s,
                Some(int_idx_ty),
            );
        }

        if let Some(splat) = splat_value {
            let value = byte_operand(run.context, splat);
            build::create_mem_set(
                run.context,
                run.declared,
                run.function,
                insert_at,
                base_ptr,
                value,
                memset_arg,
                args.store_alignment,
            );
        } else if self.options.force_memset_pattern_intrinsic || self.library.memset_pattern16 {
            // isLibFuncEmittable(memset_pattern16)
            let pattern = Operand::Constant(pattern_value.expect("a pattern value"));
            build::create_mem_set_pattern(
                run.context,
                run.declared,
                run.function,
                insert_at,
                base_ptr,
                pattern,
                memset_arg,
                args.store_alignment,
            );
        } else {
            // Neither a memset, nor memset_pattern16
            return (changed, false);
        }

        // Okay, the memset has been formed. Zap the original store and
        // anything that feeds into it.
        for &store in &args.stores {
            delete_dead_instruction(run.function, store);
        }
        (true, true)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1350
    /// `processLoopStoreOfLoopLoad(StoreInst *SI, BECount)`: if the stored
    /// value is a strided load in the same loop with the same stride this may
    /// be transformable into a memcpy.
    pub fn process_loop_store_of_loop_load_store(
        &mut self,
        run: &mut Run,
        si: InstId,
        be_count: Scev,
    ) -> bool {
        let (store_ptr, stored, store_ty, store_align) = {
            let instruction = run.function.instruction(si);
            let Opcode::Store { align, .. } = instruction.opcode else { unreachable!("a store") };
            let ty = run.function.operand_type(run.context, instruction.operands[0]).expect("a typed value");
            (instruction.operands[1], instruction.operands[0], ty, align)
        };
        let store_ev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, store_ptr)
        };
        let store_size = run.env.layout.store_size(&run.context.types, store_ty);

        // The store must be feeding a non-volatile load.
        let (load, load_ptr, load_align) = {
            let cx = cx!(run);
            let query = cx.query(Some(si));
            let (load, instruction) = query.instruction_of(stored).expect("the stored value is a load");
            let Opcode::Load { align, .. } = instruction.opcode else { unreachable!("a load") };
            (load, instruction.operands[0], align)
        };

        // See if the pointer expression is an AddRec like {base,+,1} on the
        // current loop, which indicates a strided load.
        let load_ev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, load_ptr)
        };

        let store_as = pointer_space(run.context, run.function, store_ptr);
        let store_size_scev = {
            let cx = cx!(run);
            run.se.get_constant_of_type(&cx, Ty::Ptr(store_as), store_size as i128)
        };
        // `getAlign()` is the type's ABI alignment where none is written.
        let abi = |ty| run.env.layout.align(&run.context.types, ty);
        let store_align = store_align.unwrap_or_else(|| abi(store_ty));
        let load_align = load_align.unwrap_or_else(|| abi(run.function.instruction(load).ty));
        self.process_loop_store_of_loop_load(
            run,
            &StoreOfLoad {
                dest_ptr: store_ptr,
                source_ptr: load_ptr,
                store_size_scev,
                store_align: Some(store_align),
                load_align: Some(load_align),
                the_store: si,
                the_load: load,
                store_ev,
                load_ev,
                be_count,
            },
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1412
    /// `processLoopStoreOfLoopLoad(DestPtr, SourcePtr, ...)`.
    pub fn process_loop_store_of_loop_load(
        &mut self,
        run: &mut Run,
        args: &StoreOfLoad,
    ) -> bool {
        // DEVIATE: a huge pointer's step carries into its selector, which no
        // `rep stos`/`movs` does past 64K.
        if run.env.layout.carries(pointer_space(run.context, run.function, args.dest_ptr)) {
            return false;
        }
        // FIXME (LLVM): until llvm.memcpy.inline supports dynamic sizes, we
        // need to conservatively bail here, since otherwise we may have to
        // transform llvm.memcpy.inline into llvm.memcpy which is illegal.
        if self.is_memcpy_inline(run, args.the_store) {
            return false;
        }

        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let insert_at = run.function.terminator(preheader).expect("a preheader ends");
        let mut expander = Expander::new("loop-idiom");
        expander.set_insert_point(insert_at);
        let mut cleaner = ExpanderCleaner::new(&expander);
        let (changed, used) = self.store_of_loop_load(run, &mut expander, insert_at, args);
        if used {
            cleaner.mark_result_used();
        }
        cleaner.cleanup(&mut expander, run.function);
        changed
    }

    /// Whether every constant store of the loop over `blocks` is to an object
    /// a `memset`, `memcpy` (an initializer) or a constant store outside the
    /// loop sets.
    fn fills_a_set_object(
        &self,
        run: &Run,
        cx: &Cx,
        blocks: &[llrm_mir::module::BlockId],
    ) -> bool {
        use llrm_mir::valuetracking::pattern::{IntrinsicId, call_arguments, call_intrinsic};
        use llrm_mir::valuetracking::underlying::get_underlying_object;
        let mut set = Vec::new();
        for &block in run.function.layout() {
            for inst in run.function.iter_block(block) {
                let query = cx.query(Some(inst));
                let instruction = run.function.instruction(inst);
                if matches!(
                    call_intrinsic(&query, inst),
                    Some(IntrinsicId::Memset | IntrinsicId::Memcpy)
                ) {
                    set.push(get_underlying_object(&query, call_arguments(instruction)[0], 0));
                } else if matches!(instruction.opcode, Opcode::Store { .. })
                    && matches!(instruction.operands[0], Operand::Constant(_))
                    && !blocks.contains(&block)
                {
                    // An aggregate initializer, before it is a memset.
                    set.push(get_underlying_object(&query, instruction.operands[1], 0));
                }
            }
        }
        blocks
            .iter()
            .all(
                |&block| run.function
                    .iter_block(block)
                    .all(
                        |inst| {
                            let instruction = run.function.instruction(inst);
                            if !matches!(instruction.opcode, Opcode::Store { .. })
                                || !matches!(instruction.operands[0], Operand::Constant(_))
                            {
                                return true;
                            }
                            set.contains(&get_underlying_object(&cx.query(Some(inst)), instruction.operands[1], 0))
                        },
                    ),
            )
    }

    /// `isa<MemCpyInst>(TheStore) && MCI->isForceInlined()`.
    fn is_memcpy_inline(
        &self,
        run: &Run,
        inst: InstId,
    ) -> bool {
        let cx = cx!(run);
        let query = cx.query(Some(inst));
        llrm_mir::valuetracking::pattern::call_intrinsic(&query, inst)
            == Some(llrm_mir::valuetracking::pattern::IntrinsicId::MemcpyInline)
    }

    /// `isa<MemCpyInst>(I)`: a call to `llvm.memcpy` or `llvm.memcpy.inline`.
    pub(super) fn is_memcpy(
        &self,
        run: &Run,
        inst: InstId,
    ) -> bool {
        let cx = cx!(run);
        let query = cx.query(Some(inst));
        use llrm_mir::valuetracking::pattern::{IntrinsicId, call_intrinsic};
        matches!(
            call_intrinsic(&query, inst),
            Some(IntrinsicId::Memcpy | IntrinsicId::MemcpyInline)
        )
    }

    /// The body of `processLoopStoreOfLoopLoad`.
    fn store_of_loop_load(
        &mut self,
        run: &mut Run,
        expander: &mut Expander,
        insert_at: InstId,
        args: &StoreOfLoad,
    ) -> (bool, bool) {
        let str_as = pointer_space(run.context, run.function, args.dest_ptr);
        let int_idx_ty = Ty::Int(run.env.layout.pointer(str_as).index_bits);

        let (mut str_start, _, _) = run.se.match_affine_add_rec(args.store_ev).expect("an add recurrence");
        let stride = super::get_store_stride(run.se, args.store_ev);
        let const_store_size = run.se.match_apint(args.store_size_scev).expect("a constant store size");
        // TODO (LLVM): Deal with non-constant size; Currently expect constant
        // store size.
        let store_size = const_store_size.zext_value() as u64;
        let is_neg_stride = equals(stride.neg(), store_size);

        // Handle negative strided loops.
        if is_neg_stride {
            let cx = cx!(run);
            str_start =
                get_start_for_neg_stride(&cx, run.se, str_start, args.be_count, int_idx_ty, args.store_size_scev);
        }

        // Okay, we have a strided store "p[i]" of a loaded value. We can turn
        // this into a memcpy in the loop preheader now if we want. However,
        // this would be unsafe to do if there is anything else in the loop
        // that may read or write the memory region we're storing to. This
        // includes the load that feeds the stores. Check for an alias by
        // generating the base address and checking everything.
        // DEVIATE: at -Os a copy whose length is known only at run time stays
        // a loop when the `rep movs` that replaces it is more bytes (no libc
        // `memcpy` to call). Exit: none.
        if let Some((costs, callees)) = run.bytes
            && run.se.match_apint(args.be_count).is_none()
        {
            let blocks = run.cur_loop.get_blocks();
            if !crate::loopidioms::variable_copy_pays(
                run.context,
                run.env.layout,
                run.function,
                callees,
                &blocks,
                costs,
            ) {
                return (false, false);
            }
        }
        let store_base_ptr = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            str_start,
            Some(Ty::Ptr(str_as)),
        );

        // From here on out, conservatively report to the pass manager that
        // we've changed the IR, even if we later clean up these added
        // instructions.
        let changed = true;

        let mut ignored = vec![args.the_store];

        let is_mem_cpy = self.is_memcpy(run, args.the_store);

        let loop_access_store =
            may_access(run, store_base_ptr, ModRefInfo::ModRef, args.be_count, args.store_size_scev, &ignored, None);
        if loop_access_store {
            // For memmove case it's not enough to guarantee that loop doesn't
            // access TheStore and TheLoad. Additionally we need to make sure
            // that TheStore is the only user of TheLoad.
            let only_user = run
                .function
                .instruction(args.the_load)
                .result
                .is_some_and(|result| run.function.users(result).has_one_use());
            if !only_user {
                return (changed, false);
            }
            ignored.push(args.the_load);
            if may_access(run, store_base_ptr, ModRefInfo::ModRef, args.be_count, args.store_size_scev, &ignored, None)
            {
                return (changed, false);
            }
            ignored.retain(|&one| one != args.the_load);
        }

        let (mut ld_start, _, _) = run.se.match_affine_add_rec(args.load_ev).expect("an add recurrence");
        let ld_as = pointer_space(run.context, run.function, args.source_ptr);

        // Handle negative strided loops.
        if is_neg_stride {
            let cx = cx!(run);
            ld_start = get_start_for_neg_stride(&cx, run.se, ld_start, args.be_count, int_idx_ty, args.store_size_scev);
        }

        // For a memcpy, we have to make sure that the input array is not being
        // mutated by the loop.
        let load_base_ptr = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            ld_start,
            Some(Ty::Ptr(ld_as)),
        );

        // If the store is a memcpy instruction, we must check if it will write
        // to the load memory locations. So remove it from the ignored stores.
        let verifier = MemmoveVerifier::new(run.se, ld_start, str_start, &cx!(run));
        if is_mem_cpy && !verifier.is_same_object {
            ignored.retain(|&one| one != args.the_store);
        }
        if may_access(run, load_base_ptr, ModRefInfo::Mod, args.be_count, args.store_size_scev, &ignored, None) {
            return (changed, false);
        }

        let use_mem_move = if is_mem_cpy { verifier.is_same_object } else { loop_access_store };

        if use_mem_move {
            let cx = cx!(run);
            if !verifier.load_and_store_may_form_memmove(&cx, store_size, is_neg_stride, args.the_load, is_mem_cpy) {
                return (changed, false);
            }
        }

        if self.avoid_lir_for_multi_block_loop(run.cur_loop, false, false) {
            return (changed, false);
        }

        // Okay, everything is safe, we can transform this!
        let num_bytes_s = {
            let cx = cx!(run);
            get_num_bytes(&cx, run.se, args.be_count, int_idx_ty, args.store_size_scev, run.cur_loop)
        };

        let num_bytes = expander.expand_code_for(
            run.env,
            run.context,
            run.declared,
            run.function,
            run.se,
            num_bytes_s,
            Some(int_idx_ty),
        );

        let copy = build::create_mem_copy(
            run.context,
            run.declared,
            run.function,
            insert_at,
            if use_mem_move { Copy::Memmove } else { Copy::Memcpy },
            store_base_ptr,
            args.store_align,
            load_base_ptr,
            args.load_align,
            num_bytes,
        );
        // LD6 llrm's code generator lowers a `memmove` only with its direction
        // stated (`llrm.forward`, `llrm.backward`), and no libc `memmove` to
        // call: the starts are a constant apart, so the copy runs backward
        // where the destination is after the source.
        if use_mem_move && let Some(off) = verifier.off {
            let kind =
                if off.sext_value() > 0 { llrm_mir::intrinsics::BACKWARD } else { llrm_mir::intrinsics::FORWARD };
            let node = run.declared.node(llrm_mir::module::MetadataNode { distinct: false, operands: Vec::new() });
            run.function.annotate(copy, kind, node);
        }

        // Okay, a new call to memcpy/memmove has been formed. Zap the original
        // store and anything that feeds into it.
        delete_dead_instruction(run.function, args.the_store);
        (true, true)
    }
}

/// `MemmoveVerifier`.
struct MemmoveVerifier {
    off: Option<llrm_analysis::scev::ap::Ap>,
    base_ptr: Option<Operand>,
    is_same_object: bool,
}

impl MemmoveVerifier {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1304
    fn new(
        se: &mut ScalarEvolution,
        load_start: Scev,
        store_start: Scev,
        cx: &Cx,
    ) -> Self {
        let diff = se.get_minus_scev(
            cx,
            llrm_analysis::scev::expr::Use::of(store_start),
            llrm_analysis::scev::expr::Use::of(load_start),
            0,
            0,
        );
        let off = se.match_apint(diff);
        let base = se.get_pointer_base(store_start);
        let base_ptr = se.unknown_value(base);
        Self { off, base_ptr, is_same_object: off.is_some() }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1313
    fn load_and_store_may_form_memmove(
        &self,
        cx: &Cx,
        store_size: u64,
        is_neg_stride: bool,
        the_load: InstId,
        is_mem_cpy: bool,
    ) -> bool {
        // The store must be at a constant offset from the load, and there must
        // be an underlying pointer.
        let (Some(off), Some(base_ptr)) = (self.off, self.base_ptr) else { return false };
        let off_val = off.sext_value();
        // If null is defined then the base pointer can't be null.
        if let Operand::Constant(id) = base_ptr
            && matches!(cx.context.get(id).kind, ConstantKind::Null)
        {
            let space = match cx.context.types.get(cx.context.get(id).ty) {
                Type::Pointer(space) => *space,
                _ => 0,
            };
            if null_pointer_is_defined(Some(cx.function), space) {
                return false;
            }
        }
        let load_size: i128 = if is_mem_cpy {
            // memcpy is equivalent to a sequence of byte loads and stores.
            1
        } else {
            let ty = cx.function.instruction(the_load).ty;
            let size = (cx.layout.size_bits(&cx.context.types, ty) / 8) as i128;
            if size != store_size as i128 {
                return false;
            }
            size
        };
        // Ensure that LoadBasePtr is after StoreBasePtr or before StoreBasePtr
        // for negative stride. LoadBasePtr shouldn't overlap with
        // StoreBasePtr.
        !(if is_neg_stride { off_val < load_size } else { off_val > -load_size })
    }
}
