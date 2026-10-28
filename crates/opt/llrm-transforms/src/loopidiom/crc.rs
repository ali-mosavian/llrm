//! `optimizeCRCLoop` and `optimizeCRCLoopUsingTableLookup` of
//! `LoopIdiomRecognize.cpp` at `d1106deb71cc`: a CRC loop `HashRecognize`
//! found becomes a Sarwate table lookup, one byte of data per trip.
//!
//! # How it works
//! [`LoopIdiomRecognize::optimize_crc_loop`] prices the loop against the table
//! strategy by the target's latencies (`TCK_Latency`: a CRC is serial) and
//! rewrites it where the table is cheaper and the trip count is a multiple of
//! eight. The loop keeps its induction variable and runs `trips / 8` times:
//!
//! ```text
//!   crc = (crc >> 8) ^ tbl[(iv'th byte of data) ^ (bottom byte of crc)]   little-endian
//!   crc = (crc << 8) ^ tbl[(iv'th byte of data) ^ (top byte of crc)]      big-endian
//! ```
//!
//! The table is a private constant of the module (`.crctable`); the old
//! recurrences are deleted with what only they used.
//!
//! # Deviations
//! - D9a: the carry-less multiply strategy (`optimizeCRCLoopUsingClmul`) is not
//!   translated: no target here has `llvm.clmul`, so `Auto` is the table where
//!   it pays, and `Clmul` forced is a loop left alone.
//! - Hexagon (`optimizeCRCLoop`'s first test) is not a target here.
//! - Optimization remarks and MemorySSA updates have no counterpart.

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::{ArithOp, CmpSel, Cost, CostKind, MemoryOp, OperandInfo, Tti, Ty};
use llrm_mir::types::{Type, TypeId};

use super::hashrecognize::{PolynomialInfo, gen_sarwate_table};
use super::process::Run;
use super::{CrcStrategy, LoopIdiomRecognize};

fn int(
    context: &Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.int(ty, value as i128))
}

fn width_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u32 {
    let ty = function.operand_type(context, operand).expect("a typed operand");
    context.types.int_bits(ty).expect("an integer")
}

/// An instruction made and placed before `at`.
fn emit(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    flags: Flags,
    name: &str,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, flags, Some(name));
    function.insert(inst, Position::Before(at)).expect("a placed instruction");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// `CreateZExtOrTrunc`.
fn zext_or_trunc(
    context: &Context,
    function: &mut Function,
    at: InstId,
    value: Operand,
    to: TypeId,
    name: &str,
) -> Operand {
    let from = width_of(context, function, value);
    let target = context.types.int_bits(to).expect("an integer");
    if from == target {
        return value;
    }
    let cast = if from < target { CastOp::ZExt } else { CastOp::Trunc };
    emit(function, at, Opcode::Cast(cast), to, vec![value], Flags::default(), name)
}

/// The users of `value` outside `block`, as (instruction, operand index).
fn uses_outside(
    function: &Function,
    value: llrm_mir::module::ValueId,
    block: llrm_mir::module::BlockId,
) -> Vec<(InstId, usize)> {
    function
        .users(value)
        .into_iter()
        .filter(|one| function.parent(one.user) != Some(block))
        .map(|one| (one.user, one.index as usize))
        .collect()
}

/// `RecursivelyDeleteDeadPHINode` for the dead recurrences here: the phi, and
/// each pure instruction that nothing else reads once it is gone.
fn delete_dead_cascade(
    function: &mut Function,
    first: InstId,
) {
    let mut worklist = vec![first];
    let mut erased: llrm_mir::dense::IdSet<InstId> = Default::default();
    while let Some(inst) = worklist.pop() {
        if erased.contains(&inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        let pure = match instruction.opcode {
            Opcode::Phi | Opcode::Select | Opcode::Cast(_) | Opcode::ICmp(_) => true,
            Opcode::Binary(op) => !matches!(
                op,
                BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
            ),
            _ => false,
        };
        let unused = instruction.result.is_none_or(|result| function.users(result).is_empty());
        if !pure || !unused {
            continue;
        }
        let operands: Vec<InstId> = instruction
            .operands
            .iter()
            .filter_map(|operand| match operand {
                Operand::Value(value) => match function.value(*value).def {
                    llrm_mir::module::ValueDef::Instruction(def) => Some(def),
                    llrm_mir::module::ValueDef::Argument(_) => None,
                },
                _ => None,
            })
            .collect();
        function.erase(inst).expect("a dead instruction");
        erased.insert(inst);
        worklist.extend(operands);
    }
}

impl LoopIdiomRecognize {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1596
    /// `optimizeCRCLoop`.
    pub fn optimize_crc_loop(
        &mut self,
        run: &mut Run,
        info: &PolynomialInfo,
        tti: &dyn Tti,
    ) -> bool {
        let crc_bw = info.rhs_width;
        // CRC computation is mostly serial, so latency works best for
        // comparison.
        let kind = CostKind::Latency;
        let ty = Ty::Int(crc_bw);
        let arith = |op| tti.arithmetic_cost(op, ty, kind, OperandInfo::ANY, OperandInfo::ANY);
        let xor = arith(ArithOp::Xor);
        let shift = arith(ArithOp::LShr);
        let and = arith(ArithOp::And);
        let select = tti.cmp_sel_cost(CmpSel::Select, ty, kind);
        let load = tti.memory_op_cost(MemoryOp::Load, ty, kind, false);

        let trips = i64::from(info.trip_count);
        // Estimate the cost of the original, unoptimized loop.
        let orig_loop_cost = (shift * 2 + xor * 2 + and + select) * trips;
        // Estimate the cost of the Sarwate lookup table optimization strategy.
        // A byte-multiple trip count is required.
        let table_strategy_cost =
            if info.trip_count % 8 != 0 { Cost::INVALID } else { (load + xor + shift * 2) * (trips / 8) };

        match self.options.crc_strategy {
            CrcStrategy::Disable | CrcStrategy::Clmul => false,
            CrcStrategy::Table => {
                // The table strategy is not possible in its current form
                // without a byte-multiple trip count.
                if info.trip_count % 8 == 0 {
                    self.optimize_crc_loop_using_table_lookup(run, info);
                    return true;
                }
                false
            }
            CrcStrategy::Auto => {
                // When using the auto strategy, bail if we are optimizing for
                // size since there's usually not a clear size benefit.
                if self.apply_code_size_heuristics || self.for_size {
                    return false;
                }
                // Only apply an optimization if there's a clear benefit to
                // doing so.
                if !table_strategy_cost.valid || table_strategy_cost.value >= orig_loop_cost.value {
                    return false;
                }
                self.optimize_crc_loop_using_table_lookup(run, info);
                true
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L1852
    /// `optimizeCRCLoopUsingTableLookup`.
    fn optimize_crc_loop_using_table_lookup(
        &mut self,
        run: &mut Run,
        info: &PolynomialInfo,
    ) {
        assert!(info.trip_count % 8 == 0, "A byte-multiple trip count is required");
        let context = &*run.context;
        let function = &mut *run.function;
        let l = run.cur_loop;
        let header = l.get_header();
        let latch = l.get_loop_latch(&*function).expect("a latch");
        let preheader = l.get_loop_preheader(&*function).expect("a preheader");

        // First, create a new GlobalVariable corresponding to the
        // Sarwate-lookup-table.
        let crc_ty = function.operand_type(context, info.lhs).expect("a typed crc");
        let crc_bw = info.rhs_width;
        let cells: Vec<_> = gen_sarwate_table(info.rhs, crc_bw, info.is_big_endian)
            .iter()
            .map(|entry| match int(context, crc_ty, *entry) {
                Operand::Constant(id) => id,
                _ => unreachable!("a constant"),
            })
            .collect();
        let array = context.types.intern(Type::Array { element: crc_ty, count: 256 });
        let initial = context.constant(llrm_mir::context::Constant {
            ty: array,
            kind: llrm_mir::context::ConstantKind::Aggregate(cells),
        });
        let table = run.declared.table(".crctable", array, initial);

        let iv_phi = l.get_canonical_induction_variable(context, &*function).expect("a canonical induction variable");
        let iv = Operand::Value(function.instruction(iv_phi).result.expect("a value"));
        let iv_ty = function.operand_type(context, iv).expect("a typed iv");

        // Next, mark all PHIs for removal except IV.
        let mut cleanup: Vec<InstId> = Vec::new();
        let phis: Vec<InstId> =
            function.iter_block(header).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
        for phi in phis {
            if phi == iv_phi {
                continue;
            }
            let value = function.instruction(phi).result.expect("a phi has a value");
            let ty = function.value(value).ty;
            let poison = Operand::Constant(
                context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
            );
            function.replace_all_uses_with(value, poison);
            cleanup.push(phi);
        }

        // Next, fix up the trip count.
        {
            let new_btc = u128::from(info.trip_count / 8 - 1);
            let branch = function.terminator(latch).expect("a branch");
            let exit_pred = if function.instruction(branch).operands[1] == Operand::Block(latch) {
                IntPredicate::Ne
            } else {
                IntPredicate::Eq
            };
            let exit_cond = l.get_latch_cmp_inst(&*function).expect("a latch compare");
            let limit = int(context, iv_ty, new_btc);
            let i1 = context.types.int(1);
            let new_exit_cond =
                emit(function, exit_cond, Opcode::ICmp(exit_pred), i1, vec![iv, limit], Flags::default(), "exit.cond");
            let old = function.instruction(exit_cond).result.expect("a compare has a value");
            function.replace_all_uses_with(old, new_exit_cond);
            function.erase(exit_cond).expect("a dead compare");
        }

        // Finally, fill the loop with the Sarwate-table-lookup logic, and
        // replace all uses of ComputedValue.
        //
        // Little-endian:
        //   crc = (crc >> 8) ^ tbl[(iv'th byte of data) ^ (bottom byte of crc)]
        // Big-Endian:
        //   crc = (crc << 8) ^ tbl[(iv'th byte of data) ^ (top byte of crc)]
        let first_non_phi = function
            .iter_block(header)
            .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            .expect("a non-phi in the header");
        let i8 = context.types.int(8);
        let crc_phi_inst = function.create_instruction(
            Opcode::Phi,
            crc_ty,
            vec![info.lhs, Operand::Block(preheader), info.lhs, Operand::Block(latch)],
            Flags::default(),
            Some("crc"),
        );
        function.insert(crc_phi_inst, Position::Before(first_non_phi)).expect("a placed phi");
        let crc = Operand::Value(function.instruction(crc_phi_inst).result.expect("a value"));
        let at = first_non_phi;

        let lo_byte =
            |function: &mut Function, op: Operand, name: &str| zext_or_trunc(context, function, at, op, i8, name);
        // Shift the top bits of Op to the bottom byte by using the CRC
        // bitwidth as a reference.
        let hi_idx = |function: &mut Function, mut op: Operand, name: &str| {
            if crc_bw != 8 {
                let (opcode, amount) =
                    if crc_bw > 8 { (BinaryOp::LShr, crc_bw - 8) } else { (BinaryOp::Shl, 8 - crc_bw) };
                let ty = function.operand_type(context, op).expect("typed");
                op = emit(
                    function,
                    at,
                    Opcode::Binary(opcode),
                    ty,
                    vec![op, int(context, ty, u128::from(amount))],
                    Flags::default(),
                    name,
                );
            }
            lo_byte(function, op, &format!("{name}.lo.byte"))
        };

        // TableIndexer = ((top|bottom) byte of CRC). It is XOR'ed with (iv'th
        // byte of LHSAux), if LHSAux is present.
        let mut indexer = crc;
        if let Some(data) = info.lhs_aux {
            let data_ty = function.operand_type(context, data).expect("typed data");
            // To index into the (iv'th byte of LHSAux), we multiply iv by 8,
            // and we shift right by that amount, and take the lo-byte (in the
            // little-endian case), or shift left by that amount, and take the
            // hi-idx (in the big-endian case).
            let iv_bits = emit(
                function,
                at,
                Opcode::Binary(BinaryOp::Shl),
                iv_ty,
                vec![iv, int(context, iv_ty, 3)],
                Flags::default(),
                "iv.bits",
            );
            let iv_indexer = zext_or_trunc(context, function, at, iv_bits, data_ty, "iv.indexer");
            let data_op = if info.is_big_endian { BinaryOp::Shl } else { BinaryOp::LShr };
            let data_indexer = emit(
                function,
                at,
                Opcode::Binary(data_op),
                data_ty,
                vec![data, iv_indexer],
                Flags::default(),
                "data.indexer",
            );
            let crc_indexer = zext_or_trunc(context, function, at, indexer, data_ty, "crc.indexer.cast");
            indexer = emit(
                function,
                at,
                Opcode::Binary(BinaryOp::Xor),
                data_ty,
                vec![data_indexer, crc_indexer],
                Flags::default(),
                "crc.data.indexer",
            );
        }
        indexer = if info.is_big_endian {
            hi_idx(function, indexer, "indexer.hi")
        } else {
            lo_byte(function, indexer, "indexer.lo")
        };

        // Always index into a GEP using the index type.
        let pointer = context.types.ptr(0);
        let index_ty = context.types.int(run.env.layout.pointer(0).index_bits);
        let indexer = zext_or_trunc(context, function, at, indexer, index_ty, "indexer.ext");
        let base = Operand::Constant(context.constant(llrm_mir::context::Constant {
            ty: pointer,
            kind: llrm_mir::context::ConstantKind::Global(table),
        }));
        // CRCTableLd = CRCTable[(iv'th byte of data) ^ (top|bottom) byte of
        // CRC].
        let table_gep = emit(
            function,
            at,
            Opcode::GetElementPtr { source: crc_ty },
            pointer,
            vec![base, indexer],
            Flags::INBOUNDS,
            "tbl.ptradd",
        );
        let table_ld = emit(
            function,
            at,
            Opcode::Load { align: None, volatile: false },
            crc_ty,
            vec![table_gep],
            Flags::default(),
            "tbl.ld",
        );

        // CRCNext = (CRC (<<|>>) 8) ^ CRCTableLd, or simply CRCTableLd in case
        // of CRC-8.
        let mut crc_next = table_ld;
        if crc_bw > 8 {
            let shift = if info.is_big_endian { BinaryOp::Shl } else { BinaryOp::LShr };
            let crc_shift = emit(
                function,
                at,
                Opcode::Binary(shift),
                crc_ty,
                vec![crc, int(context, crc_ty, 8)],
                Flags::default(),
                if info.is_big_endian { "crc.be.shift" } else { "crc.le.shift" },
            );
            crc_next = emit(
                function,
                at,
                Opcode::Binary(BinaryOp::Xor),
                crc_ty,
                vec![crc_shift, table_ld],
                Flags::default(),
                "crc.next",
            );
        }

        // Connect the back-edge for the loop, and RAUW the ComputedValue.
        function.set_operand(crc_phi_inst, 2, crc_next);
        let computed = function.instruction(info.computed_value).result.expect("a select has a value");
        for (user, index) in uses_outside(function, computed, latch) {
            function.set_operand(user, index, crc_next);
        }

        // Cleanup.
        for phi in cleanup {
            delete_dead_cascade(function, phi);
        }
    }
}
