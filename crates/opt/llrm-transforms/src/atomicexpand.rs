//! A read-modify-write the target has no instruction for, as a compare-exchange
//! loop: LLVM's `AtomicExpandPass` (`expandAtomicRMWToCmpXchg`).
//!
//! # How it works
//! The block is split at the call. The first block loads the value (a plain
//! load), the loop block computes the new value from the loaded one, a
//! compare-exchange of loaded for new gives the old value and whether it
//! matched, and a mismatch retries with that old value. The exit's result is
//! the last old value. The failure ordering is the strongest the success one
//! allows (`getStrongestFailureOrdering`).
//!
//! # Example
//! `%o = atomicrmw add ptr %p, i32 %v seq_cst`
//!
//! ```text
//!   %init = load i32, ptr %p ; br atomicrmw.start
//! atomicrmw.start: %loaded = phi [%init, pre], [%new_loaded, atomicrmw.start]
//!   %new = add %loaded, %v
//!   %pair = cmpxchg ptr %p, %loaded, %new seq_cst seq_cst
//!   %new_loaded = extractvalue %pair, 0 ; %ok = extractvalue %pair, 1
//!   br %ok, atomicrmw.end, atomicrmw.start
//! ```
//!
//! # Runs
//! In the m68k driver for a 68020 and later (`shouldExpandAtomicRMWInIR` is
//! `CmpXChg` there), on the atomics `llrm_mir::atomic::hidden` made calls of.
//!
//! # See also
//! LLVM's `AtomicExpandImpl::expandAtomicRMWToCmpXchg`
//! ([AtomicExpandPass.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/AtomicExpandPass.cpp)).

use llrm_mir::atomic::{decode, hidden_callee};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{
    AtomicInfo, AtomicKind, AtomicOrdering, BinaryOp, CallInfo, Flags, IntPredicate, Opcode, RmwOp, Tail,
};
use llrm_mir::types::{Type, TypeId};

use crate::edges::split_block_at;

/// The failure ordering of a compare-exchange that succeeds with `success`
/// (`AtomicCmpXchgInst::getStrongestFailureOrdering`).
fn strongest_failure(success: AtomicOrdering) -> AtomicOrdering {
    match success {
        AtomicOrdering::SeqCst => AtomicOrdering::SeqCst,
        AtomicOrdering::Acquire | AtomicOrdering::AcqRel => AtomicOrdering::Acquire,
        _ => AtomicOrdering::Monotonic,
    }
}

/// Every read-modify-write call of at most `native` bits as a compare-exchange
/// loop.
pub fn expanded(
    module: &mut Module,
    native: u32,
) -> Result<(), String> {
    let mut found: Vec<(usize, InstId, AtomicInfo, RmwOp)> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            let Some(callee) = llrm_mir::memory::callee(&module.context, function, inst) else { continue };
            if let Some(info) = module.globals[callee.0 as usize].name.as_deref().and_then(decode)
                && let AtomicKind::Rmw(op) = info.kind
            {
                found.push((at, inst, info, op));
            }
        }
    }
    for (at, call, info, op) in found {
        let GlobalKind::Function(function) = &module.globals[at].kind else { continue };
        let instruction = function.instruction(call).clone();
        let (pointer, value) = (instruction.operands[0], instruction.operands[1]);
        let ty = function.operand_type(&module.context, value).ok_or("an operand has a type")?;
        if module.context.types.int_bits(ty).is_none_or(|bits| bits > native) {
            continue;
        }
        let pointer_type = function.operand_type(&module.context, pointer).ok_or("a pointer has a type")?;
        let flag = module.context.types.int(1);
        let pair = module.context.types.intern(Type::Struct { fields: vec![ty, flag], packed: false });
        let exchange =
            AtomicInfo { kind: AtomicKind::CmpXchg { failure: strongest_failure(info.ordering), weak: false }, ..info };
        let signature = module
            .context
            .types
            .intern(
                Type::Function {
                    returns: pair,
                    parameters: vec![pointer_type, ty, ty],
                    variadic: false,
                },
            );
        let callee = hidden_callee(module, exchange, signature)?;
        let GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
        let pre = function.parent(call).ok_or("a placed call")?;
        let first = function.next_in_block(call).ok_or("a terminated block")?;
        let end = split_block_at(function, pre, first);
        function.set_block_name(end, "atomicrmw.end");
        let start = function.create_block(Some("atomicrmw.start"));
        function.insert_block(start, Some(pre)).map_err(|why| why.to_string())?;
        let jump = function.terminator(pre).ok_or("a terminated block")?;
        let void = function.instruction(jump).ty;
        function.erase(jump).map_err(|why| why.to_string())?;
        let align = info.align;
        let init = function.create_instruction(
            Opcode::Load { align, volatile: info.volatile },
            ty,
            vec![pointer],
            Flags::default(),
            None,
        );
        function.insert(init, Position::End(pre)).map_err(|why| why.to_string())?;
        let init_value = Operand::Value(function.instruction(init).result.ok_or("a loaded value")?);
        let enter = function.create_instruction(Opcode::Br, void, vec![Operand::Block(start)], Flags::default(), None);
        function.insert(enter, Position::End(pre)).map_err(|why| why.to_string())?;
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("loaded"));
        function.insert(phi, Position::End(start)).map_err(|why| why.to_string())?;
        let loaded = Operand::Value(function.instruction(phi).result.ok_or("a phi result")?);
        let new = perform(function, &mut module.context, start, op, ty, loaded, value)?;
        let call_info = CallInfo {
            function_type: signature,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 3],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let swap = function.create_instruction(
            Opcode::Call(Box::new(call_info)),
            pair,
            vec![pointer, loaded, new, callee],
            Flags::default(),
            None,
        );
        function.insert(swap, Position::End(start)).map_err(|why| why.to_string())?;
        let swapped = Operand::Value(function.instruction(swap).result.ok_or("a pair")?);
        let mut extract = |index: u32, ty| -> Result<Operand, String> {
            let inst = function.create_instruction(
                Opcode::ExtractValue(vec![index]),
                ty,
                vec![swapped],
                Flags::default(),
                None,
            );
            function.insert(inst, Position::End(start)).map_err(|why| why.to_string())?;
            Ok(Operand::Value(function.instruction(inst).result.ok_or("an extracted value")?))
        };
        let new_loaded = extract(0, ty)?;
        let success = extract(1, flag)?;
        let back = function.create_instruction(
            Opcode::Br,
            void,
            vec![success, Operand::Block(end), Operand::Block(start)],
            Flags::default(),
            None,
        );
        function.insert(back, Position::End(start)).map_err(|why| why.to_string())?;
        function.set_operands(phi, vec![init_value, Operand::Block(pre), new_loaded, Operand::Block(start)]);
        if let Some(old) = instruction.result {
            function.replace_all_uses_with(old, new_loaded);
        }
        function.erase(call).map_err(|why| why.to_string())?;
    }
    Ok(())
}

fn emit(
    function: &mut Function,
    block: BlockId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(inst, Position::End(block)).map_err(|why| why.to_string())?;
    Ok(Operand::Value(function.instruction(inst).result.ok_or("a result")?))
}

/// `buildAtomicRMWValue`: the value `op` stores, from the one loaded.
fn perform(
    function: &mut Function,
    context: &mut Context,
    block: BlockId,
    op: RmwOp,
    ty: TypeId,
    loaded: Operand,
    value: Operand,
) -> Result<Operand, String> {
    let pair = vec![loaded, value];
    let binary = |function: &mut Function, op| emit(function, block, Opcode::Binary(op), ty, pair.clone());
    let pick = |function: &mut Function, context: &mut Context, predicate| {
        let flag = context.types.int(1);
        let test = emit(function, block, Opcode::ICmp(predicate), flag, pair.clone())?;
        emit(function, block, Opcode::Select, ty, vec![test, loaded, value])
    };
    match op {
        RmwOp::Xchg => Ok(value),
        RmwOp::Add => binary(function, BinaryOp::Add),
        RmwOp::Sub => binary(function, BinaryOp::Sub),
        RmwOp::And => binary(function, BinaryOp::And),
        RmwOp::Or => binary(function, BinaryOp::Or),
        RmwOp::Xor => binary(function, BinaryOp::Xor),
        RmwOp::Nand => {
            let both = binary(function, BinaryOp::And)?;
            let ones = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(u128::MAX) }));
            emit(function, block, Opcode::Binary(BinaryOp::Xor), ty, vec![both, ones])
        }
        RmwOp::Max => pick(function, context, IntPredicate::Sgt),
        RmwOp::Min => pick(function, context, IntPredicate::Sle),
        RmwOp::UMax => pick(function, context, IntPredicate::Ugt),
        RmwOp::UMin => pick(function, context, IntPredicate::Ule),
    }
}
