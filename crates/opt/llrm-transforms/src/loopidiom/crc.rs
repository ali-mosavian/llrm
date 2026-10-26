//! `optimizeCRCLoop` and `optimizeCRCLoopUsingTableLookup` of LLVM's
//! `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a bitwise CRC loop that
//! [`super::hashrecognize`] takes becomes a byte-at-a-time lookup in a
//! 256-entry table (the Sarwate algorithm).
//!
//! # Deviations
//! - D14: no carry-less multiply (`optimizeCRCLoopUsingClmul`): no target here
//!   has `llvm.clmul`, whose cost is then invalid and never the cheaper
//!   strategy.
//!
//! # Example
//! The eight trips of `c = (c & 1) ? 0xEDB88320 ^ (c >> 1) : c >> 1` become
//! one trip of `c = (c >> 8) ^ @crctable[c & 255]`.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::{ArithOp, CmpSel, CostKind, MemoryOp, OperandInfo, Ty};
use llrm_mir::types::Type;

use super::LoopIdiomRecognize;
use super::hashrecognize::{PolynomialInfo, gen_sarwate_table, recognize_crc};
use super::process::{Run, cx};

impl LoopIdiomRecognize {
    /// The CRC attempt of `runOnCountableLoop`: `HashRecognize(*CurLoop,
    /// *SE).getResult()` and `optimizeCRCLoop`.
    pub fn run_on_crc_loop(
        &mut self,
        run: &mut Run,
    ) -> bool {
        if self.options.disable_hash_recognize || !run.cur_loop.is_innermost() || run.cur_loop.get_blocks().len() != 1 {
            return false;
        }
        let header = run.cur_loop.get_header();
        let (Some(exit), Some(latch)) =
            (run.cur_loop.get_exit_block(run.function), run.cur_loop.get_loop_latch(run.function))
        else {
            return false;
        };
        if latch != header {
            return false;
        }
        let function = &*run.function;
        let Some(indvar) = canonical_induction_variable(run, header) else { return false };
        let Some(terminator) = function.terminator(header) else { return false };
        let branch = function.instruction(terminator);
        let latch_cmp = match branch.operands.first() {
            Some(&condition) if branch.operands.len() == 3 => {
                let Operand::Value(value) = condition else { return false };
                let llrm_mir::module::ValueDef::Instruction(cmp) = function.value(value).def else { return false };
                if !matches!(function.instruction(cmp).opcode, Opcode::ICmp(_)) || function.parent(cmp) != Some(header)
                {
                    return false;
                }
                cmp
            }
            _ => return false,
        };
        let trip_count = {
            let cx = cx!(run);
            run.se.get_small_constant_trip_count(&cx, header)
        };
        let Ok(info) = recognize_crc(&*run.context, &*run.function, header, exit, indvar, latch_cmp, trip_count) else {
            return false;
        };
        self.optimize_crc_loop(run, &info, indvar, latch_cmp)
    }

    /// `optimizeCRCLoop`, for the Auto strategy.
    fn optimize_crc_loop(
        &mut self,
        run: &mut Run,
        info: &PolynomialInfo,
        indvar: InstId,
        latch_cmp: InstId,
    ) -> bool {
        let tti: std::rc::Rc<dyn llrm_mir::tti::Tti> =
            self.tti.clone().unwrap_or_else(|| std::rc::Rc::new(llrm_mir::tti::Basic));
        let ty = Ty::Int(info.width);
        let kind = CostKind::Latency;
        let arith = |op| tti.arithmetic_cost(op, ty, kind, OperandInfo::ANY, OperandInfo::ANY);
        let (xor, shift, and) = (arith(ArithOp::Xor), arith(ArithOp::LShr), arith(ArithOp::And));
        let select = tti.cmp_sel_cost(CmpSel::Select, ty, kind);
        let load = tti.memory_op_cost(MemoryOp::Load, ty, kind, false);

        // CRC computation is mostly serial, so latency works best for
        // comparison.
        let original = (shift * 2 + xor * 2 + and + select) * i64::from(info.trip_count);
        let table = if info.trip_count % 8 != 0 {
            llrm_mir::tti::Cost::INVALID
        } else {
            (load + xor + shift * 2) * i64::from(info.trip_count / 8)
        };
        // D14: there is no clmul strategy.
        let clmul = llrm_mir::tti::Cost::INVALID;

        // When using the auto strategy, bail if we are optimizing for size
        // since there's usually not a clear size benefit.
        if self.apply_code_size_heuristics || (self.for_size && self.options.use_lir_code_size_heurs) {
            return false;
        }
        // Only apply an optimization if there's a clear benefit to doing so.
        let best = if less(clmul, table) { clmul } else { table };
        if !less(best, original) {
            return false;
        }
        if !less(clmul, table) {
            self.optimize_crc_loop_using_table_lookup(run, info, indvar, latch_cmp);
            return true;
        }
        false
    }

    /// `optimizeCRCLoopUsingTableLookup`.
    fn optimize_crc_loop_using_table_lookup(
        &mut self,
        run: &mut Run,
        info: &PolynomialInfo,
        indvar: InstId,
        latch_cmp: InstId,
    ) {
        debug_assert_eq!(info.trip_count % 8, 0, "A byte-multiple trip count is required");
        let header = run.cur_loop.get_header();
        let preheader = run.cur_loop.get_loop_preheader(run.function).expect("a preheader");
        let context = &mut *run.context;
        let function = &mut *run.function;

        // First, create a new GlobalVariable corresponding to the
        // Sarwate-lookup-table.
        let crc_ty = context.types.int(info.width);
        let cells: Vec<_> = gen_sarwate_table(info.rhs, info.width, info.is_big_endian)
            .iter()
            .map(|&entry| context.constant(Constant { ty: crc_ty, kind: ConstantKind::Int(entry) }))
            .collect();
        let array = context.types.intern(Type::Array { element: crc_ty, count: 256 });
        let initial = context.constant(Constant { ty: array, kind: ConstantKind::Aggregate(cells) });
        let table = run.declared.table(".crctable", array, initial);
        let pointer = context.types.ptr(0);
        let table = Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(table) }));

        // Next, mark all PHIs for removal except IV.
        let phis: Vec<InstId> = function
            .iter_block(header)
            .take_while(|&one| function.instruction(one).opcode == Opcode::Phi)
            .filter(|&one| one != indvar)
            .collect();
        for &phi in &phis {
            let value = function.instruction(phi).result.expect("a phi has a value");
            let ty = function.instruction(phi).ty;
            let poison = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }));
            function.replace_all_uses_with(value, poison);
        }

        // Next, fix up the trip count.
        let indvar_value = Operand::Value(function.instruction(indvar).result.expect("a value"));
        let iv_ty = function.instruction(indvar).ty;
        let terminator = function.terminator(header).expect("a terminator");
        let exit_pred = if function.instruction(terminator).operands[1] == Operand::Block(header) {
            IntPredicate::Ne
        } else {
            IntPredicate::Eq
        };
        let limit = Operand::Constant(context.int(iv_ty, i128::from(info.trip_count / 8) - 1));
        let bool_ty = context.types.int(1);
        let exit_cond = function.create_instruction(
            Opcode::ICmp(exit_pred),
            bool_ty,
            vec![indvar_value, limit],
            Flags::default(),
            Some("exit.cond"),
        );
        function.insert(exit_cond, Position::Before(latch_cmp)).expect("a placed compare");
        let old = function.instruction(latch_cmp).result.expect("a value");
        function.replace_all_uses_with(old, Operand::Value(function.instruction(exit_cond).result.expect("a value")));
        function.erase(latch_cmp).expect("a dead compare");

        // Finally, fill the loop with the Sarwate-table-lookup logic, and
        // replace all uses of ComputedValue.
        //
        // Little-endian:
        //   crc = (crc >> 8) ^ tbl[(iv'th byte of data) ^ (bottom byte of crc)]
        // Big-Endian:
        //   crc = (crc << 8) ^ tbl[(iv'th byte of data) ^ (top byte of crc)]
        let first = function
            .iter_block(header)
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a non-phi instruction");
        let mut emit = |function: &mut llrm_mir::module::Function,
                        opcode: Opcode,
                        ty,
                        operands: Vec<Operand>,
                        flags: Flags,
                        name: &str| {
            let inst = function.create_instruction(opcode, ty, operands, flags, Some(name));
            function.insert(inst, Position::Before(first)).expect("a placed instruction");
            Operand::Value(function.instruction(inst).result.expect("a value"))
        };
        let int = |context: &llrm_mir::context::Context, ty, value: i128| Operand::Constant(context.int(ty, value));

        // Create the CRC PHI, and initialize its incoming value to the
        // initial value of CRC.
        let crc_phi =
            emit(function, Opcode::Phi, crc_ty, vec![info.lhs, Operand::Block(preheader)], Flags::default(), "crc");
        let crc = crc_phi;
        let i8_ty = context.types.int(8);
        let cast = |function: &mut llrm_mir::module::Function,
                    context: &llrm_mir::context::Context,
                    emit: &mut dyn FnMut(
            &mut llrm_mir::module::Function,
            Opcode,
            llrm_mir::types::TypeId,
            Vec<Operand>,
            Flags,
            &str,
        ) -> Operand,
                    value: Operand,
                    to: llrm_mir::types::TypeId,
                    name: &str| {
            let from = function.operand_type(context, value).expect("a typed value");
            let (from_bits, to_bits) = (context.types.int_bits(from).unwrap(), context.types.int_bits(to).unwrap());
            match from_bits.cmp(&to_bits) {
                std::cmp::Ordering::Equal => value,
                std::cmp::Ordering::Greater => {
                    emit(function, Opcode::Cast(CastOp::Trunc), to, vec![value], Flags::default(), name)
                }
                std::cmp::Ordering::Less => {
                    emit(function, Opcode::Cast(CastOp::ZExt), to, vec![value], Flags::default(), name)
                }
            }
        };

        // TableIndexer = ((top|bottom) byte of CRC). It is XOR'ed with (iv'th
        // byte of LHSAux), if LHSAux is non-nullptr.
        let mut indexer = crc;
        if let Some(data) = info.lhs_aux {
            let data_ty = function.operand_type(context, data).expect("a typed value");
            let three = int(context, iv_ty, 3);
            let iv_bits = emit(
                function,
                Opcode::Binary(BinaryOp::Shl),
                iv_ty,
                vec![indvar_value, three],
                Flags::default(),
                "iv.bits",
            );
            let iv_indexer = cast(function, context, &mut emit, iv_bits, data_ty, "iv.indexer");
            let data_indexer = emit(
                function,
                Opcode::Binary(if info.is_big_endian { BinaryOp::Shl } else { BinaryOp::LShr }),
                data_ty,
                vec![data, iv_indexer],
                Flags::default(),
                "data.indexer",
            );
            let crc_cast = cast(function, context, &mut emit, indexer, data_ty, "crc.indexer.cast");
            indexer = emit(
                function,
                Opcode::Binary(BinaryOp::Xor),
                data_ty,
                vec![data_indexer, crc_cast],
                Flags::default(),
                "crc.data.indexer",
            );
        }
        indexer = if info.is_big_endian {
            // Shift the top bits of Op to the bottom byte by using the CRC
            // bitwidth as a reference.
            let mut op = indexer;
            let op_ty = function.operand_type(context, op).expect("a typed value");
            let op_bits = context.types.int_bits(op_ty).unwrap();
            if info.width != 8 {
                op = if info.width > 8 {
                    let by = int(context, op_ty, i128::from(info.width - 8));
                    emit(function, Opcode::Binary(BinaryOp::LShr), op_ty, vec![op, by], Flags::default(), "indexer.hi")
                } else {
                    let by = int(context, op_ty, i128::from(8 - info.width));
                    emit(function, Opcode::Binary(BinaryOp::Shl), op_ty, vec![op, by], Flags::default(), "indexer.hi")
                };
            }
            let _ = op_bits;
            cast(function, context, &mut emit, op, i8_ty, "indexer.hi.lo.byte")
        } else {
            cast(function, context, &mut emit, indexer, i8_ty, "indexer.lo")
        };

        // Always index into a GEP using the index type.
        let index_ty = context.types.int(run.env.layout.pointer(0).index_bits);
        let indexer =
            emit(function, Opcode::Cast(CastOp::ZExt), index_ty, vec![indexer], Flags::default(), "indexer.ext");

        // CRCTableLd = CRCTable[(iv'th byte of data) ^ (top|bottom) byte of
        // CRC].
        let gep = emit(
            function,
            Opcode::GetElementPtr { source: crc_ty },
            pointer,
            vec![table, indexer],
            Flags::INBOUNDS,
            "tbl.ptradd",
        );
        let table_ld = emit(
            function,
            Opcode::Load { align: None, volatile: false },
            crc_ty,
            vec![gep],
            Flags::default(),
            "tbl.ld",
        );

        // CRCNext = (CRC (<<|>>) 8) ^ CRCTableLd, or simply CRCTableLd in case
        // of CRC-8.
        let mut crc_next = table_ld;
        if info.width > 8 {
            let eight = int(context, crc_ty, 8);
            let shifted = emit(
                function,
                Opcode::Binary(if info.is_big_endian { BinaryOp::Shl } else { BinaryOp::LShr }),
                crc_ty,
                vec![crc, eight],
                Flags::default(),
                if info.is_big_endian { "crc.be.shift" } else { "crc.le.shift" },
            );
            crc_next = emit(
                function,
                Opcode::Binary(BinaryOp::Xor),
                crc_ty,
                vec![shifted, table_ld],
                Flags::default(),
                "crc.next",
            );
        }

        // Connect the back-edge for the loop, and RAUW the ComputedValue.
        let Operand::Value(phi_value) = crc_phi else { unreachable!("a phi value") };
        let ValueDefOf::Instruction(phi_inst) = value_def(function, phi_value) else { unreachable!("a phi") };
        let mut operands = function.instruction(phi_inst).operands.clone();
        operands.extend([crc_next, Operand::Block(header)]);
        function.set_operands(phi_inst, operands);
        let computed = function.instruction(info.computed_value).result.expect("a value");
        replace_uses_outside_block(function, computed, crc_next, header);

        // Cleanup.
        for phi in phis {
            delete_dead_chain(function, phi);
        }
    }
}

use llrm_mir::module::ValueDef as ValueDefOf;

fn value_def(
    function: &llrm_mir::module::Function,
    value: llrm_mir::module::ValueId,
) -> ValueDefOf {
    function.value(value).def
}

/// `InstructionCost::operator<`: a valid cost is below an invalid one.
fn less(
    a: llrm_mir::tti::Cost,
    b: llrm_mir::tti::Cost,
) -> bool {
    match (a.valid, b.valid) {
        (true, true) => a.value < b.value,
        (true, false) => true,
        _ => false,
    }
}

/// `Value::replaceUsesOutsideBlock`.
fn replace_uses_outside_block(
    function: &mut llrm_mir::module::Function,
    value: llrm_mir::module::ValueId,
    with: Operand,
    block: BlockId,
) {
    let uses: Vec<_> = function.users(value).iter().map(|one| (one.user, one.index)).collect();
    for (user, index) in uses {
        if function.parent(user) != Some(block) {
            function.set_operand(user, index as usize, with);
        }
    }
}

/// `RecursivelyDeleteDeadPHINode`, for the pure chain of a recognized CRC: the
/// instruction if nothing uses it, then the operands that leaves unused.
fn delete_dead_chain(
    function: &mut llrm_mir::module::Function,
    inst: InstId,
) {
    let mut worklist = vec![inst];
    while let Some(inst) = worklist.pop() {
        if function.is_erased(inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        let pure = matches!(
            instruction.opcode,
            Opcode::Phi | Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::Select
        );
        if !pure || instruction.result.is_none_or(|result| !function.users(result).is_empty()) {
            continue;
        }
        let operands = instruction.operands.clone();
        function.erase(inst).expect("a dead instruction");
        for operand in operands {
            if let Operand::Value(value) = operand
                && let ValueDefOf::Instruction(def) = function.value(value).def
            {
                worklist.push(def);
            }
        }
    }
}

/// `Loop::getCanonicalInductionVariable`: a phi of the header that starts at
/// zero and adds one on the backedge.
fn canonical_induction_variable(
    run: &Run,
    header: BlockId,
) -> Option<InstId> {
    let function = &*run.function;
    let context = &*run.context;
    function
        .iter_block(header)
        .take_while(|&one| function.instruction(one).opcode == Opcode::Phi)
        .find(
            |&phi| {
                let operands = &function.instruction(phi).operands;
                if operands.len() != 4 {
                    return false;
                }
                let constant = |operand: Operand, want: u128| match operand {
                    Operand::Constant(id) => match context.get(id).kind {
                        ConstantKind::Int(value) => value == want,
                        ConstantKind::Zero => want == 0,
                        _ => false,
                    },
                    _ => false,
                };
                let value = Operand::Value(function.instruction(phi).result.expect("a value"));
                (0..2).any(|at| {
                    let from_latch = operands[at * 2 + 1] == Operand::Block(header);
                    let other = operands[(1 - at) * 2];
                    let Operand::Value(next) = operands[at * 2] else { return false };
                    if !from_latch || !constant(other, 0) {
                        return false;
                    }
                    let ValueDefOf::Instruction(next) = function.value(next).def else { return false };
                    let add = function.instruction(next);
                    add.opcode == Opcode::Binary(BinaryOp::Add)
                        && ((add.operands[0] == value && constant(add.operands[1], 1))
                            || (add.operands[1] == value && constant(add.operands[0], 1)))
                })
            },
        )
}
