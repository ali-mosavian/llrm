//! Whether a variadic function can read the vector half of its register save
//! area.
//!
//! Translation: GCC `tree-stdarg.cc` (`pass_stdarg`: `va_list_fpr_size` is 0
//! when no `va_arg` of the function takes a floating-point value, and the
//! prologue then saves no vector register and sets no `fp_offset`). The pin's
//! `LowerFormalArguments` saves them behind an `al` test whatever the body
//! does.
//!
//! DEVIATE STDARG-FP (owner x86-m64, exit 2026-12-12): the pin
//! has no such analysis. A `va_list` that is a slot of the function, touched
//! only by `va_start`, `va_end`, the lifetime markers and loads and stores at
//! constant offsets none of which reaches `fp_offset` (bytes 4..8), has no
//! reader of the vector area: `va_arg` of a `double` reads `fp_offset` first.
//!
//! # Example
//! `sum(short n, ...)` taking only `long`s: no `test al, al` and eight
//! `movaps` less, and `va_start` stores `gp_offset` alone.

use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::Opcode;

/// The bytes of the SysV x86-64 `va_list` that hold `fp_offset`.
const FP_OFFSET: std::ops::Range<u64> = 4..8;

fn intrinsic(
    module: &Module,
    function: &Function,
    inst: InstId,
) -> Option<Intrinsic> {
    match function.instruction(inst).opcode {
        Opcode::Call(_) => match *function.instruction(inst).operands.last()? {
            Operand::Constant(id) => match module.context.get(id).kind {
                ConstantKind::Global(global) => module.global(global).intrinsic(),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn overlaps_fp_offset(
    offset: u64,
    size: u64,
) -> bool {
    offset < FP_OFFSET.end && offset + size > FP_OFFSET.start
}

/// The value of an integer constant operand.
fn constant_index(
    module: &Module,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match module.context.get(id).kind {
            ConstantKind::Int(value) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

/// Whether every use of `list` (and of the addresses made from it, `offset`
/// bytes in) stays clear of `fp_offset`.
fn clear_of_fp_offset(
    module: &Module,
    layout: &DataLayout,
    function: &Function,
    list: ValueId,
    offset: u64,
) -> bool {
    function.users(list).iter().all(|one| {
        let user = function.instruction(one.user);
        let index = one.index as usize;
        match &user.opcode {
            Opcode::Load { .. } => {
                index == 0 && !overlaps_fp_offset(offset, layout.alloc_size(&module.context.types, user.ty))
            }
            Opcode::Store { .. } => {
                // The list's address as the stored value escapes.
                index == 1
                    && function
                        .operand_type(&module.context, user.operands[0])
                        .is_some_and(|ty| !overlaps_fp_offset(offset, layout.alloc_size(&module.context.types, ty)))
            }
            Opcode::GetElementPtr { .. } => {
                let Some(result) = user.result else { return false };
                let steps = &user.operands[1..];
                // `getelementptr i8, ptr, K` or an all-zero array decay.
                let moved = match steps {
                    [only] => constant_index(module, *only).filter(|_| {
                        matches!(
                            &user.opcode,
                            Opcode::GetElementPtr { source } if module.context.types.int_bits(*source) == Some(8)
                        )
                    }),
                    _ => steps.iter().all(|step| constant_index(module, *step) == Some(0)).then_some(0),
                };
                index == 0
                    && moved.is_some_and(|by| {
                        by < 1 << 16 && clear_of_fp_offset(module, layout, function, result, offset + by as u64)
                    })
            }
            // `va_start(ap)`, `va_end(ap)`; `lifetime.start(size, ap)`.
            Opcode::Call(_) => match intrinsic(module, function, one.user) {
                Some(Intrinsic::VaStart | Intrinsic::VaEnd) => index == 0,
                Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd) => index == 1,
                _ => false,
            },
            _ => false,
        }
    })
}

/// Whether no code of `function` can read the vector registers `va_start`
/// leaves in the save area: every `va_list` it starts is its own slot and is
/// used clear of `fp_offset`.
pub(super) fn leaves_vector_save_unread(
    module: &Module,
    layout: &DataLayout,
    function: &Function,
) -> bool {
    function
        .walk()
        .all(
            |(_, inst)| match intrinsic(module, function, inst) {
                Some(Intrinsic::VaCopy) => false,
                Some(Intrinsic::VaStart) => {
                    let Some(&Operand::Value(list)) = function.instruction(inst).operands.first() else { return false };
                    matches!(
                        function.value(list).def,
                        ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. })
                    ) && clear_of_fp_offset(module, layout, function, list, 0)
                }
                _ => true,
            },
        )
}
