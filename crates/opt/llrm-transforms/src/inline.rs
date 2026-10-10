//! Adapted from llrm-core's `optimize/inline.rs`, the port of
//! `qbopt/optimize/inline.py`: selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameters to the actual
//! operands, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! A callee is a candidate when cloning it is safe; whether it is worth it is
//! cost: the body's priced work copied against the calls it removes, in the
//! target's clocks, or its code bytes at -Os, bounded by the `Threshold`
//! (nominal: a body's budget is at most 24 operations).  A
//! body that only reads or writes through its arguments or its own frame
//! inlines like any other: the clone keeps its memory operations and the
//! ordinary body pipeline turns the caller's argument cells into values.  A
//! public callee is inlined at its sites and stays defined; a private one
//! called nowhere else goes with its last site.  MIR chooses from semantic
//! costs and never sees opcodes or registers.  The call's price is profit's
//! `OperationCosts::call`.
//!
//! What changed with the IR: a call names its callee and carries its
//! actuals, a formal is a parameter value, and the call's one result is what
//! `ret` returns, so the old call-site and ARG tables, formal entry cells
//! (`_parameter`), materialized actual copies, unmodelled extra call results
//! and width checks have no counterpart; the function type decides whether a
//! call fits its callee.  A candidate is a snapshot of its callee, as the old
//! one held the body it was chosen from.  A clone keeps its original's
//! metadata, which replaces merging the pointer and range side tables.  The
//! old `sealed` flag is a definition; a clone keeps its memory operations,
//! which replaces admitting only formal loads.  The clone is llrm-mir's
//! `splice`.  SSA is checked by the verifier
//! after each pass, not here.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_analysis::cfg;
use llrm_analysis::consts;
use llrm_analysis::memory::Unit;
use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::{ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::facts::{Facts, Inlining};
use llrm_mir::memory::{Callees, Effects, callee};
use llrm_mir::module::{Function, InstId, Linkage, Module, Operand, ValueDef};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::splice::{carries, splice, splice_before};
use llrm_mir::types::Type;
use llrm_support::hash::{IndexMap, SparseIdMap};

use crate::profit::{self, OperationCosts, operation};

/// How much inlining may copy: LLVM's inline threshold, 225 at -O2 and 0
/// for none. A callee's budget, in semantic operations, scales with it.
/// `hint` is the ratio a routine the language marks worth inlining may grow
/// by: LLVM's inline-hint threshold, 325 against 225; 1/1 where code size
/// outranks speed. `hot` is the same for a call in a loop: LLVM's
/// locally-hot call site threshold, 525 against 225.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Threshold {
    pub limit: i64,
    pub hint: (i64, i64),
    pub hot: (i64, i64),
    /// Where code size outranks speed: a call also removes its arguments'
    /// pushes and cleanup.
    pub single: bool,
    /// The last call of a function nothing else reaches inlines at any size:
    /// its body moves, nothing is copied: LLVM's last-call-to-static bonus
    /// and GCC's `-finline-functions-called-once`, which
    /// `-fno-inline-functions` leaves on as GCC's does;
    /// `-fno-inline-functions-called-once` turns it off.
    pub last: bool,
    /// `-fipa-cp-clone` (-O3): a function is copied for the constants its
    /// callers pass though the unit grows (`ipacp`).
    pub cp_clone: bool,
}

impl Threshold {
    pub fn new(limit: i64) -> Self {
        Self { limit, hint: (325, 225), hot: (525, 225), single: false, last: true, cp_clone: false }
    }

    /// Nothing inlines, the last call of a function included.
    pub fn none() -> Self {
        Self { last: false, ..Self::new(0) }
    }

    /// The same where code size outranks speed: a hint or a loop buys nothing.
    pub fn for_size(self) -> Self {
        Self { hint: (1, 1), hot: (1, 1), single: true, ..self }
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self::new(225)
    }
}

impl Threshold {
    /// The budget for a call priced `call_cost`; None when nothing inlines.
    pub(crate) fn budget(
        self,
        call_cost: i64,
    ) -> Option<i64> {
        (self.limit > 0).then(|| 6.max(24.min(call_cost.div_euclid(2))) * self.limit / Self::default().limit)
    }

    /// The same for a call a function makes to itself:
    /// `want_inline_small_function_p` is asked of the recursive edge too,
    /// so the growth the function may have is `max-inline-insns-auto`, 15
    /// at -O2 (params.opt:545), where a call priced at 12 or less admits
    /// six operations. `rectwo`'s body is nine.
    pub(crate) fn recursive_budget(
        self,
        call_cost: i64,
    ) -> Option<i64> {
        // -O1 has no `-finline-functions`: its threshold keeps the ordinary
        // budget.
        let floor =
            if self.limit < Self::default().limit { 0 } else { INSNS_AUTO * self.limit / Self::default().limit };
        self.budget(call_cost).map(|budget| budget.max(floor))
    }
}

/// GCC's `max-inline-insns-auto` at -O2 (params.opt:545).
const INSNS_AUTO: i64 = 15;

/// Direct call counts by callee.
pub type Counter = IndexMap<GlobalId, i64>;

/// What the language says of inlining `body`.
fn stated(body: &Function) -> Option<Inlining> {
    Facts::of(&body.attrs).inline()
}

/// The stack, in bytes, inlining may add to one function: a copy's frame is
/// a cell of its own wherever it sits, so copies add up, and in a recursive
/// function they add up per level.  LLVM bounds the same way.
const FRAME_LIMIT: u64 = 256;

/// Where the register allocator's cost leaves linear. gcc bounds a caller's
/// growth at `large-function-insns` (2700) and `large-function-growth` (100%,
/// ipa-inline.cc `caller_growth_limits`) and LLVM moves a once-called body up
/// to the last-call bonus (15000 over 5 a instruction, about 3000): both for
/// allocators near linear in function size. Ours rebuilds its intervals and
/// facts over the whole body at each spill and split, so a body merged past the
/// knee costs several times what its parts did. Measured (compile-time's curve,
/// QCport -O2, 732 functions, instrument commit 646b62f0 on perf/walk, data in
/// ~/scratch/ctime-out/curve): backend milliseconds per LIR instruction 0.20 up
/// to about 200 instructions, 0.45 at 200-400, 0.96 at 400-800, 1.8 above 1600;
/// the log-log slope of time against size 1.25 below 300 instructions and 2.2
/// above. The knee is in LIR instructions and this counts MIR operations, which
/// are 1.86 LIR instructions each at the median (QCport's 108 functions of 60
/// operations or more at -O2, 16-bit; 1.66 over 15 on the 32-bit target), so
/// 250 / 1.8. Counted as the same number, sb_build's 38-operation callee went
/// into a caller of 190 and left 667 instructions where its parts were 175 and
/// 465: backend 0.36 s -> 2.4 s. Called-once inlining (#769) merged part_frame
/// from 435 to 1647 instructions: backend 271 ms -> 11,385 ms.
// Re-measure when the allocator's slot numbering lands:
// https://github.com/ali-mosavian/llrm/issues/794. Measured 2026-10-07.
const KNEE_INSTRUCTIONS: i64 = 250;
/// LIR instructions a MIR operation comes to, in percent: the median over
/// QCport's 108 functions of 60 operations or more at -O2 (186) and 15 on the
/// 32-bit target (166), taken as 180.
const INSTRUCTIONS_PER_OPERATION: i64 = 180;
/// The knee in the unit this counts, MIR operations.
const ALLOCATION_KNEE: i64 = KNEE_INSTRUCTIONS * 100 / INSTRUCTIONS_PER_OPERATION;

/// gcc's rule of `caller_growth_limits` with the knee for
/// `large-function-insns`: an inline that leaves its caller over
/// `ALLOCATION_KNEE` operations and over the larger of the caller's own size
/// (before any inlining) and the callee's grown by `LARGE_GROWTH` percent is
/// refused, the last call of a function included.
const LARGE_FUNCTION: i64 = ALLOCATION_KNEE;
const LARGE_GROWTH: i64 = 100;

/// A body that is moved into its one caller is at most the knee: gcc's limits
/// above let a large callee into a small caller whole, which is the merge that
/// costs.
const LAST_CALL_OPERATIONS: i64 = ALLOCATION_KNEE;

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub body: Rc<Function>,
    /// Bytes of stack the body allocates.
    pub frame: u64,
    /// Admitted only as the last call of a body nothing else reaches, which it
    /// moves whatever its price: not a copy the price allows.
    pub moved: bool,
}

/// What a caller says of itself that bounds what may be copied into it.
pub struct Caller<'a> {
    pub layout: &'a DataLayout,
    pub recursive: bool,
    /// The caller's operations before anything was inlined into it, 0 where
    /// that is not known: the growth its inlines may come to is measured
    /// against it (`LARGE_FUNCTION`).
    pub base: i64,
}

/// Whether `inst` does semantic work: not a phi, a jump or a return.
fn semantic(
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    match instruction.opcode {
        Opcode::Phi | Opcode::Ret => false,
        Opcode::Br => instruction.operands.len() != 1,
        _ => true,
    }
}

/// How many operations `body` does: gcc's size in insns, which its growth
/// limits count.
pub fn operations(body: &Function) -> i64 {
    semantic_count(body)
}

fn semantic_count(body: &Function) -> i64 {
    body.walk().filter(|&(_, inst)| semantic(body, inst)).count() as i64
}

/// The defined function `id`.
fn body(
    module: &Module,
    id: GlobalId,
) -> Option<&Function> {
    module.global(id).function().filter(|function| !function.is_declaration())
}

/// The `byval` aggregate of each parameter of `callee`, by position.
fn byval_types(callee: &Function) -> Vec<(usize, llrm_mir::types::TypeId)> {
    callee
        .parameter_attrs
        .iter()
        .enumerate()
        .filter_map(|(at, attrs)| {
            attrs
                .iter()
                .find_map(
                    |one| match one {
                        llrm_mir::Attribute::Type(name, ty) if name == "byval" => Some((at, *ty)),
                        _ => None,
                    },
                )
        })
        .collect()
}

/// A `byval` parameter is the callee's own copy: the pointer the call passes is
/// the caller's object, which the callee may write. LLVM's InlineFunction
/// (HandleByValArgument) passes the pointer on when the callee only reads
/// memory, and otherwise copies the object into a new alloca on the caller's
/// entry and passes that: the same, here, with `memcpy` declared as the module
/// needs it.
fn copy_byval_arguments(
    context: &mut Context,
    function: &mut Function,
    layout: &DataLayout,
    call: InstId,
    callee: &Function,
    declared: &mut Declared,
    anchor: Option<InstId>,
) -> Result<Option<InstId>, String> {
    let mut placed = None;
    if !llrm_mir::memory::stated(&callee.attrs).writes {
        return Ok(None);
    }
    for (at, aggregate) in byval_types(callee) {
        let source = function.instruction(call).operands[at];
        let space = match context.types.get(function.operand_type(context, source).ok_or("a typed byval argument")?) {
            Type::Pointer(space) => *space,
            _ => return Err("a byval argument that is no pointer".to_owned()),
        };
        let bytes = layout.alloc_size(&context.types, aggregate);
        let width = layout.pointer(space).index_bits;
        let (pointer, void, flag) = (context.types.ptr(0), context.types.void(), context.types.int(1));
        let entry = function.entry().ok_or("a caller with a body")?;
        let first = anchor.or_else(|| {
            function
                .block(entry)
                .instructions()
                .iter()
                .copied()
                .find(|&one| !matches!(function.instruction(one).opcode, Opcode::Alloca { .. }))
        });
        let alloca = function.create_instruction(
            Opcode::Alloca { allocated: aggregate, align: None, address_space: 0 },
            pointer,
            Vec::new(),
            Flags::default(),
            Some("byval"),
        );
        function.insert(alloca, first.map_or(Position::End(entry), Position::Before))?;
        placed.get_or_insert(alloca);
        let copy = Operand::Value(function.instruction(alloca).result.expect("a pointer"));
        let count_type = context.types.int(width);
        let length = Operand::Constant(context.int(count_type, i128::from(bytes)));
        let (memcpy, memcpy_type) = crate::fill::_copy(context, declared, crate::fill::How::Apart, 0, space, width);
        let callee_pointer = Operand::Constant(
            context.constant(llrm_mir::context::Constant { ty: pointer, kind: ConstantKind::Global(memcpy) }),
        );
        let info = CallInfo {
            function_type: memcpy_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 4],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let off = Operand::Constant(context.int(flag, 0));
        let made = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![copy, source, length, off, callee_pointer],
            Flags::default(),
            None,
        );
        function.insert(made, Position::Before(call))?;
        let mut operands = function.instruction(call).operands.clone();
        operands[at] = copy;
        function.set_operands(call, operands);
    }
    Ok(placed)
}

/// Bytes of stack `function` allocates.
fn frame(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
) -> u64 {
    function
        .walk()
        .filter_map(|(_, inst)| {
            if let Opcode::Alloca { allocated, .. } = function.instruction(inst).opcode {
                Some(layout.alloc_size(&context.types, allocated))
            } else {
                None
            }
        })
        .sum()
}

/// The stack a copy of `body` adds to a caller: its allocas, and a copy of each
/// `byval` object it may write.
fn grown(
    context: &Context,
    layout: &DataLayout,
    body: &Function,
) -> u64 {
    let copies: u64 = if llrm_mir::memory::stated(&body.attrs).writes {
        byval_types(body).iter().map(|&(_, ty)| layout.alloc_size(&context.types, ty)).sum()
    } else {
        0
    };
    frame(context, layout, body) + copies
}

/// The functions that call themselves, directly or not.
pub fn recursive(module: &Module) -> BTreeSet<GlobalId> {
    let graph = CallGraph::new(module);
    module.functions().map(|(id, _, _)| id).filter(|&id| graph.recursive(id)).collect()
}

/// Whether `body` may be cloned into another function: it returns, `splice`
/// carries it, and it is not recursive, never to be inlined or `setjmp`-like.
pub(crate) fn cloneable(
    module: &Module,
    recursive: &BTreeSet<GlobalId>,
    id: GlobalId,
    body: &Function,
) -> bool {
    !recursive.contains(&id) && copyable(module, body)
}

/// Whether `body` may be copied as a function of its own (a recursive one
/// included): it returns, `splice` carries it, and it is not to be inlined or
/// `setjmp`-like.
pub(crate) fn copyable(
    module: &Module,
    body: &Function,
) -> bool {
    !body.is_declaration()
        && carries(body)
        && stated(body) != Some(Inlining::Never)
        && body.walk().any(|(_, inst)| body.instruction(inst).opcode == Opcode::Ret)
        && !llrm_mir::memory::calls_returns_twice(module, body)
}

/// The priced work `body` does once, unless something in it is unpriced.
fn work(
    module: &Module,
    body: &Function,
    callees: &Callees,
    costs: &OperationCosts,
) -> Option<i64> {
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(DataLayout::default, |text| DataLayout::parse(text).unwrap_or_default());
    body.walk()
        .filter(|&(_, inst)| semantic(body, inst))
        .map(|(_, inst)| operation(&module.context, &layout, body, callees, inst, costs))
        .sum()
}

/// What `function` comes to, priced by `costs`: its work, and each call's
/// arguments.
pub fn size(
    module: &Module,
    callees: &Callees,
    function: GlobalId,
    costs: &OperationCosts,
) -> Option<i64> {
    size_of(module, callees, module.global(function).function()?, costs)
}

/// `size` of a body that need not be in the module: a copy specialised for
/// the constants a site passes (`interprocedural::Specialisations`).
pub fn size_of(
    module: &Module,
    callees: &Callees,
    body: &Function,
    costs: &OperationCosts,
) -> Option<i64> {
    let calls: i64 = body
        .walk()
        .filter(|&(_, inst)| matches!(body.instruction(inst).opcode, Opcode::Call(_)))
        .map(|(_, inst)| (body.instruction(inst).operands.len() as i64 - 1).max(0) * costs.argument)
        .sum();
    // What a call keeps live across it is stored to the frame and read back:
    // the callee may use every register but two, which a body with no call
    // has for itself.
    let found = llrm_analysis::liveness::live(body);
    let kept: i64 = body
        .layout()
        .iter()
        .flat_map(|&block| llrm_analysis::liveness::live_points(body, &found, block))
        // An intrinsic, an inline block, is code in line: it keeps every
        // register but those it names.
        .filter(|point| {
            matches!(body.instruction(point.inst).opcode, Opcode::Call(_))
                && !callee(&module.context, body, point.inst).is_some_and(|id| {
                    module
                        .global(id)
                        .name
                        .as_deref()
                        .is_some_and(|name| name.starts_with("llvm.") || name.starts_with("llrm."))
                })
        })
        .map(|point| point.across.len() as i64)
        .sum::<i64>()
        * costs.store;
    work(module, body, callees, costs).map(|work| work + calls + kept)
}

/// The priced work of `body` that its known actuals fold away: what a copy at
/// such a site no longer does. Instructions whose inputs are all known, and
/// branches they decide; a lower bound, as control flow past a decided branch
/// is not followed.
pub(crate) fn folded(
    module: &Module,
    layout: &DataLayout,
    body: &Function,
    known: &[Option<ConstantId>],
    callees: &Callees,
    costs: &OperationCosts,
) -> i64 {
    let unit = Unit::of(module, layout, body);
    let mut values = IndexMap::default();
    for (&parameter, constant) in body.parameters().iter().zip(known) {
        if let Some(number) = constant.and_then(|id| consts::_operand(&unit, Operand::Constant(id), &values, None)) {
            values.insert(parameter, number);
        }
    }
    let mut saved = 0;
    for (_, inst) in body.walk().filter(|&(_, inst)| semantic(body, inst)) {
        let instruction = body.instruction(inst);
        let decided = instruction.opcode == Opcode::Br
            && consts::_operand(&unit, instruction.operands[0], &values, None).is_some();
        let result = consts::_result(&unit, inst, &values, None);
        if !decided && result.is_none() {
            continue;
        }
        if let (Some(value), Some(number)) = (consts::_defined(&unit, inst), result) {
            values.insert(value, number);
        }
        saved += operation(&module.context, layout, body, callees, inst, costs).unwrap_or(0);
    }
    saved
}

/// What a call costs beyond its own instruction, which inlining saves: the
/// return, and each argument pushed by the caller and read back by the callee.
pub fn call_overhead(
    costs: &OperationCosts,
    arguments: usize,
) -> i64 {
    costs.call + costs.return_ + arguments as i64 * costs.argument
}

/// Functions worth moving into their direct callers.
///
/// A candidate's size is bounded by the `Threshold`; a routine the language
/// says is `Always` inlined is a candidate at any size, and `Never` is none.
/// Inlining it at all
/// `count` sites leaves the body behind when it is public or its address is
/// taken; a private one goes with the last site, so one site costs nothing.
/// Each copy beyond that duplicates the body's priced work, which has to
/// stay below the calls removed.
pub fn candidates(
    module: &Module,
    callees: &Callees,
    layout: &DataLayout,
    calls: &Counter,
    private: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    reach: i64,
    threshold: Threshold,
) -> IndexMap<GlobalId, Candidate> {
    candidates_over(
        module,
        callees,
        layout,
        calls,
        private,
        costs,
        reach,
        threshold,
        &recursive(module),
        &llrm_mir::callgraph::addressed(module),
    )
}

/// `candidates` where the module's recursive and addressed functions are known:
/// a round asks twice (the bytes and the clocks) and found them each time.
pub fn candidates_over(
    module: &Module,
    callees: &Callees,
    layout: &DataLayout,
    calls: &Counter,
    private: &BTreeSet<GlobalId>,
    costs: &OperationCosts,
    reach: i64,
    threshold: Threshold,
    recursive: &BTreeSet<GlobalId>,
    addressed: &BTreeSet<GlobalId>,
) -> IndexMap<GlobalId, Candidate> {
    let budget = threshold.budget(reach);
    let call_cost = costs.call;
    let mut out = IndexMap::default();
    let mut lasts = IndexMap::default();
    for (&name, &count) in calls {
        let Some(body) = body(module, name) else { continue };
        if count == 0 || !cloneable(module, &recursive, name, body) {
            continue;
        }
        let always = stated(body) == Some(Inlining::Always);
        // A hint is worth a larger body, and a larger duplication, by LLVM's
        // ratio.
        let scale =
            |n: i64| if stated(body) == Some(Inlining::Hint) { n * threshold.hint.0 / threshold.hint.1 } else { n };
        // What a call removes: it, and where code size is what counts, its
        // arguments' pushes and cleanup. A `byval` argument's copy goes
        // too, as LLVM counts it (InlineCost: the bytes copied): a push's price
        // per word.
        let copied: i64 = body
            .parameter_attrs
            .iter()
            .flatten()
            .filter_map(|one| match one {
                llrm_mir::Attribute::Type(kind, ty) if kind == "byval" => {
                    Some(layout.alloc_size(&module.context.types, *ty) as i64 / 4)
                }
                _ => None,
            })
            .sum();
        let saved = call_cost
            + copied * costs.argument
            + if threshold.single { module.signature(body.ty).1.len() as i64 * costs.argument } else { 0 };
        let copies = if private.contains(&name) && !addressed.contains(&name) { count - 1 } else { count };
        // The last call of a function nothing else reaches moves its body: no
        // copy, and the call, its arguments and the return gone (LLVM's
        // last-call-to-static bonus).
        let admitted = || {
            budget.is_some_and(|budget| semantic_count(body) <= scale(budget))
                && (copies == 0
                    || work(module, body, callees, costs).is_some_and(|work| work * copies < scale(count * saved)))
        };
        // Only once nothing else is: a body that a call in it is about to be
        // inlined into would be copied with that call still in it, and
        // the call's callee counted once too many.
        let last =
            threshold.last && copies == 0 && semantic_count(body) <= LAST_CALL_OPERATIONS && !always && !admitted();
        // A body held only to inline from (`available_externally`) is a
        // candidate at any size: it is emitted nowhere, so a copy costs
        // nothing the program had.
        let verdict = always || admitted() || module.global(name).linkage == Linkage::AvailableExternally;
        llrm_support::debug!(
            "inline",
            "{} x{count} ({copies} copies): {} ops, budget {budget:?}, work {:?}, call {call_cost}: {}",
            module.global(name).name.as_deref().unwrap_or("?"),
            semantic_count(body),
            work(module, body, callees, costs),
            if verdict { "candidate" } else { "refused" }
        );
        if verdict {
            out.insert(
                name,
                Candidate { body: Rc::new(body.clone()), frame: grown(&module.context, layout, body), moved: false },
            );
        } else if last {
            lasts.insert(
                name,
                Candidate { body: Rc::new(body.clone()), frame: grown(&module.context, layout, body), moved: true },
            );
        }
    }
    if out.is_empty() {
        out = lasts;
    }
    out
}

/// Functions worth cloning at one call site whose actual is a known
/// constant; `recursive` is `recursive(module)`.
///
/// Whole-body parameter specialization needs every caller to agree.  This
/// narrower policy instead admits a call whose known actual exposes local
/// SCCP after the normal MIR clone.  The original body remains for dynamic
/// callers, so no source-level calling convention or symbol changes.
/// The work a copy keeps, priced in the target's clocks after what the known
/// actuals fold, must stay below the call it replaces.
pub fn constant_sites(
    module: &Module,
    callees: &Callees,
    layout: &DataLayout,
    recursive: &BTreeSet<GlobalId>,
    caller: &Function,
    constants: &IndexMap<InstId, Vec<Option<ConstantId>>>,
    costs: &OperationCosts,
    reach: i64,
    threshold: Threshold,
) -> IndexMap<InstId, Candidate> {
    let Some(budget) = threshold.budget(reach) else { return IndexMap::default() };
    let frequency =
        profit::_frequencies(&module.context, &module.metadata, &module.globals, caller, None).unwrap_or_default();
    let mut out = IndexMap::default();
    for (block, at) in caller.walk() {
        let Some(name) = callee(&module.context, caller, at) else { continue };
        let known = constants.get(&at).map_or(&[][..], Vec::as_slice);
        if !known.iter().any(Option::is_some) {
            continue;
        }
        let Some(body) = body(module, name) else { continue };
        let semantic = semantic_count(body);
        // What stays of the copy, in clocks, against the call it replaces.
        // Where code size is what counts, a body of arithmetic every actual of
        // which is known is taken to fold whole: `folded` follows no
        // branch past a decided one, so a loop on known bounds looked
        // all kept.
        let folds = threshold.single
            && known.iter().all(Option::is_some)
            && callees.get(&name).is_some_and(|summary| summary.effects == Effects::NONE);
        let saved = if folds { None } else { Some(folded(module, layout, body, known, callees, costs)) };
        let kept = if folds { Some(0) } else { work(module, body, callees, costs).map(|all| all - saved.unwrap_or(0)) };
        // A call in a loop saves its overhead on every trip, which LLVM's
        // hot-site threshold weighs.
        let hot = frequency.get(&cfg::id(block)).is_some_and(|&one| one > profit::UNIT);
        let overhead = call_overhead(costs, known.len()) * if hot { threshold.hot.0 } else { 1 }
            / if hot { threshold.hot.1 } else { 1 };
        // A copy that folds nothing buys the call's overhead once, which
        // `candidates` prices by its copies, unless the site is in a
        // loop and buys it every trip.
        let verdict = (folds || hot || saved.is_some_and(|saved| saved > 0))
            && kept.is_some_and(|kept| kept <= overhead)
            && semantic <= budget
            && cloneable(module, recursive, name, body);
        llrm_support::debug!(
            "inline",
            "constant site of {}: {semantic} ops, {kept:?} clocks kept, budget {budget}, call {overhead}, {} of {} actuals known: {}",
            module.global(name).name.as_deref().unwrap_or("?"),
            known.iter().flatten().count(),
            known.len(),
            if verdict { "candidate" } else { "refused" }
        );
        if verdict {
            out.insert(
                at,
                Candidate { body: Rc::new(body.clone()), frame: grown(&module.context, layout, body), moved: false },
            );
        }
    }
    out
}

/// Surviving direct call counts.
pub fn call_counts(module: &Module) -> Counter {
    let mut counts = Counter::default();
    for (_, _, function) in module.functions() {
        for (_, inst) in function.walk() {
            if matches!(function.instruction(inst).opcode, Opcode::Call(_))
                && let Some(name) = callee(&module.context, function, inst)
                && body(module, name).is_some()
            {
                *counts.entry(name).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Inline the first legal call site in `function`; whether one was.
pub fn expanded(
    context: &mut Context,
    function: &mut Function,
    caller: &Caller,
    available: &IndexMap<GlobalId, Candidate>,
    constant: Option<&IndexMap<InstId, Candidate>>,
    declared: &mut Declared,
) -> Result<bool, String> {
    let empty = IndexMap::default();
    let constant = constant.unwrap_or(&empty);
    let calls = function.walk().map(|(_, inst)| inst).collect::<Vec<_>>();
    let own = semantic_count(function);
    for call in calls {
        if !matches!(function.instruction(call).opcode, Opcode::Call(_)) {
            continue;
        }
        let candidate =
            constant.get(&call).or_else(|| callee(context, function, call).and_then(|name| available.get(&name)));
        let Some(candidate) = candidate else {
            continue;
        };
        if fits(context, function, caller, own, call, candidate) {
            copy_byval_arguments(context, function, caller.layout, call, &candidate.body, declared, None)?;
            splice(context, function, call, &candidate.body);
            return Ok(true);
        }
    }
    Ok(false)
}

/// What a call is told of one actual: the function it names, and whether it
/// is stack or global memory the caller owns.
#[derive(Clone, Copy, Default)]
struct Actual {
    callee: Option<GlobalId>,
    owned: bool,
}

/// How the operands of a body read once it is a copy: its parameters are the
/// actuals of the call it replaces, and the calls inside it that are inlined
/// are what they return (`expanded` replaced their results as it went).
struct Scope<'a> {
    function: &'a Function,
    actuals: Option<&'a [Actual]>,
    results: &'a SparseIdMap<InstId, Actual>,
}

impl Scope<'_> {
    fn actual(
        &self,
        context: &Context,
        operand: Operand,
        depth: usize,
    ) -> Actual {
        match operand {
            Operand::Constant(id) => match context.get(id).kind {
                ConstantKind::Global(global) => Actual { callee: Some(global), owned: true },
                _ => Actual::default(),
            },
            Operand::Value(value) => match self.function.value(value).def {
                ValueDef::Argument(at) => {
                    self.actuals.map_or(Actual { callee: None, owned: true }, |one| one[at as usize])
                }
                ValueDef::Instruction(inst) => match self.results.get(&inst) {
                    Some(&result) => result,
                    None => match self.function.instruction(inst).opcode {
                        Opcode::Alloca { .. } => Actual { callee: None, owned: true },
                        Opcode::GetElementPtr { .. } if depth < 8 => Actual {
                            callee: None,
                            owned: self.actual(context, self.function.instruction(inst).operands[0], depth + 1).owned,
                        },
                        _ => Actual::default(),
                    },
                },
            },
            Operand::Block(_) => Actual::default(),
        }
    }
}

/// The calls that are inlined into a copy, by their place among the copy's
/// instructions, each with what is inlined into it.
struct Planned<'a> {
    /// Where it comes among the decisions, which are taken in the order
    /// `expanded` would splice.
    seq: usize,
    candidate: &'a Candidate,
    nested: Vec<(usize, Planned<'a>)>,
}

/// The decisions of `expanded` called until it says no, taken ahead of the
/// splices: the size and the stack as they grow, and what each call is told.
struct Plan<'a, 'c> {
    context: &'c Context,
    caller: &'c Caller<'c>,
    available: &'a IndexMap<GlobalId, Candidate>,
    own: i64,
    frame: u64,
    seq: usize,
}

impl<'a> Plan<'a, '_> {
    /// `candidate` inlined at a call of `function_type` with `actuals`: the
    /// size and stack it adds are kept, and what is inlined in the copy.
    fn inline(
        &mut self,
        function_type: llrm_mir::types::TypeId,
        actuals: &[Actual],
        candidate: &'a Candidate,
    ) -> Option<(Planned<'a>, Actual)> {
        if !fits_with(self.context, self.caller, self.own, self.frame, function_type, |at| actuals[at].owned, candidate)
        {
            return None;
        }
        let callee = &*candidate.body;
        self.seq += 1;
        let seq = self.seq;
        let mut actuals = actuals.to_vec();
        let copied = copied_byval(callee);
        for &(at, _) in &copied {
            actuals[at] = Actual { callee: None, owned: true };
        }
        self.own += copied.len() as i64 * 2 + semantic_count(callee) - 1;
        self.frame += candidate.frame;
        let mut results = SparseIdMap::default();
        let mut nested = Vec::new();
        let mut place = 0;
        for (_, inst) in callee.walk() {
            let instruction = callee.instruction(inst);
            if instruction.opcode == Opcode::Ret {
                continue;
            }
            if let Opcode::Call(info) = &instruction.opcode {
                let operands = &instruction.operands;
                let seen: Vec<Actual> = operands
                    .iter()
                    .map(|&one| {
                        Scope { function: callee, actuals: Some(&actuals), results: &results }.actual(
                            self.context,
                            one,
                            0,
                        )
                    })
                    .collect();
                let target = seen.last().and_then(|one| one.callee).and_then(|name| self.available.get(&name));
                if let Some(target) = target
                    && let Some((planned, result)) = self.inline(info.function_type, &seen, target)
                {
                    if instruction.result.is_some() {
                        results.insert(inst, result);
                    }
                    nested.push((place, planned));
                }
            }
            place += 1;
        }
        let returns: Vec<&llrm_mir::module::Instruction> =
            callee.walk().map(|(_, inst)| callee.instruction(inst)).filter(|one| one.opcode == Opcode::Ret).collect();
        let result = match returns[..] {
            [one] if !one.operands.is_empty() => Scope { function: callee, actuals: Some(&actuals), results: &results }
                .actual(self.context, one.operands[0], 0),
            _ => Actual::default(),
        };
        Some((Planned { seq, candidate, nested }, result))
    }
}

/// The byval arguments `copy_byval_arguments` copies for a call of `callee`
/// (their places): their bytes are in the candidate's frame.
fn copied_byval(callee: &Function) -> Vec<(usize, llrm_mir::types::TypeId)> {
    if llrm_mir::memory::stated(&callee.attrs).writes { byval_types(callee) } else { Vec::new() }
}

/// The static allocas the copies made so far put on the caller's entry, by the
/// place of their site among the decisions. Made in order, each copy's go
/// before those of the copies made before it (the stack objects of a byval
/// argument after them): made from the last to the first, each goes
/// where the copies of its neighbours say, and the frame comes out as made in
/// order.
#[derive(Default)]
struct Placed {
    /// The first instruction of the entry before any copy: the allocas go
    /// before it.
    base: Option<InstId>,
    stack: std::collections::BTreeMap<usize, InstId>,
    copies: std::collections::BTreeMap<usize, InstId>,
}

/// `planned` at `call`, then what is inlined into the copy.
fn executed(
    context: &mut Context,
    function: &mut Function,
    layout: &DataLayout,
    call: InstId,
    planned: &Planned,
    declared: &mut Declared,
    placed: &mut Placed,
) -> Result<(), String> {
    let seq = planned.seq;
    let after = placed.copies.range(seq + 1..).next().map(|(_, &one)| one);
    if let Some(copy) = copy_byval_arguments(context, function, layout, call, &planned.candidate.body, declared, after)?
    {
        placed.copies.insert(seq, copy);
    }
    let anchor = placed.stack.range(..seq).next_back().map(|(_, &one)| one).or(placed.base);
    let copies = splice_before(context, function, call, &planned.candidate.body, anchor);
    let body = &*planned.candidate.body;
    let entry = body.entry();
    let first = body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret).zip(&copies).find(
        |&((block, inst), _)| Some(block) == entry && matches!(body.instruction(inst).opcode, Opcode::Alloca { .. }),
    );
    if let Some((_, &copy)) = first {
        placed.stack.insert(seq, copy);
    }
    for (place, nested) in planned.nested.iter().rev() {
        executed(context, function, layout, copies[*place], nested, declared, placed)?;
    }
    Ok(())
}

/// Every legal call site in `function` inlined, as `expanded` called until it
/// says no would: the first legal one, again from the top, a copy's own calls
/// ahead of the calls after it. The decisions are taken in that order with
/// the size and the stack kept as they grow (`expanded` counted the body
/// again for each site, a body of N sites N times), and the splices made
/// from the last site to the first, so that each splits a block whose tail
/// is only what is left of it (made in order, each moved the whole tail of
/// the one block a body of calls is: N sites N times its size). How many
/// sites were inlined.
pub fn expanded_all(
    context: &mut Context,
    function: &mut Function,
    caller: &Caller,
    available: &IndexMap<GlobalId, Candidate>,
    constant: Option<&SparseIdMap<InstId, Candidate>>,
    declared: &mut Declared,
) -> Result<usize, String> {
    let calls: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let start = (semantic_count(function), frame(context, caller.layout, function));
    let mut plan = Plan { context: &*context, caller, available, own: start.0, frame: start.1, seq: 0 };
    let mut results = SparseIdMap::default();
    let mut sites: Vec<(InstId, Planned)> = Vec::new();
    for call in calls {
        let instruction = function.instruction(call);
        let Opcode::Call(info) = &instruction.opcode else { continue };
        let scope = Scope { function, actuals: None, results: &results };
        let seen: Vec<Actual> = instruction.operands.iter().map(|&one| scope.actual(plan.context, one, 0)).collect();
        let target = constant
            .and_then(|sites| sites.get(&call))
            .or_else(|| seen.last().and_then(|one| one.callee).and_then(|name| available.get(&name)));
        let Some(target) = target else { continue };
        if let Some((planned, result)) = plan.inline(info.function_type, &seen, target) {
            if instruction.result.is_some() {
                results.insert(call, result);
            }
            sites.push((call, planned));
        }
    }
    let end = (plan.own, plan.frame);
    let mut placed = Placed {
        base: function.entry().and_then(|entry| function.block(entry).instructions().first().copied()),
        ..Placed::default()
    };
    for (call, planned) in sites.iter().rev() {
        executed(context, function, caller.layout, *call, planned, declared, &mut placed)?;
    }
    assert_eq!(end.0, semantic_count(function), "the size kept is not the body's");
    assert_eq!(end.1, frame(context, caller.layout, function), "the stack kept is not the body's");
    Ok(sites.len())
}

/// Whether the call fits its callee, which returns, and the stack the copy
/// adds stays within `FRAME_LIMIT`, and in a recursive function none; `own` is
/// the caller's size now.
fn fits(
    context: &Context,
    function: &Function,
    caller: &Caller,
    own: i64,
    call: InstId,
    candidate: &Candidate,
) -> bool {
    let Opcode::Call(info) = &function.instruction(call).opcode else { return false };
    let stack = if candidate.frame == 0 { 0 } else { frame(context, caller.layout, function) };
    fits_with(
        context,
        caller,
        own,
        stack,
        info.function_type,
        |at| owned(context, function, function.instruction(call).operands[at], 0),
        candidate,
    )
}

/// `fits` where the call is told what it is by `owned_at` and the caller's
/// stack is `stack` (only read of a candidate that allocates).
fn fits_with(
    context: &Context,
    caller: &Caller,
    own: i64,
    stack: u64,
    function_type: llrm_mir::types::TypeId,
    owned_at: impl Fn(usize) -> bool,
    candidate: &Candidate,
) -> bool {
    let callee = &*candidate.body;
    function_type == callee.ty
        && !matches!(
            context.types.get(callee.ty),
            Type::Function { variadic: true, .. }
        )
        && (candidate.frame == 0 || (!caller.recursive && stack + candidate.frame <= FRAME_LIMIT))
        && grows_within_limits(own, caller, callee, candidate.moved)
        && callee
            .parameters()
            .iter()
            .enumerate()
            .all(|(at, _)| !Facts::of(&callee.parameter_attrs[at]).releases() || owned_at(at))
}

/// gcc's `caller_growth_limits`: the size after the inline, against the
/// function limits. A caller whose size before is not known is its own base.
fn grows_within_limits(
    own: i64,
    caller: &Caller,
    callee: &Function,
    moved: bool,
) -> bool {
    let callee_size = semantic_count(callee);
    let base = if caller.base > 0 { caller.base } else { own };
    let limit = base.max(callee_size) * (100 + LARGE_GROWTH) / 100;
    let after = own + callee_size;
    // A body moved into a caller already past the knee is the merge that costs
    // most: that caller does not grow by one.
    if moved && after >= callee_size && after > ALLOCATION_KNEE && own > ALLOCATION_KNEE {
        return false;
    }
    !(after >= callee_size && after > LARGE_FUNCTION && after > limit)
}

/// Whether `operand` is an object the program owns: a variable, a frame object,
/// or a parameter, which the language passes owned. What a call returns, loads
/// or joins may be a runtime temporary, which the runtime frees where a routine
/// that `releases` it is called.
fn owned(
    context: &Context,
    function: &Function,
    operand: Operand,
    depth: usize,
) -> bool {
    match operand {
        Operand::Constant(id) => matches!(context.get(id).kind, ConstantKind::Global(_)),
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(inst) => match function.instruction(inst).opcode {
                Opcode::Alloca { .. } => true,
                Opcode::GetElementPtr { .. } if depth < 8 => {
                    owned(context, function, function.instruction(inst).operands[0], depth + 1)
                }
                _ => false,
            },
        },
        Operand::Block(_) => false,
    }
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;

/// GCC's `max-inline-recursive-depth-auto` (params.opt:573).
const RECURSIVE_DEPTH: u32 = 8;
/// GCC's `max-inline-insns-recursive-auto` (params.opt:553): what a function
/// may grow to by inlining itself.
const RECURSIVE_SIZE: i64 = 450;
/// GCC's `min-inline-recursive-probability` (params.opt:769), percent: a
/// recursive call is inlined into the function only if it runs more often than
/// this per call of it.
const RECURSIVE_PROBABILITY: i64 = 10;

/// GCC's `recursive_inlining` (ipa-inline.cc): `function`, the body of `id`,
/// with calls to itself replaced by copies of `original`, its body as it was,
/// breadth first and each copy's own calls in turn, while a call is likelier
/// than `RECURSIVE_PROBABILITY` percent of the function's calls, is no deeper
/// than `RECURSIVE_DEPTH`, and the function stays under `RECURSIVE_SIZE`; the
/// copies made. A function that allocates stack is left alone: each level would
/// add its frame.
pub fn inlined_into_itself(
    id: GlobalId,
    function: &mut Function,
    original: &Function,
    budget: i64,
    frequencies: &dyn Fn(&Context, &Function) -> std::collections::BTreeMap<i64, i64>,
    context: &mut Context,
) -> usize {
    let own = |function: &Function, context: &Context| -> Vec<InstId> {
        function.walk().map(|(_, inst)| inst).filter(|&inst| callee(context, function, inst) == Some(id)).collect()
    };
    // Only a body the ordinary inline threshold admits
    // (`want_inline_small_function_p` is asked of the recursive edge
    // too), by its growth: the body less the call it replaces.
    if semantic_count(original) - 1 > budget
        || original.walk().any(|(_, inst)| matches!(original.instruction(inst).opcode, Opcode::Alloca { .. }))
        || !carries(original)
    {
        return 0;
    }
    let mut depth: IndexMap<InstId, u32> = own(function, context).into_iter().map(|call| (call, 1)).collect();
    let mut made = 0;
    loop {
        let weights = frequencies(context, function);
        let next = own(function, context)
            .into_iter()
            .filter(|call| depth.get(call).copied().unwrap_or(1) <= RECURSIVE_DEPTH)
            .filter(|&call| {
                function
                    .parent(call)
                    .and_then(|block| weights.get(&cfg::id(block)))
                    .is_some_and(|&weight| weight * 100 > profit::UNIT * RECURSIVE_PROBABILITY)
            })
            .min_by_key(|call| depth.get(call).copied().unwrap_or(1));
        let Some(call) = next else { break };
        if semantic_count(function) + semantic_count(original) >= RECURSIVE_SIZE {
            break;
        }
        let at = depth.get(&call).copied().unwrap_or(1);
        let before: BTreeSet<InstId> = own(function, context).into_iter().collect();
        splice(context, function, call, original);
        made += 1;
        for new in own(function, context) {
            if !before.contains(&new) {
                depth.insert(new, at + 1);
            }
        }
    }
    made
}
