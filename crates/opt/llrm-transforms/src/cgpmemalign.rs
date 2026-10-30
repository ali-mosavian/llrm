//! The alignment of a memory intrinsic's pointers, raised to what is known of
//! them: `CodeGenPrepare::optimizeCallInst`, which sets the destination (and
//! the source of a transfer) `align` to `getKnownAlignment` where that is
//! larger than the call states.
//!
//! # How it works
//! Each `llvm.memcpy`, `llvm.memmove` and `llvm.memset` call has its pointer
//! arguments' alignment (`Value::getPointerAlignment`: a global's or alloca's
//! stated alignment, less what a constant GEP may add) compared with the
//! `align` attribute it carries (one when it has none). The larger is written
//! back. The pin runs the pass at -O1 and up only, so `-O0` keeps the call;
//! here the pipeline does not run at -O0 at all.
//!
//! # Example
//! `call void @llvm.memcpy.p0.p0.i32(ptr @b, ptr @a, i32 100, i1 false)` with
//! `@a` and `@b` declared `align 4` becomes the same call with `align 4` on
//! both pointers, so x86's `EmitTargetCodeForMemcpy` copies with `rep movsl`
//! and not a call of `memcpy`.
//!
//! # Runs
//! In `cgpaddr`, with the pass's other CodeGenPrepare rewrites.
//!
//! # See also
//! `backend::memcpycall` reads the alignment this states.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, GlobalValue, InstId};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::known::pointer_alignment;

/// Whether any memory intrinsic call of `function` was given a larger `align`.
pub fn raised(
    context: &Context,
    layout: &DataLayout,
    globals: &[GlobalValue],
    function: &mut Function,
) -> bool {
    let mut changes: Vec<(InstId, Vec<Vec<Attribute>>)> = Vec::new();
    {
        let function: &Function = function;
        let query = Query { globals, ..Query::bare(context, function, layout) };
        for (_, call) in function.walk() {
            let Some(callee) = llrm_mir::memory::callee(context, function, call) else { continue };
            let pointers = match globals.get(callee.0 as usize).and_then(GlobalValue::intrinsic) {
                Some(Intrinsic::MemCpy | Intrinsic::MemMove) => 2,
                Some(Intrinsic::MemSet) => 1,
                _ => continue,
            };
            let instruction = function.instruction(call);
            let Opcode::Call(info) = &instruction.opcode else { continue };
            let mut attrs = info.argument_attrs.clone();
            let mut changed = false;
            for index in 0..pointers {
                let known = pointer_alignment(&query, instruction.operands[index]);
                let stated = attrs
                    .get(index)
                    .into_iter()
                    .flatten()
                    .find_map(|attr| match attr {
                        Attribute::Int(name, value) if name == "align" => Some(*value),
                        _ => None,
                    })
                    .unwrap_or(1);
                if known <= stated {
                    continue;
                }
                if attrs.len() <= index {
                    attrs.resize(index + 1, Vec::new());
                }
                attrs[index].retain(|attr| !matches!(attr, Attribute::Int(name, _) if name == "align"));
                attrs[index].push(Attribute::Int("align".to_owned(), known));
                changed = true;
            }
            if changed {
                changes.push((call, attrs));
            }
        }
    }
    let any = !changes.is_empty();
    for (call, attrs) in changes {
        let Opcode::Call(info) = &function.instruction(call).opcode else { continue };
        let (call_attrs, return_attrs) = (info.attrs.clone(), info.return_attrs.clone());
        function.set_call_attributes(call, call_attrs, return_attrs, attrs);
    }
    any
}
