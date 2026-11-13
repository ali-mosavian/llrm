//! PartiallyInlineLibCalls: a call to `sqrt` or `sqrtf` that may set `errno`
//! becomes the target's square-root instruction, and the library call runs
//! only when the instruction cannot answer (a NaN, a negative operand)
//! ([PartiallyInlineLibCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/PartiallyInlineLibCalls.cpp)).
//!
//! # How it works
//! A call is a candidate when the library has the function, the target answers
//! `have_fast_sqrt` for its type, and the call is not `nobuiltin`, `strictfp`,
//! `musttail`, to a local function, or already read-only (the back end emits
//! the instruction for that one). The call is marked `memory(none)`; a branch
//! on `fcmp ord v, v` (`fcmp oge x, 0.0` where the target says that costs
//! less) skips a clone of the call, and a phi joins the two results. One call
//! per block: the rest of the block is the next block, visited next.
//!
//! ```text
//!  b:  %r = call double @sqrt(double %x)       b:  %r = call double @sqrt(double %x) memory(none)
//!      ...                                         %c = fcmp ord double %r, %r
//!                                                  br i1 %c, label %b.split, label %call.sqrt
//!                                              call.sqrt: %l = call double @sqrt(double %x)
//!                                                         br label %b.split
//!                                              b.split: %p = phi double [%r, %b], [%l, %call.sqrt]
//!                                                       ...
//! ```
//!
//! Why it matters here: a `sqrt` that sets `errno` is a call; with this it is
//! an instruction on the path that has no error.
//!
//! DEVIATE PIL-OS: where the target's `Tuning::partial_libcalls_skip_at_size`
//! is on (x86, arm64, wasm), a function with `optsize` or `minsize` is
//! skipped. The pin splits there too, which grows -Os code by a compare, a
//! branch and a copy of the call, and -Os is strict bytes. Owner: alim; exit
//! 2026-11-27, revisit when the -Os rows allow.
//!
//! DEVIATE PI2: MIR's text has no `strictfp`; the check reads the attribute
//! where a front end states it, and no test can state it.
//!
//! DEVIATE PI1: the likely-branch weight the pin adds when the function has an
//! entry count is not added: MIR has no profile counts.
//!
//! # Runs
//! After the strength slot (the pin: `addIRPasses`, after LSR), before the
//! code-generation-prepare passes.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::facts::Fact;
use llrm_mir::libfunc::LibFunc;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Attribute, Flags, FloatPredicate, Opcode, Tail};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::Ty;
use llrm_mir::types::{FloatKind, Type};

use crate::edges::split_block_and_insert_if_then;
use crate::lcssa::{arms, from_arms};
use crate::simplifylibcalls::target_library_info;

pub struct PartiallyInlineLibCalls;

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

impl FunctionPass for PartiallyInlineLibCalls {
    fn name(&self) -> &'static str {
        "partialinline"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        let for_size = unit
            .function
            .attrs
            .iter()
            .any(
                |one| matches!(
                    one,
                    Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
                ),
            );
        // `LLRM_PIL_SKIP_AT_SIZE` runs the per-target -Os A/B before a
        // description turns the field on.
        if for_size && (tti.partial_libcalls_skip_at_size() || llrm_support::env_flag!("LLRM_PIL_SKIP_AT_SIZE")) {
            return PreservedAnalyses::all();
        }
        let info = target_library_info(&outer, unit.layout.pointer(0).index_bits);
        let mut blocks: Vec<BlockId> = unit.function.layout().to_vec();
        let mut changed = false;
        let mut at = 0;
        while at < blocks.len() {
            let block = blocks[at];
            at += 1;
            let calls: Vec<InstId> = unit.function.iter_block(block).collect();
            for call in calls {
                let Opcode::Call(call_info) = &unit.function.instruction(call).opcode else { continue };
                let call_info = (**call_info).clone();
                if call_info.tail == Tail::MustTail {
                    continue;
                }
                let Some(callee) = llrm_mir::memory::callee(unit.context, unit.function, call) else { continue };
                let Some(global) = outer.globals.get(callee.0 as usize) else { continue };
                let callee_attrs: &[Attribute] = match &global.kind {
                    llrm_mir::module::GlobalKind::Function(function) => &function.attrs,
                    _ => &[],
                };
                // `isNoBuiltin` and `isStrictFP` read the call site and the
                // callee.
                let either = |name: &str| has_flag(&call_info.attrs, name) || has_flag(callee_attrs, name);
                if (either("nobuiltin") && !either("builtin")) || either("strictfp") {
                    continue;
                }
                if matches!(
                    global.linkage,
                    llrm_mir::module::Linkage::Internal | llrm_mir::module::Linkage::Private
                ) {
                    continue;
                }
                let Some(func) = info.get_lib_func_of(&unit.context.types, global) else { continue };
                if !info.has(func) || !matches!(func, LibFunc::Sqrt | LibFunc::Sqrtf) {
                    continue;
                }
                let ty = unit.function.instruction(call).ty;
                let (kind, tti_ty) = match unit.context.types.get(ty) {
                    Type::Float(FloatKind::Float) => (FloatKind::Float, Ty::Float),
                    Type::Float(FloatKind::Double) => (FloatKind::Double, Ty::Double),
                    _ => continue,
                };
                if !tti.have_fast_sqrt(tti_ty) {
                    continue;
                }
                // The back end emits the instruction for a call that touches
                // no memory already.
                let reads_only = !llrm_mir::memory::stated(&call_info.attrs).writes
                    || !llrm_mir::memory::stated(callee_attrs).writes;
                if reads_only {
                    continue;
                }
                let join = optimize_sqrt(unit, &*tti, call, kind, tti_ty);
                // The rest of the block is the next to visit; the library
                // call's block is not.
                blocks.insert(at, join);
                changed = true;
                break;
            }
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// `optimizeSQRT`: the join block, which holds what followed the call.
fn optimize_sqrt(
    unit: &mut Unit,
    tti: &dyn llrm_mir::tti::Tti,
    call: InstId,
    kind: FloatKind,
    tti_ty: Ty,
) -> BlockId {
    let ty = unit.function.instruction(call).ty;
    let i1 = unit.context.types.int(1);
    let head = unit.function.parent(call).expect("a placed call");
    let next = unit.function.iter_from(call).nth(1).expect("a terminated block");
    let always = Operand::Constant(unit.context.int(i1, 1));
    // `SplitBlockAndInsertIfThen`: the new block runs on `true`; the library
    // call goes on `false`, so the branch's arms swap.
    let (libcall_bb, join) = split_block_and_insert_if_then(unit.function, always, next);
    let branch = unit.function.terminator(head).expect("a branch");
    let name = unit.function.block(head).name.clone().unwrap_or_default();
    unit.function.set_block_name(join, &format!("{name}.split"));
    unit.function.set_block_name(libcall_bb, "call.sqrt");
    let result = unit.function.instruction(call).result.expect("a call with a result");
    let void = unit.function.instruction(branch).ty;
    let first = unit.function.iter_block(join).next().expect("a terminated block");
    let phi = unit.function.create_instruction(Opcode::Phi, ty, vec![], Flags::default(), None);
    unit.function.insert(phi, Position::Before(first)).expect("a placed phi");
    let phi_value = Operand::Value(unit.function.instruction(phi).result.expect("a phi has a result"));
    unit.function.replace_all_uses_with(result, phi_value);
    // The library call, a copy of this one.
    let libcall = unit.function.clone_instruction(call);
    let end = unit.function.terminator(libcall_bb).expect("a branch");
    unit.function.insert(libcall, Position::Before(end)).expect("a placed call");
    // This call no longer touches memory, so the back end may use the
    // instruction.
    if let Opcode::Call(info) = unit.function.instruction(call).opcode.clone() {
        let mut info = *info;
        // The facts that state what the call touches go by the fact table; the
        // older spellings of a narrower effect are not facts and go by
        // name.
        info.attrs.retain(|one| {
            !matches!(
                Fact::of_attribute(one),
                Some(Fact::Memory(_) | Fact::ReadNone | Fact::ReadOnly | Fact::WriteOnly)
            )
        });
        info.attrs.retain(|one| {
            !matches!(
                one,
                Attribute::Flag(flag) if matches!(flag.as_str(), "argmemonly" | "inaccessiblememonly")
            )
        });
        info.attrs.push(Attribute::Memory(vec![(None, "none".to_owned())]));
        unit.function.set_opcode(call, Opcode::Call(Box::new(info)));
    }
    let zero = Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::float_zero(kind) }));
    let argument = unit.function.instruction(call).operands[0];
    let (lhs, rhs, predicate) = if tti.is_fcmp_ord_cheaper_than_fcmp_zero(tti_ty) {
        (Operand::Value(result), Operand::Value(result), FloatPredicate::Ord)
    } else {
        (argument, zero, FloatPredicate::Oge)
    };
    let compare = unit.function.create_instruction(Opcode::FCmp(predicate), i1, vec![lhs, rhs], Flags::default(), None);
    unit.function.insert(compare, Position::Before(branch)).expect("a placed compare");
    let condition = Operand::Value(unit.function.instruction(compare).result.expect("a compare has a result"));
    let _ = void;
    unit.function.set_operands(branch, vec![condition, Operand::Block(join), Operand::Block(libcall_bb)]);
    let libcall_value = Operand::Value(unit.function.instruction(libcall).result.expect("a call with a result"));
    let incoming = [(Operand::Value(result), head), (libcall_value, libcall_bb)];
    let _ = arms;
    unit.function.set_operands(phi, from_arms(&incoming));
    join
}

#[cfg(test)]
#[path = "partialinline_tests.rs"]
mod tests;
