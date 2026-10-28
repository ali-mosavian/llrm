//! `runOnLoop` and what it calls to find the stores and mem intrinsics worth
//! rewriting: the countable half of `LoopIdiomRecognize`.

use llrm_analysis::mustexecute::SimpleLoopSafetyInfo;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expr::Scev;
use llrm_analysis::scev::expr::Ty;
use llrm_mir::context::ConstantKind;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::valuetracking::bytewise::{Byte, is_bytewise_value};
use llrm_mir::valuetracking::pattern::{IntrinsicId, call_intrinsic};

use super::process::{Run, StoreOfLoad, Strided, argument_align, cx, pointer_space};
use super::{LoopIdiomRecognize, equals, get_mem_set_pattern_value, get_store_stride};

/// `ForMemset`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForMemset {
    No,
    Yes,
}

/// What `processLoopMemIntrinsic` looks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MemKind {
    Cpy,
    Set,
}

/// `isVolatile` of a mem intrinsic call: its fourth argument.
fn is_volatile(
    run: &Run,
    call: InstId,
) -> bool {
    match run.function.instruction(call).operands.get(3) {
        Some(Operand::Constant(id)) => !matches!(
            run.context.get(*id).kind,
            ConstantKind::Int(0) | ConstantKind::Zero
        ),
        _ => true,
    }
}

/// `dyn_cast<ConstantInt>(Value)`: its zero-extended value.
fn constant_int(
    run: &Run,
    value: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = value else { return None };
    match run.context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        ConstantKind::Zero => Some(0),
        _ => None,
    }
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L349
    /// `runOnLoop`. `None` where the loop is not countable: the C++ then runs
    /// `runOnNoncountableLoop` (stage L4 of the plan), which is not translated
    /// yet.
    pub fn run_on_loop(
        &mut self,
        run: &mut Run,
    ) -> Option<bool> {
        // If the loop could not be converted to canonical form, it must have
        // an indirectbr in it, just give up.
        if run.cur_loop.get_loop_preheader(run.function).is_none() {
            return Some(false);
        }

        // Disable loop idiom recognition if the function's name is a common
        // idiom.
        // The name is the symbol's: a C compile decorates it (`_strlen`, Open
        // Watcom's `strlen_`), which the pin's `getName` never sees.
        let undecorated = run.name.strip_prefix('_').or_else(|| run.name.strip_suffix('_')).unwrap_or(run.name);
        if matches!(run.name, "memset" | "memcpy" | "strlen" | "wcslen")
            || matches!(undecorated, "memset" | "memcpy" | "strlen" | "wcslen")
        {
            return Some(false);
        }

        // Determine if code size heuristics need to be applied.
        self.apply_code_size_heuristics =
            run.function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "optsize"))
                && self.options.use_lir_code_size_heurs;

        self.no_builtins =
            run.function.attrs.iter().any(|attr| matches!(attr, Attribute::Str(key, _) if key == "no-builtins"));
        self.has_memset = self.libcall(self.library.memset);
        // TODO (LLVM): Unconditionally enable use of the memset pattern
        // intrinsic (or at least, opt-in via target hook) once we are
        // confident it will never result in worse codegen than without. For
        // now, use it only when the target supports memset_pattern16 libcall
        // (or unless this is overridden by command line option).
        self.has_memset_pattern = self.libcall(self.library.memset_pattern16);
        self.has_memcpy = self.libcall(self.library.memcpy);

        if self.has_memset
            || self.has_memset_pattern
            || self.options.force_memset_pattern_intrinsic
            || self.has_memcpy
            || !self.options.disable_hash_recognize
        {
            let header = run.cur_loop.get_header();
            let cx = cx!(run);
            if run.se.has_loop_invariant_backedge_taken_count(&cx, header) {
                return Some(self.run_on_countable_loop(run));
            }
        }

        Some(self.run_on_noncountable_loop(run))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L383
    /// `runOnCountableLoop`.
    pub fn run_on_countable_loop(
        &mut self,
        run: &mut Run,
    ) -> bool {
        let header = run.cur_loop.get_header();
        let (be_count, exit_blocks, may_throw) = {
            let cx = cx!(run);
            let be_count = run.se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
            (
                be_count,
                run.cur_loop.get_unique_exit_blocks(run.function),
                // The following transforms hoist stores/memsets into the loop
                // pre-header. Give up if the loop has instructions that may
                // throw.
                SimpleLoopSafetyInfo::new(&cx, run.cur_loop).any_block_may_throw(),
            )
        };

        // If this loop executes exactly one time, then it should be peeled,
        // not optimized by this pass.
        if run.se.match_zero(be_count) {
            return false;
        }

        if may_throw {
            return false;
        }

        let mut made_change = false;

        // Scan all the blocks in the loop that are not in subloops.
        for &block in run.cur_loop.get_blocks() {
            // Ignore blocks in subloops.
            if run.env.loops.get_loop_for(block) != Some(run.cur_loop) {
                continue;
            }

            made_change |= self.run_on_loop_block(run, block, be_count, &exit_blocks);
        }

        // Attempt to optimize a CRC loop if one is detected by HashRecognize.
        if !self.options.disable_hash_recognize
            && self.options.crc_strategy != super::CrcStrategy::Disable
            && let Some(tti) = run.tti
        {
            let found = {
                let cx = cx!(run);
                super::hashrecognize::recognize_crc(&cx, run.se, run.cur_loop)
            };
            if let Ok(info) = found {
                made_change |= self.optimize_crc_loop(run, &info, tti);
            }
        }

        made_change
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L629
    /// `runOnLoopBlock`: process the block, which lives in a counted loop with
    /// the given backedge count. This block is known to be in the current loop
    /// and not in any subloops.
    pub fn run_on_loop_block(
        &mut self,
        run: &mut Run,
        block: BlockId,
        be_count: Scev,
        exit_blocks: &[BlockId],
    ) -> bool {
        // We can only promote stores in this block if they are unconditionally
        // executed in the loop. For a block to be unconditionally executed, it
        // has to dominate all the exit blocks of the loop. Verify this now.
        for &exit_block in exit_blocks {
            if !run.env.tree.dominates(block, exit_block) {
                return false;
            }
        }

        let mut made_change = false;
        // Look for store instructions, which may be optimized to memset/memcpy.
        {
            let cx = cx!(run);
            self.collect_stores(&cx, run.se, run.cur_loop, block);
        }

        // Look for a single store or sets of stores with a common base, which
        // can be optimized into a memset (memset_pattern). The latter most
        // commonly happens with structs and handunrolled loops.
        let for_memset = self.store_refs_for_memset.clone();
        for (_, list) in &for_memset {
            made_change |= self.process_loop_stores(run, list, be_count, ForMemset::Yes);
        }

        let for_pattern = self.store_refs_for_memset_pattern.clone();
        for (_, list) in &for_pattern {
            made_change |= self.process_loop_stores(run, list, be_count, ForMemset::No);
        }

        // Optimize the store into a memcpy, if it feeds an similarly strided
        // load.
        let for_memcpy = self.store_refs_for_memcpy.clone();
        for &store in &for_memcpy {
            made_change |= self.process_loop_store_of_loop_load_store(run, store, be_count);
        }

        made_change |= self.process_loop_mem_intrinsic(run, block, be_count, MemKind::Cpy);
        made_change |= self.process_loop_mem_intrinsic(run, block, be_count, MemKind::Set);

        made_change
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L665
    /// `processLoopStores`: see if this store(s) can be promoted to a memset.
    pub fn process_loop_stores(
        &mut self,
        run: &mut Run,
        sl: &[InstId],
        be_count: Scev,
        for_memset: ForMemset,
    ) -> bool {
        // Try to find consecutive stores that can be transformed into memsets.
        let mut heads: Vec<InstId> = Vec::new();
        let mut tails: Vec<InstId> = Vec::new();
        let mut consecutive_chain = llrm_mir::dense::IdMap::<InstId, InstId>::new();

        let layout = run.env.layout;
        let store_size_of = |run: &Run, store: InstId| {
            let ty =
                run.function.operand_type(run.context, run.function.instruction(store).operands[0]).expect("typed");
            layout.store_size(&run.context.types, ty)
        };
        let value_of = |run: &Run, store: InstId| run.function.instruction(store).operands[0];
        let pointer_of = |run: &Run, store: InstId| run.function.instruction(store).operands[1];

        /// What the first or second store writes: its splat or its pattern.
        #[derive(Clone, Copy, PartialEq)]
        enum Written {
            Splat(Option<Byte>),
            Pattern(Option<llrm_mir::context::ConstantId>),
        }
        let written = |run: &Run, store: InstId| -> Written {
            let cx = cx!(run);
            match for_memset {
                ForMemset::Yes => Written::Splat(is_bytewise_value(&cx.query(Some(store)), value_of(run, store))),
                ForMemset::No => Written::Pattern(get_mem_set_pattern_value(&cx, value_of(run, store))),
            }
        };
        let is_undef = |run: &Run, written: Written| match written {
            Written::Splat(byte) => matches!(byte, Some(Byte::Undef | Byte::Poison)),
            Written::Pattern(Some(id)) => {
                matches!(
                    run.context.get(id).kind,
                    ConstantKind::Poison | ConstantKind::Undef
                )
            }
            Written::Pattern(None) => false,
        };

        // Do a quadratic search on all of the given stores and find all of the
        // pairs of stores that follow each other.
        for i in 0..sl.len() {
            let first_store_ptr = pointer_of(run, sl[i]);
            let first_store_ev = {
                let cx = cx!(run);
                run.se.get_scev(&cx, first_store_ptr)
            };
            let first_stride = get_store_stride(run.se, first_store_ev);
            let first_store_size = store_size_of(run, sl[i]);

            // See if we can optimize just this store in isolation.
            if equals(first_stride, first_store_size) || equals(first_stride.neg(), first_store_size) {
                if !heads.contains(&sl[i]) {
                    heads.push(sl[i]);
                }
                continue;
            }

            let mut first_written = written(run, sl[i]);

            // If a store has multiple consecutive store candidates, search
            // Stores array according to the sequence: from i+1 to e, then from
            // i-1 to 0. This is because usually pairing with immediate
            // succeeding or preceding candidate create the best chance to find
            // memset opportunity.
            let index_queue: Vec<usize> = (i + 1..sl.len()).chain((0..i).rev()).collect();

            for k in index_queue {
                let second_store_ptr = pointer_of(run, sl[k]);
                let second_store_ev = {
                    let cx = cx!(run);
                    run.se.get_scev(&cx, second_store_ptr)
                };
                let second_stride = get_store_stride(run.se, second_store_ev);

                if first_stride != second_stride {
                    continue;
                }

                let second_written = written(run, sl[k]);

                let consecutive = {
                    let cx = cx!(run);
                    llrm_analysis::loopaccess::is_consecutive_access(&cx, run.se, sl[i], sl[k], false)
                };
                if consecutive {
                    if is_undef(run, first_written) {
                        first_written = second_written;
                    }
                    if first_written != second_written {
                        continue;
                    }
                    if !tails.contains(&sl[k]) {
                        tails.push(sl[k]);
                    }
                    if !heads.contains(&sl[i]) {
                        heads.push(sl[i]);
                    }
                    consecutive_chain.insert(sl[i], sl[k]);
                    break;
                }
            }
        }

        // We may run into multiple chains that merge into a single chain. We
        // mark the stores that we transformed so that we don't visit the same
        // store twice.
        let mut transformed_stores: Vec<InstId> = Vec::new();
        let mut changed = false;

        // For stores that start but don't end a link in the chain:
        for &head in &heads.clone() {
            if tails.contains(&head) {
                continue;
            }

            // We found a store instr that starts a chain. Now follow the chain
            // and try to transform it.
            let mut adjacent_stores: Vec<InstId> = Vec::new();
            let head_store = head;
            let mut store_size = 0u64;

            // Collect the chain into a list.
            let mut current = head;
            while tails.contains(&current) || heads.contains(&current) {
                if transformed_stores.contains(&current) {
                    break;
                }
                if !adjacent_stores.contains(&current) {
                    adjacent_stores.push(current);
                }

                store_size += store_size_of(run, current);
                // Move to the next value in the chain.
                match consecutive_chain.get(&current) {
                    Some(&next) => current = next,
                    None => break,
                }
            }

            let stored_val = value_of(run, head_store);
            let store_ptr = pointer_of(run, head_store);
            let store_ev = {
                let cx = cx!(run);
                run.se.get_scev(&cx, store_ptr)
            };
            let stride = get_store_stride(run.se, store_ev);

            // Check to see if the stride matches the size of the stores. If so,
            // then we know that every byte is touched in the loop.
            if !(equals(stride, store_size) || equals(stride.neg(), store_size)) {
                continue;
            }

            let is_neg_stride = equals(stride.neg(), store_size);

            let space = pointer_space(run.context, run.function, store_ptr);
            let int_idx_ty = Ty::Int(layout.pointer(space).index_bits);
            let store_size_scev = {
                let cx = cx!(run);
                run.se.get_constant_of_type(&cx, int_idx_ty, store_size as i128)
            };
            let Opcode::Store { align, .. } = run.function.instruction(head_store).opcode else { unreachable!() };
            // `getAlign()` is the type's ABI alignment where none is written.
            let head_ty = run.function.operand_type(run.context, stored_val).expect("typed");
            let store_alignment = Some(align.unwrap_or_else(|| layout.align(&run.context.types, head_ty)));
            if self.process_loop_strided_store(
                run,
                &Strided {
                    dest_ptr: store_ptr,
                    store_size_scev,
                    store_alignment,
                    stored_val,
                    the_store: head_store,
                    stores: adjacent_stores.clone(),
                    ev: store_ev,
                    be_count,
                    is_neg_stride,
                    is_loop_memset: false,
                },
            ) {
                transformed_stores.extend(adjacent_stores);
                changed = true;
            }
        }

        changed
    }

    /// `processLoopMemIntrinsic`: calls the processor for each mem intrinsic
    /// of the block, starting over from the top where processing erased the
    /// instruction that came next.
    fn process_loop_mem_intrinsic(
        &mut self,
        run: &mut Run,
        block: BlockId,
        be_count: Scev,
        kind: MemKind,
    ) -> bool {
        let mut made_change = false;
        let mut insts: Vec<InstId> = run.function.iter_block(block).collect::<Vec<_>>();
        let mut at = 0;
        while at < insts.len() {
            let inst = insts[at];
            at += 1;
            let is_kind = {
                let cx = cx!(run);
                let query = cx.query(Some(inst));
                match kind {
                    MemKind::Cpy => {
                        matches!(
                            call_intrinsic(&query, inst),
                            Some(IntrinsicId::Memcpy | IntrinsicId::MemcpyInline)
                        )
                    }
                    MemKind::Set => call_intrinsic(&query, inst) == Some(IntrinsicId::Memset),
                }
            };
            // Look for memory instructions, which may be optimized to a
            // larger one.
            if !is_kind {
                continue;
            }
            let next = insts.get(at).copied();
            let done = match kind {
                MemKind::Cpy => self.process_loop_mem_cpy(run, inst, be_count),
                MemKind::Set => self.process_loop_mem_set(run, inst, be_count),
            };
            if !done {
                continue;
            }
            made_change = true;

            // If processing the instruction invalidated our iterator, start
            // over from the top of the block.
            if next.is_some_and(|next| run.function.is_erased(next)) {
                insts = run.function.iter_block(block).collect::<Vec<_>>();
                at = 0;
            }
        }
        made_change
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L834
    /// `processLoopMemCpy`: see if this memcpy can be promoted to a large
    /// memcpy.
    fn process_loop_mem_cpy(
        &mut self,
        run: &mut Run,
        mci: InstId,
        be_count: Scev,
    ) -> bool {
        let (dest, source, length) = {
            let operands = &run.function.instruction(mci).operands;
            (operands[0], operands[1], operands[2])
        };
        // We can only handle non-volatile memcpys with a constant size.
        let Some(size_in_bytes) = constant_int(run, length) else { return false };
        if is_volatile(run, mci) {
            return false;
        }

        // If we're not allowed to hack on memcpy, we fail.
        let force_inlined = call_intrinsic(&cx!(run).query(Some(mci)), mci) == Some(IntrinsicId::MemcpyInline);
        if (!self.has_memcpy && !force_inlined) || self.options.disable_memcpy {
            return false;
        }

        // See if the load and store pointer expressions are AddRec like
        // {base,+,1} on the current loop, which indicates a strided load and
        // store. If we have something else, it's a random load or store we
        // can't handle.
        let (store_ev, load_ev) = {
            let cx = cx!(run);
            (run.se.get_scev(&cx, dest), run.se.get_scev(&cx, source))
        };
        let header = run.cur_loop.get_header();
        let strided = |se: &llrm_analysis::scev::ScalarEvolution, ev: Scev| {
            let (_, step, loop_header) = se.match_affine_add_rec(ev)?;
            if loop_header != header {
                return None;
            }
            se.match_apint(step)
        };
        let (Some(store_stride), Some(load_stride)) = (strided(run.se, store_ev), strided(run.se, load_ev)) else {
            return false;
        };

        // Reject memcpys that are so large that they overflow an unsigned.
        if (size_in_bytes >> 32) != 0 {
            return false;
        }

        // Huge stride value - give up.
        if store_stride.width > 64 || load_stride.width > 64 {
            return false;
        }

        if !(equals(store_stride, size_in_bytes as u64) || equals(store_stride.neg(), size_in_bytes as u64)) {
            return false;
        }

        // Check if the load stride matches the store stride.
        if store_stride.sext_value() as i64 != load_stride.sext_value() as i64 {
            return false;
        }

        let space = pointer_space(run.context, run.function, dest);
        let store_size_scev = {
            let cx = cx!(run);
            run.se.get_constant_of_type(&cx, Ty::Ptr(space), size_in_bytes as i128)
        };
        self.process_loop_store_of_loop_load(
            run,
            &StoreOfLoad {
                dest_ptr: dest,
                source_ptr: source,
                store_size_scev,
                store_align: argument_align(run.function, mci, 0),
                load_align: argument_align(run.function, mci, 1),
                the_store: mci,
                the_load: mci,
                store_ev,
                load_ev,
                be_count,
            },
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L897
    /// `processLoopMemSet`: see if this memset can be promoted to a large
    /// memset.
    fn process_loop_mem_set(
        &mut self,
        run: &mut Run,
        msi: InstId,
        be_count: Scev,
    ) -> bool {
        // We can only handle non-volatile memsets.
        if is_volatile(run, msi) {
            return false;
        }

        // If we're not allowed to hack on memset, we fail.
        if !self.has_memset || self.options.disable_memset {
            return false;
        }

        let (pointer, splat_value, length) = {
            let operands = &run.function.instruction(msi).operands;
            (operands[0], operands[1], operands[2])
        };

        // See if the pointer expression is an AddRec like {base,+,1} on the
        // current loop, which indicates a strided store. If we have something
        // else, it's a random store we can't handle.
        let ev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, pointer)
        };
        let header = run.cur_loop.get_header();
        let Some((_, pointer_stride_scev, loop_header)) = run.se.match_affine_add_rec(ev) else { return false };
        if loop_header != header {
            // Pointer is not affine, abort.
            return false;
        }

        let memset_size_scev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, length)
        };

        let is_neg_stride;
        let size_constant = constant_int(run, length);

        if let Some(size_in_bytes) = size_constant {
            // Memset size is constant. Check if the pointer stride matches the
            // memset size. If so, then we know that every byte is touched in
            // the loop.
            let Some(stride) = run.se.match_apint(pointer_stride_scev) else { return false };
            if !(equals(stride, size_in_bytes as u64) || equals(stride.neg(), size_in_bytes as u64)) {
                return false;
            }
            is_neg_stride = equals(stride.neg(), size_in_bytes as u64);
        } else {
            // Memset size is non-constant. Check if the pointer stride matches
            // the memset size. To be conservative, the pass would not promote
            // pointers that aren't in address space zero. Also, the pass only
            // handles memset length and stride that are invariant for the top
            // level loop.
            if pointer_space(run.context, run.function, pointer) != 0 {
                return false;
            }
            let invariant = {
                let cx = cx!(run);
                run.se.is_loop_invariant(&cx, memset_size_scev, Some(header))
            };
            if !invariant {
                return false;
            }

            // Compare positive direction PointerStrideSCEV with
            // MemsetSizeSCEV.
            is_neg_stride = run.se.is_non_constant_negative(pointer_stride_scev);
            let positive_stride_scev = if is_neg_stride {
                let cx = cx!(run);
                run.se.get_negative_scev(&cx, pointer_stride_scev, 0)
            } else {
                pointer_stride_scev
            };

            if positive_stride_scev != memset_size_scev {
                // If an expression is covered by the loop guard, compare again
                // and proceed with optimization if equal.
                let cx = cx!(run);
                let folded_positive_stride = run.se.apply_loop_guards(&cx, positive_stride_scev, header);
                let folded_memset_size = run.se.apply_loop_guards(&cx, memset_size_scev, header);
                if folded_positive_stride != folded_memset_size {
                    return false;
                }
            }
        }

        // Verify that the memset value is loop invariant. If not, we can't
        // promote the memset.
        if !run.cur_loop.is_loop_invariant(run.function, splat_value) {
            return false;
        }

        let store_size_scev = {
            let cx = cx!(run);
            run.se.get_scev(&cx, length)
        };
        self.process_loop_strided_store(
            run,
            &Strided {
                dest_ptr: pointer,
                store_size_scev,
                store_alignment: argument_align(run.function, msi, 0),
                stored_val: splat_value,
                the_store: msi,
                stores: vec![msi],
                ev,
                be_count,
                is_neg_stride,
                is_loop_memset: true,
            },
        )
    }
}
