//! Aligns a static local that wants more than the stack gives, in MIR before
//! instruction selection.
//!
//! # How it works
//! The frame keeps the stack aligned to 16 at most, so a `_Alignas(64)` local
//! lay wherever the stack happened to be. Until the frame realigns the stack
//! pointer (the pin's `and rsp, -64`), such an `alloca` becomes a byte buffer
//! with `align - 1` spare bytes, and its users address the first aligned
//! address in it:
//!
//! ```text
//!  %v = alloca [4 x i32], align 64
//!               |
//!               v
//!  %v = alloca i8, i32 79            ; 64 + 16 - 1
//!  %i = ptrtoint ptr %v to i64
//!  %a = add i64 %i, 63
//!  %m = and i64 %a, -64
//!  %p = inttoptr i64 %m to ptr       ; every user of %v but its lifetime
//!                                    ; markers reads %p
//! ```
//!
//! DEVIATE OA1 (owner w4, 2026-10-28): the pin realigns the stack instead and
//! addresses the local from the stack pointer; this costs the spare bytes and
//! four instructions per local. Exit: realign in `masm::lowered`. Dynamic
//! `alloca`s keep the alignment the DAG builder passes (`DynamicStackAlloc`).
//!
//! # Example
//! `int aligned(int n) { _Alignas(64) int v[4] = {n, 1, 2, 3}; return use(v);
//! }` at `-m64 -O1` first had `sub rsp, 24; lea rdi, [rsp]`. Now it takes 88
//! bytes and `lea rax, [rsp]; add rax, 63; and rax, -64` before the first use.
//!
//! From the compiler's `-S` output of `tests/frame_realign_m64.rs`' source.
//!
//! # Runs
//! `mir over-aligned locals` in `driver::optimized`, after `ehprepare`, once
//! per module, whatever the -O level.

use llrm_mir::GlobalId;
use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};

/// The most the frame aligns a local to by itself.
const KEPT: u64 = 16;

/// Every static `alloca` in `module` wanting more than 16 bytes of alignment
/// made aligned by hand.
pub fn aligned(
    module: &mut Module,
    layout: &llrm_target::layout::Layout,
) -> Result<(), String> {
    let data = DataLayout::parse(&layout.datalayout)?;
    let markers: Vec<GlobalId> = module
        .functions()
        .filter(|(_, global, _)| global.name.as_deref().is_some_and(|name| name.starts_with("llvm.lifetime.")))
        .map(|(id, ..)| id)
        .collect();
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        let Some(entry) = function.entry() else { continue };
        let found: Vec<(InstId, u64)> = function
            .iter_block(entry)
            .filter_map(|inst| {
                let instruction = function.instruction(inst);
                let Opcode::Alloca { allocated, align, .. } = instruction.opcode else { return None };
                let constant = instruction.operands.iter().all(|one| matches!(one, Operand::Constant(_)));
                let wanted = data.pref_align(&context.types, allocated).max(align.unwrap_or(1));
                (constant && wanted > KEPT).then_some((inst, wanted))
            })
            .collect();
        for (inst, wanted) in found {
            let instruction = function.instruction(inst).clone();
            let Opcode::Alloca { allocated, address_space, .. } = instruction.opcode else { unreachable!() };
            let count = match instruction.operands.first() {
                None => 1,
                Some(Operand::Constant(id)) => match context.get(*id).kind {
                    ConstantKind::Int(count) => count,
                    _ => return Err("an alloca whose count is not a number".into()),
                },
                Some(_) => unreachable!(),
            };
            let bytes = data.alloc_size(&context.types, allocated) as i128 * count as i128 + wanted as i128 - 1;
            let (result, pointer) = (instruction.result.ok_or("an alloca's address")?, instruction.ty);
            let wide = context.types.int(data.pointer(address_space).bits);
            let (byte, count_type) = (context.types.int(8), context.types.int(32));
            // The users to move: all but the lifetime markers, which cover the
            // whole buffer.
            let moved: Vec<_> = function
                .users(result)
                .into_iter()
                .filter(|one| {
                    let user = function.instruction(one.user);
                    let marker = matches!(user.opcode, Opcode::Call(_))
                        && matches!(
                            user.operands.last(),
                            Some(Operand::Constant(callee)) if matches!(context.get(*callee).kind, ConstantKind::Global(id) if markers.contains(&id))
                        );
                    !marker
                })
                .map(|one| (one.user, one.index as usize))
                .collect();
            function.set_opcode(inst, Opcode::Alloca { allocated: byte, align: Some(1), address_space });
            let size = Operand::Constant(context.int(count_type, bytes));
            function.set_operands(inst, vec![size]);
            let mut after = inst;
            let mut make = |function: &mut llrm_mir::module::Function, opcode, ty, operands: Vec<Operand>| {
                let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
                function.insert(made, Position::After(after))?;
                after = made;
                Ok::<_, String>(Operand::Value(function.instruction(made).result.expect("a value")))
            };
            let address = make(function, Opcode::Cast(CastOp::PtrToInt), wide, vec![Operand::Value(result)])?;
            let up = Operand::Constant(context.int(wide, wanted as i128 - 1));
            let added = make(function, Opcode::Binary(BinaryOp::Add), wide, vec![address, up])?;
            let mask = Operand::Constant(context.int(wide, -(wanted as i128)));
            let masked = make(function, Opcode::Binary(BinaryOp::And), wide, vec![added, mask])?;
            let aligned = make(function, Opcode::Cast(CastOp::IntToPtr), pointer, vec![masked])?;
            for (user, index) in moved {
                function.set_operand(user, index, aligned);
            }
        }
    }
    Ok(())
}
