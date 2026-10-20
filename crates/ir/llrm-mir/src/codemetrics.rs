//! `CodeMetrics`: the size of a block and what keeps it from being copied.
//!
//! # How it works
//! `analyze_basic_block` adds up the `CodeSize` cost of each instruction of a
//! block ([`instruction_cost`]), except the ephemeral ones, and notes
//! what it finds: a call that may be inlined, a dynamic `alloca`, a call that
//! cannot be duplicated or is convergent. `collect_ephemeral_values` finds the
//! values only an `llvm.assume` uses (the conditions assumed), which cost
//! nothing.
//!
//! Loop rotation reads `num_insts` against its header limit, and the unroll
//! cost estimate reads it for the body of the loop.
//!
//! # Deviations
//! - C1 No `noduplicate` check and no controlled convergence: MIR has neither
//!   attribute `noduplicate` nor convergence tokens (`ConvergenceControlInst`),
//!   so a convergent call is `Uncontrolled`. LLVM:
//!   [`cannotDuplicate`,
//!   `Convergence`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CodeMetrics.cpp#L158).
//! - C2 The assumptions come as a list of `llvm.assume` calls: llrm derives
//!   them from the body (`Assumptions`) and keeps no `AssumptionCache`.
//!
//! # Provides
//! [`CodeMetrics`], filled by `analyze_basic_block`.
//!
//! # See also
//! [`CodeMetrics::analyzeBasicBlock`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/CodeMetrics.cpp#L117).

use crate::context::Context;
use crate::context::GlobalId;
use crate::datalayout::DataLayout;
use crate::dense::IdSet;
use crate::instructioncost::instruction_cost;
use crate::module::{BlockId, Function, GlobalKind, GlobalValue, InstId, Linkage, Operand, ValueDef};
use crate::opcode::{Attribute, Opcode};
use crate::tti::{Cost, CostKind, Tti};
use crate::types::Type;

/// `ConvergenceKind`. The controlled kinds need convergence tokens, which MIR
/// does not have: a convergent call is uncontrolled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Convergence {
    #[default]
    None,
    Uncontrolled,
}

/// `CodeMetrics`.
#[derive(Clone, Debug)]
pub struct CodeMetrics {
    pub is_recursive: bool,
    pub not_duplicatable: bool,
    pub uses_dynamic_alloca: bool,
    pub convergence: Convergence,
    pub num_insts: Cost,
    pub num_inline_candidates: u32,
    pub num_vector_insts: u32,
    pub num_rets: u32,
}

impl Default for CodeMetrics {
    fn default() -> Self {
        Self {
            is_recursive: false,
            not_duplicatable: false,
            uses_dynamic_alloca: false,
            convergence: Convergence::None,
            num_insts: Cost::FREE,
            num_inline_candidates: 0,
            num_vector_insts: 0,
            num_rets: 0,
        }
    }
}

/// What a metrics query reads besides the block: the module's globals and
/// whether a function has a single live use.
pub struct Query<'a> {
    pub tti: &'a dyn Tti,
    pub context: &'a Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    /// The function holding the block, where it is a global: a call to it is
    /// recursion.
    pub this: Option<GlobalId>,
    /// `Function::hasOneLiveUse`.
    pub has_one_live_use: &'a dyn Fn(GlobalId) -> bool,
}

fn flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

/// `appendSpeculatableOperands`.
fn append_speculatable_operands(
    function: &Function,
    effects_free: &dyn Fn(InstId) -> bool,
    inst: InstId,
    visited: &mut IdSet<crate::module::ValueId>,
    worklist: &mut Vec<InstId>,
) {
    for &operand in &function.instruction(inst).operands {
        let Operand::Value(value) = operand else { continue };
        if !visited.insert(value) {
            continue;
        }
        if let ValueDef::Instruction(def) = function.value(value).def
            && effects_free(def)
            && !function.instruction(def).opcode.is_terminator()
        {
            worklist.push(def);
        }
    }
}

/// `completeEphemeralValues`.
fn complete_ephemeral_values(
    function: &Function,
    effects_free: &dyn Fn(InstId) -> bool,
    visited: &mut IdSet<crate::module::ValueId>,
    worklist: &mut Vec<InstId>,
    eph: &mut IdSet<InstId>,
) {
    // The worklist is walked by index and grows as it goes: a queue without
    // quadratic work.
    let mut at = 0;
    while at < worklist.len() {
        let inst = worklist[at];
        at += 1;
        // If all uses of this value are ephemeral, then so is this value.
        let Some(result) = function.instruction(inst).result else { continue };
        if !function.users(result).iter().all(|one| eph.contains(&one.user)) {
            continue;
        }
        eph.insert(inst);
        append_speculatable_operands(function, effects_free, inst, visited, worklist);
    }
}

/// `collectEphemeralValues(Loop)`: the values of `blocks` only an `assume`
/// of `blocks` uses, and the assumes. `assumes` are the `llvm.assume` calls of
/// the function (`AssumptionCache::assumptions`), the ones outside `blocks`
/// filtered here.
pub fn collect_ephemeral_values(
    function: &Function,
    effects_free: &dyn Fn(InstId) -> bool,
    assumes: &[InstId],
    in_loop: &dyn Fn(BlockId) -> bool,
) -> IdSet<InstId> {
    let mut eph = IdSet::<InstId>::new();
    let mut visited = IdSet::<crate::module::ValueId>::new();
    let mut worklist = Vec::new();
    for &assume in assumes {
        // Calls outside the loop are left out so a function's worth of work is
        // not done for each of its loops.
        if !function.parent(assume).is_some_and(in_loop) {
            continue;
        }
        if eph.insert(assume) {
            append_speculatable_operands(function, effects_free, assume, &mut visited, &mut worklist);
        }
    }
    complete_ephemeral_values(function, effects_free, &mut visited, &mut worklist, &mut eph);
    eph
}

impl CodeMetrics {
    /// `analyzeBasicBlock`.
    pub fn analyze_basic_block(
        &mut self,
        q: &Query,
        function: &Function,
        block: BlockId,
        eph: &IdSet<InstId>,
        prepare_for_lto: bool,
    ) {
        for inst in function.iter_block(block) {
            // Skip ephemeral values.
            if eph.contains(&inst) {
                continue;
            }
            let operation = function.instruction(inst);
            if let Opcode::Call(call) | Opcode::Invoke(call) = &operation.opcode {
                let callee = crate::memory::callee(q.context, function, inst);
                let no_inline = flag(&call.attrs, "noinline");
                // A callee a pass has declared and the module's globals do not
                // hold yet is an external call.
                if let Some((callee, global)) = callee.and_then(|id| q.globals.get(id.0 as usize).map(|one| (id, one)))
                {
                    // A function that is internal and has a single use is
                    // extremely likely to be inlined later.
                    let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
                    let name = global.name.as_deref();
                    let lowered =
                        !name.is_some_and(|name| name.starts_with("llvm.")) && q.tti.is_lowered_to_call(name, internal);
                    if !no_inline && lowered && ((internal && (q.has_one_live_use)(callee)) || prepare_for_lto) {
                        self.num_inline_candidates += 1;
                    }
                    // A call to the function itself: inlining it is a form of
                    // peeling that these metrics do not measure.
                    if Some(callee) == q.this {
                        self.is_recursive = true;
                    }
                } else if prepare_for_lto && !no_inline {
                    self.num_inline_candidates += 1;
                }
                let callee_attrs: &[Attribute] =
                    match callee.and_then(|id| q.globals.get(id.0 as usize)).map(|one| &one.kind) {
                        Some(GlobalKind::Function(function)) => &function.attrs,
                        _ => &[],
                    };
                if self.convergence == Convergence::None
                    && (flag(&call.attrs, "convergent") || flag(callee_attrs, "convergent"))
                {
                    self.convergence = Convergence::Uncontrolled;
                }
            }
            if let Opcode::Alloca { .. } = operation.opcode {
                let sized = operation.operands.first().is_none_or(|count| matches!(count, Operand::Constant(_)));
                let in_entry = function.layout().first().is_some_and(|&entry| function.parent(inst) == Some(entry));
                if !(sized && in_entry) {
                    self.uses_dynamic_alloca = true;
                }
            }
            if matches!(q.context.types.get(operation.ty), Type::Vector { .. }) {
                self.num_vector_insts += 1;
            }
            // A token used outside its block cannot be duplicated.
            if matches!(q.context.types.get(operation.ty), Type::Token)
                && operation.result.is_some_and(|value| {
                    function.users(value).iter().any(|one| function.parent(one.user) != Some(block))
                })
            {
                self.not_duplicatable = true;
            }
            self.num_insts = self.num_insts
                + instruction_cost(q.tti, q.context, q.layout, q.globals, function, inst, CostKind::CodeSize);
        }
        if function.terminator(block).is_some_and(|term| function.instruction(term).opcode == Opcode::Ret) {
            self.num_rets += 1;
        }
    }
}
