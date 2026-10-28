//! Whether a module forges pointers into the data segment: the one fact that
//! decides if a global can be "reached by name only" on a segmented target.
//!
//! # How it works
//! The pin's `GlobalsAAResult` (and every analysis that reads it) takes a
//! global whose address is never computed as unreachable by a pointer of
//! unknown origin: forging a pointer to it is undefined
//! ([`isNonEscapingGlobalNoAlias`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/GlobalsModRef.cpp#L813)
//! answers may-alias for a pointer from `inttoptr` only because it cannot
//! point there). Real-mode BASIC defines it: `DEF SEG = VARSEG(w)` and `POKE`
//! reach any byte of the data segment, a variable whose address nobody took
//! among them. A far pointer is forged where an `inttoptr` makes a selector
//! alone (the segment space the target description names) from an integer that
//! is not a plain number.
//!
//! A plain numeric selector (`&HB800`) is exempt: it names a segment no
//! program object occupies. The data segment's own value is a load-time
//! relocation, never a number a source can write, so a constant cannot name
//! it; a selector computed (`VARSEG`, arithmetic, a `PEEK` result) is not
//! exempt.
//!
//! A selector read from an array descriptor (an `inttoptr` the frontend marks
//! `!llrm.descriptor`) is exempt too: it is the array's own segment, which an
//! access in bounds never leaves.
//!
//! The runtime's own code is no forger of the program's: its bodies are
//! `available_externally`, and the runtime module is another module; both are
//! out of the scan, the same trust boundary as `!llrm.named`.
//!
//! # Example
//! `%s = add i16 %v, 4` then `%p = inttoptr i16 %s to ptr addrspace(2)` forges:
//! the selector is computed, and every global of the module is reachable by it.
//! `inttoptr i16 -18432 to ptr addrspace(2)` (`&HB800`) does not.
//!
//! # Provides
//! `forges_selector`; the manager entry `ForgedSelectors` computes it once for
//! a module, and `GlobalsAA` and `GlobalsModRef` take it from there.

use llrm_mir::context::{ConstantExpr, ConstantKind, Context};
use llrm_mir::module::{DESCRIPTOR_WORD, Function, Linkage, Module, Operand};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::spaces::Spaces;
use llrm_mir::types::Type;

/// Whether the integer `operand` is a plain number.
fn numeric(
    context: &Context,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(_))
    )
}

/// Whether making a pointer of type `to` from `operand` forges one into the
/// data segment: a selector made of an integer that is no plain number.
fn forges(
    context: &Context,
    spaces: Spaces,
    to: llrm_mir::types::TypeId,
    operand: Operand,
) -> bool {
    let Type::Pointer(space) = context.types.get(to) else { return false };
    spaces.is_segment(Some(*space)) && !numeric(context, operand)
}

/// Whether the frontend marks `instruction`, an `inttoptr`, as making an
/// array's own segment from a descriptor word.
fn described(instruction: &llrm_mir::module::Instruction) -> bool {
    instruction.metadata.iter().any(|(kind, _)| kind == DESCRIPTOR_WORD)
}

/// Whether `function` makes a pointer into the data segment from an integer.
fn forging_body(
    context: &Context,
    spaces: Spaces,
    function: &Function,
) -> bool {
    function
        .walk()
        .any(
            |(_, inst)| {
                let instruction = function.instruction(inst);
                if matches!(instruction.opcode, Opcode::Cast(CastOp::IntToPtr))
                    && forges(context, spaces, instruction.ty, instruction.operands[0])
                    && !described(instruction)
                {
                    return true;
                }
                // A constant expression `inttoptr (...)` an operand holds.
                instruction
                    .operands
                    .iter()
                    .any(
                        |operand| {
                            let Operand::Constant(id) = operand else { return false };
                            matches!(
                                &context.get(*id).kind,
                                ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value })
                                    if forges(context, spaces, context.get(*id).ty, Operand::Constant(*value))
                            )
                        },
                    )
            },
        )
}

/// Whether a body of the program's own in `module` forges a pointer into the
/// data segment. False on a flat target (no selector space).
pub fn forges_selector(
    module: &Module,
    spaces: Spaces,
) -> bool {
    spaces.segment.is_some()
        && module
            .functions()
            .filter(|(_, global, function)| {
                global.linkage != Linkage::AvailableExternally && !function.is_declaration()
            })
            .any(|(_, _, function)| forging_body(&module.context, spaces, function))
}
