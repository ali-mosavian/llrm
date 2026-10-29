//! Whole-module MIR inlining: chooses the calls worth replacing by a copy of
//! the callee's body, and makes the copies. A call costs its own instructions
//! and hides the callee from the caller's passes; the copy saves the first and
//! exposes the second.
//!
//! # How it works
//! Inlining is a CFG operation: the caller's block is split at the call, the
//! callee's blocks are cloned in (`llrm_mir::splice`), formals become the
//! actual operands, and each `ret` joins the continuation. The body pipeline
//! then simplifies the result. A clone keeps its original's metadata and its
//! memory operations; a body that reads or writes through its arguments or its
//! own frame inlines like any other.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef note fill:transparent,stroke:transparent,color:#64748b
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   subgraph g_before["CALLER"]
//!     a1["<b>A</b>"]:::mid
//!     call_["<b>%r = call @g(%a, %b)</b>"]:::mid
//!     b1["<b>B</b>"]:::mid
//!   end
//!   g["g(%x, %y): P#59; ret %v"]:::note
//!   subgraph g_after["CALLER AFTER"]
//!     a2["<b>A</b>"]:::mid
//!     p["<b>P[%x:=%a, %y:=%b]</b>"]:::mid
//!     b2["<b>B</b>"]:::mid
//!   end
//!   a1 --> call_
//!   call_ --> b1
//!   a2 --> p
//!   p -->|"each ret branches here,<br/>%r := phi of the %v"| b2
//!   call_ ==>|"inline"| p
//!   style g_before fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//!   style g_after fill:#47556912,stroke:#475569,stroke-width:1.6px,stroke-dasharray:7 5,color:#475569
//! ```
//!
//! *Inlining a call to g*
//!
//! A callee can be cloned (`cloneable`: defined, `splice` carries it, it
//! returns, it is not recursive, not `noinline`, and does not call
//! `setjmp`-like functions); whether it is copied at a site is
//! `inlinecost::decide`'s, the walk's ([`crate::walk`]).
//!
//! A static alloca of the callee goes to the caller's entry; one whose count
//! is an argument folds to a constant in the copy, as
//! `CloneAndPruneFunctionInto` simplifies it, and is static too. Any other (a
//! variable count) stays where it is, and the copy is bracketed by
//! `llvm.stacksave` and, before each return, `llvm.stackrestore`.
//!
//! A copy is allowed (`fits_with`) only where
//!
//! - the call's function type is the callee's and the callee is not variadic;
//! - no body that allocates is copied into a recursive caller, except the last
//!   call of a private callee;
//! - a parameter the callee `releases` is passed an object the caller owns.
//!
//! Which sites are copied is `inlinecost::decide`'s (the walk's).
//!
//! `expanded_all` decides every site of a caller first (`Plan`), tracking the
//! caller's size and stack as they grow and including what is inlined into each
//! copy, then splices from the last site to the first so each split moves only
//! what is left of its block. `byval` arguments the callee writes are copied
//! first.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @min(ptr %x, ptr %y) {
//! b1:
//!   %a = load i16, ptr %x
//!   %b = load i16, ptr %y
//!   %less = icmp slt i16 %a, %b
//!   br i1 %less, label %b2, label %b3
//!
//! b2:
//!   ret i16 %a
//!
//! b3:
//!   ret i16 %b
//! }
//!
//! define i16 @main(i16 %p, i16 %q) {
//! b1:
//!   %x = alloca i16
//!   %y = alloca i16
//!   store i16 %p, ptr %x
//!   store i16 %q, ptr %y
//!   %r = call i16 @min(ptr %x, ptr %y)
//!   ret i16 %r
//! }
//! ```
//!
//! After, `@min` unchanged:
//!
//! ```mir
//! define i16 @main(i16 %p, i16 %q) {
//! b1:
//!   %x = alloca i16
//!   %y = alloca i16
//!   store i16 %p, ptr %x
//!   store i16 %q, ptr %y
//!   br label %0
//!
//! 0:
//!   %a = load i16, ptr %x
//!   %b = load i16, ptr %y
//!   %less = icmp slt i16 %a, %b
//!   br i1 %less, label %1, label %2
//!
//! 1:
//!   br label %3
//!
//! 2:
//!   br label %3
//!
//! 3:
//!   %4 = phi i16 [ %a, %1 ], [ %b, %2 ]
//!   ret i16 %4
//! }
//! ```
//!
//! The call is the callee's blocks with the formals bound to `%x` and `%y`; its
//! two `ret`s branch to the block `3`, which takes the call's result as a phi
//! of what they returned.
//!
//! From unit test
//! `test_a_public_callee_reading_through_its_arguments_inlines_and_stays_defined`
//! in `inline_tests.rs`.
//!
//! # Runs
//! Library of [`crate::interprocedural`], which runs it once every body has
//! reached its fixed point, then sends each changed body back through the
//! pipeline (`inline<N>`, `ipa-inline<N>.`; `LLRM_DEBUG=inline` logs each
//! verdict). Off at -O0; the threshold is 90 at -O1 and -Og, 225 at -O2, 250 at
//! -O3 and -Omax. Per-level budgets are in `docs/reference/levels.md`.
//!
//! # Needs
//! [`crate::profit`] (operation prices, block frequencies),
//! [`llrm_analysis::consts`] (`folded`), and the module's call graph and
//! callee effects.
//!
//! # See also
//! LLVM's
//! [`InlineCost.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/InlineCost.cpp)
//! and GCC's
//! [`ipa-inline.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/ipa-inline.cc).

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::callgraph::CallGraph;
use llrm_mir::context::{ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::facts::{Facts, Inlining};
use llrm_mir::memory::{Callees, callee};
use llrm_mir::module::{Function, InstId, Metadata, MetadataId, Module, Operand, ValueDef};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::splice::{Spliced, carries, spliced};
use llrm_mir::types::Type;
use llrm_support::hash::{IndexMap, SparseIdMap};

use crate::profit::{OperationCosts, operation};

/// How much inlining may copy: LLVM's inline threshold, 225 at -O2 and 0
/// for none. A callee's budget, in semantic operations, scales with it. The
/// hint and hot-site thresholds are `InlineParams`' (inlinecost.rs).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Threshold {
    pub limit: i64,
    /// Where code size outranks speed: a call also removes its arguments'
    /// pushes and cleanup.
    pub single: bool,
    /// The caller is `minsize` (-Oz): a call is held to `OptMinSizeThreshold`.
    pub min_size: bool,
    /// The last call of a function nothing else reaches inlines at any size:
    /// its body moves, nothing is copied: LLVM's last-call-to-static bonus
    /// and GCC's `-finline-functions-called-once`, which
    /// `-fno-inline-functions` leaves on as GCC's does;
    /// `-fno-inline-functions-called-once` turns it off.
    pub last: bool,
    /// `-fipa-cp-clone` (-O3): a function is copied for the constants its
    /// callers pass though the unit grows (`ipacp`).
    pub cp_clone: bool,
    /// `-finline-recursive`: gcc's recursive inlining and peeling (a recursive
    /// function copied into itself to depth 3, a recursive callee's own calls
    /// inlined in the copy, its tail-call elimination deferred until the
    /// copies are made). Off: the pin's inliner, which copies a recursive
    /// callee once (DEVIATE (recursion), docs/architecture/recursion-rows.md).
    pub recursive: bool,
}

impl Threshold {
    pub fn new(limit: i64) -> Self {
        Self { limit, single: false, min_size: false, last: true, cp_clone: false, recursive: false }
    }

    /// Nothing inlines, the last call of a function included.
    pub fn none() -> Self {
        Self { last: false, ..Self::new(0) }
    }

    /// The same where code size outranks speed: the caller is `optsize`, so the
    /// call is held to `OptSizeThreshold` (inlinecost.rs, D8).
    pub fn for_size(self) -> Self {
        Self { single: true, ..self }
    }

    /// What the pin's inliner calls the caller: `hasMinSize`, `optsize` or
    /// neither.
    pub fn caller_size(self) -> crate::inlinecost::CallerSize {
        use crate::inlinecost::CallerSize;
        if self.min_size {
            CallerSize::MinSize
        } else if self.single {
            CallerSize::OptSize
        } else {
            CallerSize::Speed
        }
    }

    /// The same where the caller is `minsize` (-Oz).
    pub fn for_min_size(self) -> Self {
        Self { single: true, min_size: true, ..self }
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self::new(225)
    }
}

impl Threshold {}

/// Direct call counts by callee.
pub type Counter = IndexMap<GlobalId, i64>;

/// What the language says of inlining `body`.
fn stated(body: &Function) -> Option<Inlining> {
    Facts::of(&body.attrs).inline()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub body: Rc<Function>,
    /// Bytes of stack the body allocates.
    pub frame: u64,
    /// Admitted only as the last call of a body nothing else reaches, which it
    /// moves whatever its price: not a copy the price allows.
    pub moved: bool,
    /// A private callee nothing but this one call reaches: the copy takes
    /// the callee's place, so it may allocate in a recursive caller
    /// (INL-REC-1).
    pub last_call: bool,
    /// The function the body is of, where it is a recursive callee whose own
    /// calls may be peeled.
    pub callee: Option<GlobalId>,
    /// Where the body calls itself: the calls it makes of itself per call,
    /// the sum of those calls' frequencies over the entry's.
    pub self_calls: Option<f64>,
    /// How many calls of itself the body has.
    pub self_sites: usize,
}

/// What a caller says of itself that bounds what may be copied into it.
pub struct Caller<'a> {
    pub layout: &'a DataLayout,
    pub recursive: bool,
    /// The copies are of the caller itself: they stop at the recursion limits,
    /// and this is the frequency of each call of the function's own body
    /// over its entry's (a call not in it counts 1).
    pub self_inlining: Option<&'a SparseIdMap<InstId, f64>>,
    /// A recursive callee inlined here may have its own calls inlined in the
    /// copy, gcc's peeling.
    pub peeling: bool,
}

/// DEVIATE (recursion), gcc's `--param max-inline-recursive-depth-auto` is 8
/// (params.opt:573, gcc 13.3; `want_inline_self_recursive_call_p`,
/// ipa-inline.cc:991); 3 here. Measured on the i486 rows,
/// -O2 clocks / bytes (main | depth 8 | depth 3): fib 227171/537 | 227940/1736
/// | 132006/360, rectwo 612458/538 | 1539189/1848 | 530081/520; gcc's depth
/// costs 3-10x main's bytes (bintree 291 -> 2876, quicksort 343 -> 3116).
/// Owner: w-6. Exit: the copies are chosen by gcc's badness heap with its
/// real growth limits (`inline-unit-growth`).
pub const MAX_INLINE_RECURSIVE_DEPTH: usize = 3;

/// gcc 13.3's depth (params.opt:573), which peeling keeps.
const MAX_INLINE_RECURSIVE_DEPTH_GCC: usize = 8;

/// gcc 13.3's `max-inline-insns-recursive-auto` (params.opt:553), which caps
/// what the copies of one peeled call add.
const MAX_INLINE_INSNS_RECURSIVE_GCC: i64 = 450;

/// DEVIATE (recursion), gcc 13.3's `--param max-inline-insns-recursive-auto`
/// is 450 (params.opt:553, `recursive_inlining`, ipa-inline.cc:1645); 150 here,
/// in `gcc_insns`: the size the function may reach by copies of itself. The
/// smallest that keeps fib and rectwo under main (depth 3, -O2 clocks/bytes;
/// main fib 227171/537, rectwo 612458/538): 60: fib 207179/264, rectwo
/// 1489763/280; 100: 132006/360, 876030/424; 150: 132006/360, 530081/520; 450:
/// 132006/360, 530081/520. Owner: w-6. Exit: as the depth's.
pub const MAX_INLINE_INSNS_RECURSIVE: i64 = 150;

/// `--param min-inline-recursive-probability` (10 per cent, params.opt:769): a
/// copy is made only where the call's frequency over the entry's, in the
/// body, exceeds this (`want_inline_self_recursive_call_p`,
/// ipa-inline.cc:1067).
pub const MIN_INLINE_RECURSIVE_PROBABILITY: f64 = 10.0;

/// gcc 13.3 `--param max-inline-insns-auto` (params.opt:545), in `gcc_insns`:
/// what a peeled copy may add.
pub const MAX_INLINE_INSNS_AUTO: i64 = 15;

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

/// `body`'s size as gcc's `estimate_num_insns` counts it (`eni_size_weights`),
/// the unit of `max-inline-insns-auto`: a compare that only a conditional
/// branch reads is that branch (GIMPLE_COND holds its comparison), an address
/// computation only a load or store reads is the access (an ARRAY_REF), and a
/// return counts one where a phi and an unconditional jump count none. The
/// unit of every limit that is gcc's own number (`max-inline-insns-auto`,
/// jump threading's copy limit).
pub fn gcc_insns(body: &Function) -> i64 {
    gcc_insns_in(body, body.walk().map(|(_, inst)| inst))
}

/// `gcc_insns` of the instructions `insts` of `body`: the size of a path
/// jump threading copies.
pub fn gcc_insns_in(
    body: &Function,
    insts: impl IntoIterator<Item = InstId>,
) -> i64 {
    let only_by = |inst: InstId, accepts: &dyn Fn(&Function, &llrm_mir::module::Use) -> bool| {
        let Some(result) = body.instruction(inst).result else { return false };
        let uses = body.users(result);
        !uses.is_empty() && uses.iter().all(|one| accepts(body, one))
    };
    insts
        .into_iter()
        .filter(|&inst| {
            let instruction = body.instruction(inst);
            match instruction.opcode {
                Opcode::Phi => false,
                Opcode::Br => instruction.operands.len() != 1,
                Opcode::ICmp(_) => !only_by(inst, &|body, one| {
                    let user = body.instruction(one.user);
                    user.opcode == Opcode::Br && user.operands.len() == 3 && one.index == 0
                }),
                Opcode::GetElementPtr { .. } => !only_by(inst, &|body, one| {
                    let user = body.instruction(one.user);
                    match user.opcode {
                        Opcode::Load { .. } => one.index == 0,
                        Opcode::Store { .. } => one.index == 1,
                        _ => false,
                    }
                }),
                _ => true,
            }
        })
        .count() as i64
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
            function.iter_block(entry).find(|&one| !matches!(function.instruction(one).opcode, Opcode::Alloca { .. }))
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
        let (memcpy, memcpy_type) =
            crate::loopidioms::_copy(context, declared, crate::loopidioms::How::Apart, 0, space, width);
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
pub(crate) fn grown(
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

/// Whether `body` may be copied as a function of its own (a recursive one
/// included): it returns, `splice` carries it, and it is not to be inlined or
/// `setjmp`-like.
pub(crate) fn copyable(
    module: &Module,
    body: &Function,
) -> bool {
    !body.is_declaration()
        && !llrm_mir::facts::opaque(body)
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
    size_of(module, callees, module.global(function).function()?, None, costs)
}

/// `size` of a body that need not be in the module: a copy specialised for
/// the constants a site passes (`interprocedural::Specialisations`). Its calls
/// may name functions it declared, which `declared` holds and the module not.
pub fn size_of(
    module: &Module,
    callees: &Callees,
    body: &Function,
    declared: Option<&Declared>,
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
                    let name = match module.globals.get(id.0 as usize) {
                        Some(one) => one.name.as_deref(),
                        None => declared.and_then(|declared| declared.pending_name(id)),
                    };
                    name.is_some_and(|name| name.starts_with("llvm.") || name.starts_with("llrm."))
                })
        })
        .map(|point| point.across.len() as i64)
        .sum::<i64>()
        * costs.store;
    work(module, body, callees, costs).map(|work| work + calls + kept)
}

/// What a call costs beyond its own instruction, which inlining saves: the
/// return, and each argument pushed by the caller and read back by the callee.
pub fn call_overhead(
    costs: &OperationCosts,
    arguments: usize,
) -> i64 {
    costs.call + costs.return_ + arguments as i64 * costs.argument
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
    /// The caller's size in `gcc_insns`, kept only to bound self-inlining.
    insns: i64,
    /// What peeled copies have added, in `gcc_insns`.
    peeled: i64,
    frame: u64,
    seq: usize,
}

impl<'a> Plan<'a, '_> {
    /// Whether the call `target` names, inside the copy of `candidate`, is
    /// inlined in turn: a function's call of itself only where peeling is on
    /// (self-inlining, which has no callee set, bounds itself); the peeled
    /// copy descends into nothing else.
    fn descends(
        &self,
        target: &Candidate,
        candidate: &Candidate,
    ) -> bool {
        let itself = target.callee.is_some() && target.callee == candidate.callee;
        if self.caller.peeling { itself } else { !itself }
    }

    /// `candidate` inlined at a call of `function_type` with `actuals`: the
    /// size and stack it adds are kept, and what is inlined in the copy.
    fn inline(
        &mut self,
        function_type: llrm_mir::types::TypeId,
        actuals: &[Actual],
        candidate: &'a Candidate,
        site: InstId,
        depth: usize,
        parent: Option<&'a Candidate>,
    ) -> Option<(Planned<'a>, Actual)> {
        if !fits_with(self.context, self.caller, function_type, |at| actuals[at].owned, candidate) {
            return None;
        }
        let callee = &*candidate.body;
        if let Some(frequencies) = self.caller.self_inlining {
            let grown = gcc_insns(callee) - 1;
            // `want_inline_self_recursive_call_p`: the frequency of the
            // call within the body, over the entry's, above the minimum.
            let frequency = frequencies.get(&site).copied().unwrap_or(1.0);
            if depth > MAX_INLINE_RECURSIVE_DEPTH
                || self.insns + grown > MAX_INLINE_INSNS_RECURSIVE
                || frequency * 100.0 <= MIN_INLINE_RECURSIVE_PROBABILITY
            {
                return None;
            }
            self.insns += grown;
        } else if self.caller.peeling && depth > 1 {
            // Peeling (`want_inline_self_recursive_call_p`,
            // ipa-inline.cc:1033): the copy's own call, `depth - 1`
            // copies deep, is inlined while the probability of
            // recursing is below 1 - 1/8, squared once for
            // each copy before it, and the copy adds a small function's
            // growth. DEVIATE: gcc reads the probability of the one call's
            // edge; here it is the calls of the function to itself per call,
            // so a tree (fib: 1, bintree: 1.25) is not peeled, which doubles
            // a copy at each level (bintree -O2 +0.13% clocks, 988 to 1276 B).
            let level = depth - 1;
            // The edge's frequency in the caller, over that of the call that
            // made the outermost copy: the calls to itself per call, to the
            // power of the copies it is deep.
            let reached = parent.and_then(|one| one.self_calls).unwrap_or(1.0).powi(level as i32);
            // The squaring bounds the copies, which are `sites` to the power of
            // the depth: with one call the copies grow by one a level and the
            // limit stays (hanoi: 133 clocks with it squared, 103 without).
            let mut limit = 1.0 - 1.0 / MAX_INLINE_RECURSIVE_DEPTH_GCC as f64;
            if parent.is_some_and(|one| one.self_sites > 1) {
                for _ in 1..level {
                    limit *= limit;
                }
            }
            let grown = gcc_insns(callee) - 1;
            // DEVIATE: gcc has no cap on peeling; the copies of one call add at
            // most what gcc lets a function grow by recursive inlining, 450
            // (recpow -O1: uncapped 32 copies and 8203 bytes; 150 left it at
            // 486966 clocks against 475701 before; 450 is 454246 / 5547).
            if level > MAX_INLINE_RECURSIVE_DEPTH_GCC
                || reached >= limit
                || grown > MAX_INLINE_INSNS_AUTO
                || self.peeled + grown > MAX_INLINE_INSNS_RECURSIVE_GCC
            {
                return None;
            }
            self.peeled += grown;
        }
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
                    && self.descends(target, candidate)
                    && let Some((planned, result)) =
                        self.inline(info.function_type, &seen, target, inst, depth + 1, Some(candidate))
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
    /// The copies' lifetimes, marked once every copy is made.
    lifetimes: Vec<Lifetimes>,
}

/// The static allocas of a copy no marker bounds, and where the copy begins
/// and returns.
struct Lifetimes {
    allocas: Vec<InstId>,
    /// The address space of the copy's dynamic allocas, if it has any: the copy
    /// is bracketed by `stacksave` and `stackrestore` so the caller's stack
    /// does not grow per call.
    dynamic: Option<u32>,
    entry: llrm_mir::module::BlockId,
    returns: Vec<InstId>,
}

/// `lifetime.start` and `lifetime.end`'s name and type for an alloca in
/// `space`, declared where the module has none.
fn lifetime_markers(
    context: &mut Context,
    declared: &mut Declared,
    space: u32,
) -> ([GlobalId; 2], llrm_mir::types::TypeId) {
    let types = &mut context.types;
    let (void, pointer, size) = (types.void(), types.ptr(space), types.int(64));
    let ty = types.intern(Type::Function { returns: void, parameters: vec![size, pointer], variadic: false });
    let start = declared.declare(&format!("llvm.lifetime.start.p{space}"), ty);
    let end = declared.declare(&format!("llvm.lifetime.end.p{space}"), ty);
    ([start, end], ty)
}

/// The static allocas of `body`'s copy that `InsertLifetime` marks: those of
/// its entry no lifetime marker of `body`'s bounds already
/// (`hasLifetimeMarkers`), of a size other than 0.
fn unmarked_allocas(
    context: &mut Context,
    function: &Function,
    layout: &DataLayout,
    body: &Function,
    made: &Spliced,
    declared: &mut Declared,
) -> Lifetimes {
    let entry = body.entry();
    let originals = body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret);
    let mut allocas = Vec::new();
    for ((block, inst), &copy) in originals.zip(&made.copies) {
        let Opcode::Alloca { allocated, address_space, .. } = body.instruction(inst).opcode else { continue };
        // Only a static one: a dynamic alloca stays where it was copied, after
        // its markers.
        let static_copy = function.parent(copy) == function.entry() && is_static_alloca(context, function, copy);
        if Some(block) != entry || !static_copy || layout.alloc_size(&context.types, allocated) == 0 {
            continue;
        }
        let pointer = body.instruction(inst).result.map(Operand::Value);
        if pointer.is_none_or(|one| outlives_the_copy(body, one)) {
            continue;
        }
        let markers = lifetime_markers(context, declared, address_space).0;
        let marked = body
            .walk()
            .any(
                |(_, one)| callee(context, body, one).is_some_and(|id| markers.contains(&id))
                    && body.instruction(one).operands.get(1).copied() == pointer,
            );
        if !marked {
            allocas.push(copy);
        }
    }
    Lifetimes {
        allocas,
        dynamic: dynamic_alloca_space(function, &made.copies),
        entry: made.entry,
        returns: made.returns.clone(),
    }
}

/// Whether the object at `pointer` may be read after the body returns: its
/// address, or one derived from it, is returned or stored as a value.
///
/// DEVIATE D10 (docs/architecture/cgscc-plan.md): `InsertLifetime` marks every
/// static alloca, as returning or keeping a local's address is undefined in C.
/// Fact: QB's runtime bodies, copied only, return their own allocas (`B$FMID`'s
/// view); ending the view at the return made the read through it undef. Exit:
/// the lifetime ends after the copy's last use of the address, not at its
/// return.
fn outlives_the_copy(
    body: &Function,
    pointer: Operand,
) -> bool {
    let Operand::Value(object) = pointer else { return false };
    let mut pending = vec![object];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        for one in body.users(value) {
            let instruction = body.instruction(one.user);
            match (&instruction.opcode, one.index) {
                (Opcode::Ret, _) | (Opcode::Store { .. }, 0) => return true,
                (Opcode::GetElementPtr { .. } | Opcode::Cast(_) | Opcode::Phi | Opcode::Select, _) => {
                    pending.extend(instruction.result);
                }
                _ => {}
            }
        }
    }
    false
}

/// `CloneAndPruneFunctionInto`: the copy of a body's entry is simplified as the
/// arguments make constants of it, so an alloca of the entry whose count was
/// the argument's (`int a[n]`) has a constant one and is static: it goes to the
/// caller's entry, and the copy has no dynamic alloca to bracket.
fn fold_entry_allocas(
    context: &Context,
    function: &mut Function,
    layout: &DataLayout,
    body: &Function,
    made: &Spliced,
    anchor: Option<InstId>,
) {
    let entry = body.entry();
    let originals = body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret);
    let copies: Vec<(InstId, bool)> = originals
        .zip(&made.copies)
        .filter(|&((block, _), _)| Some(block) == entry)
        .map(|((_, inst), &copy)| (copy, matches!(body.instruction(inst).opcode, Opcode::Alloca { .. })))
        .collect();
    if !copies.iter().any(|&(copy, alloca)| alloca && !is_static_alloca(context, function, copy)) {
        return;
    }
    for &(copy, alloca) in &copies {
        if alloca {
            continue;
        }
        let Some(result) = function.instruction(copy).result else { continue };
        if let Some(folded) = llrm_mir::simplify::simplify_instruction(context, layout, function, copy) {
            function.replace_all_uses_with(result, folded);
        }
    }
    let first = anchor
        .filter(|&one| function.parent(one).is_some())
        .or_else(|| function.entry().and_then(|entry| function.block(entry).first()));
    let Some(first) = first else { return };
    for &(copy, alloca) in &copies {
        if alloca && function.parent(copy) != function.entry() && is_static_alloca(context, function, copy) {
            function.move_to(copy, Position::Before(first)).expect("a placed alloca");
        }
    }
}

/// Whether the alloca `inst` has a constant count (the entry block aside).
fn is_static_alloca(
    _context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).operands.iter().all(|one| matches!(one, Operand::Constant(_)))
}

/// The address space of the first alloca among `copies` that is not static: not
/// on the caller's entry or of a variable count.
fn dynamic_alloca_space(
    function: &Function,
    copies: &[InstId],
) -> Option<u32> {
    copies
        .iter()
        .find_map(
            |&copy| match function.instruction(copy).opcode {
                Opcode::Alloca { address_space, .. }
                    if function.parent(copy) != function.entry()
                        || !function.instruction(copy).operands.iter().all(|one| matches!(one, Operand::Constant(_))) =>
                {
                    Some(address_space)
                }
                _ => None,
            },
        )
}

/// `InlineFunction`'s stack save and restore: a copy with a dynamic alloca
/// saves the stack pointer where it begins and restores it before each return.
fn bracket_dynamic_stack(
    context: &mut Context,
    function: &mut Function,
    copy: &Lifetimes,
    space: u32,
    declared: &mut Declared,
) {
    let types = &mut context.types;
    let (void, pointer) = (types.void(), types.ptr(space));
    let save_ty = types.intern(Type::Function { returns: pointer, parameters: vec![], variadic: false });
    let restore_ty = types.intern(Type::Function { returns: void, parameters: vec![pointer], variadic: false });
    let save = declared.declare(&format!("llvm.stacksave.p{space}"), save_ty);
    let restore = declared.declare(&format!("llvm.stackrestore.p{space}"), restore_ty);
    let call = |context: &Context, function: &mut Function, id: GlobalId, ty, ret, operands: Vec<Operand>| {
        let target =
            context.constant(llrm_mir::context::Constant { ty: context.types.ptr(0), kind: ConstantKind::Global(id) });
        let info = CallInfo {
            function_type: ty,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); operands.len()],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let mut operands = operands;
        operands.push(Operand::Constant(target));
        function.create_instruction(Opcode::Call(Box::new(info)), ret, operands, Flags::default(), None)
    };
    let first = function.block(copy.entry).first().expect("a copied entry");
    let saved = call(context, function, save, save_ty, pointer, Vec::new());
    function.insert(saved, Position::Before(first)).expect("a placed block");
    let saved = Operand::Value(function.instruction(saved).result.expect("a pointer"));
    for &back in &copy.returns {
        let finish = call(context, function, restore, restore_ty, void, vec![saved]);
        function.insert(finish, Position::Before(back)).expect("a placed block");
    }
}

/// `InlineFunction`'s `InsertLifetime`: each alloca of `lifetimes` begins at
/// the copy's first instruction and ends before each of its returns, so that
/// copies whose code does not overlap may share a slot (`lifetimes` in the
/// backend).
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/InlineFunction.cpp>
fn insert_lifetimes(
    context: &mut Context,
    function: &mut Function,
    layout: &DataLayout,
    lifetimes: &Lifetimes,
    declared: &mut Declared,
) {
    let void = context.types.void();
    let i64_ty = context.types.int(64);
    if let Some(space) = lifetimes.dynamic {
        bracket_dynamic_stack(context, function, lifetimes, space, declared);
    }
    for &alloca in &lifetimes.allocas {
        let Opcode::Alloca { allocated, address_space, .. } = function.instruction(alloca).opcode else { continue };
        let ([start, end], ty) = lifetime_markers(context, declared, address_space);
        let size = Operand::Constant(context.int(i64_ty, layout.alloc_size(&context.types, allocated) as i128));
        let pointer = Operand::Value(function.instruction(alloca).result.expect("an alloca's value"));
        let marker = |context: &Context, function: &mut Function, id: GlobalId| {
            let target = context
                .constant(llrm_mir::context::Constant { ty: context.types.ptr(0), kind: ConstantKind::Global(id) });
            let info = CallInfo {
                function_type: ty,
                calling_convention: 0,
                return_attrs: Vec::new(),
                argument_attrs: vec![Vec::new(); 2],
                attrs: Vec::new(),
                tail: Default::default(),
            };
            function.create_instruction(
                Opcode::Call(Box::new(info)),
                void,
                vec![size, pointer, Operand::Constant(target)],
                Flags::default(),
                None,
            )
        };
        let first = function.block(lifetimes.entry).first().expect("a copied entry");
        let begin = marker(context, function, start);
        function.insert(begin, Position::Before(first)).expect("a placed block");
        for &back in &lifetimes.returns {
            let finish = marker(context, function, end);
            function.insert(finish, Position::Before(back)).expect("a placed block");
        }
    }
}

/// The `!dbg` location of `inst`.
fn dbg_of(
    function: &Function,
    inst: InstId,
) -> Option<MetadataId> {
    function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "dbg").map(|&(_, node)| node)
}

/// The copies of `body` placed at a call located at `site`: each location
/// says it was inlined at `site`, and an instruction without one is at
/// `site` itself (`fixupLineNumbers`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/InlineFunction.cpp#L1706>).
fn inlined_at(
    function: &mut Function,
    body: &Function,
    copies: &[InstId],
    site: MetadataId,
    metadata: Metadata,
    declared: &mut Declared,
) {
    let originals = body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret);
    for ((_, inst), &copy) in originals.zip(copies) {
        let node = dbg_of(body, inst).map_or(site, |loc| declared.inlined_at(metadata, loc, site));
        function.set_metadata(copy, "dbg", Some(node));
    }
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
    metadata: Metadata,
) -> Result<(), String> {
    let seq = planned.seq;
    let located = dbg_of(function, call);
    let after = placed.copies.range(seq + 1..).next().map(|(_, &one)| one);
    if let Some(copy) = copy_byval_arguments(context, function, layout, call, &planned.candidate.body, declared, after)?
    {
        placed.copies.insert(seq, copy);
    }
    let anchor = placed.stack.range(..seq).next_back().map(|(_, &one)| one).or(placed.base);
    let site = match &function.instruction(call).opcode {
        Opcode::Call(info) => info.tail,
        _ => llrm_mir::opcode::Tail::None,
    };
    let made = spliced(context, function, call, &planned.candidate.body, anchor);
    if let Some(site) = located {
        inlined_at(function, &planned.candidate.body, &made.copies, site, metadata, declared);
    }
    fold_entry_allocas(context, function, layout, &planned.candidate.body, &made, anchor);
    narrow_tail_calls(function, &made.copies, site);
    placed.lifetimes.push(unmarked_allocas(context, function, layout, &planned.candidate.body, &made, declared));
    let copies = made.copies;
    // The call was the entry's first instruction and is gone: the copies that
    // come out of this one are anchored on what the entry starts with now.
    if placed.base == Some(call) {
        placed.base = function.entry().and_then(|entry| function.block(entry).first());
    }
    let body = &*planned.candidate.body;
    let entry = body.entry();
    let first = body.walk().filter(|&(_, inst)| body.instruction(inst).opcode != Opcode::Ret).zip(&copies).find(
        |&((block, inst), _)| Some(block) == entry && matches!(body.instruction(inst).opcode, Opcode::Alloca { .. }),
    );
    if let Some((_, &copy)) = first {
        placed.stack.insert(seq, copy);
    }
    for (place, nested) in planned.nested.iter().rev() {
        executed(context, function, layout, copies[*place], nested, declared, placed, metadata)?;
    }
    Ok(())
}

/// The `tail` markers of the calls a body brings, as `InlineFunction` leaves
/// them: a call is no more `tail` than the site it was inlined at
/// (`ChildTCK = std::min(CallSiteTailKind, ChildTCK)` unless the child is
/// `notail`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/InlineFunction.cpp#L3136>);
/// the body's `tail` rested on its own frame, not on the caller's. `notail`
/// at the site is no marker.
fn narrow_tail_calls(
    function: &mut Function,
    copies: &[InstId],
    site: llrm_mir::opcode::Tail,
) {
    use llrm_mir::opcode::Tail;
    let rank = |kind: Tail| match kind {
        Tail::None => 0,
        Tail::Tail => 1,
        Tail::MustTail => 2,
        Tail::NoTail => 3,
    };
    let site = if site == Tail::NoTail { Tail::None } else { site };
    for &copy in copies {
        let Opcode::Call(info) = &function.instruction(copy).opcode else { continue };
        if info.tail != Tail::NoTail && rank(site) < rank(info.tail) {
            let mut info = info.clone();
            info.tail = site;
            function.set_opcode(copy, Opcode::Call(info));
        }
    }
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
    metadata: Metadata,
) -> Result<usize, String> {
    let calls: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let start = (semantic_count(function), frame(context, caller.layout, function));
    let mut plan = Plan {
        context: &*context,
        caller,
        available,
        own: start.0,
        insns: gcc_insns(function),
        peeled: 0,
        frame: start.1,
        seq: 0,
    };
    let mut results = SparseIdMap::default();
    let mut sites: Vec<(InstId, Planned)> = Vec::new();
    for call in calls {
        let instruction = function.instruction(call);
        let Opcode::Call(info) = &instruction.opcode else { continue };
        let scope = Scope { function, actuals: None, results: &results };
        let seen: Vec<Actual> = instruction.operands.iter().map(|&one| scope.actual(plan.context, one, 0)).collect();
        let target = match constant {
            Some(sites) => sites.get(&call),
            None => seen.last().and_then(|one| one.callee).and_then(|name| available.get(&name)),
        };
        let Some(target) = target else { continue };
        plan.peeled = 0;
        if let Some((planned, result)) = plan.inline(info.function_type, &seen, target, call, 1, None) {
            if instruction.result.is_some() {
                results.insert(call, result);
            }
            sites.push((call, planned));
        }
    }
    let end = (plan.own, plan.frame);
    let mut placed =
        Placed { base: function.entry().and_then(|entry| function.block(entry).first()), ..Placed::default() };
    for (call, planned) in sites.iter().rev() {
        executed(context, function, caller.layout, *call, planned, declared, &mut placed, metadata)?;
    }
    assert_eq!(end.0, semantic_count(function), "the size kept is not the body's");
    assert_eq!(end.1, frame(context, caller.layout, function), "the stack kept is not the body's");
    for lifetimes in &placed.lifetimes {
        insert_lifetimes(context, function, caller.layout, lifetimes, declared);
    }
    Ok(sites.len())
}

/// Whether the call fits its callee, which returns, and in a recursive
/// function no body that allocates is copied unless it is the callee's last
/// call; the call is told what it is by `owned_at`.
fn fits_with(
    context: &Context,
    caller: &Caller,
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
        && (candidate.frame == 0 || !caller.recursive || candidate.last_call)
        && callee
            .parameters()
            .iter()
            .enumerate()
            .all(|(at, _)| !Facts::of(&callee.parameter_attrs[at]).releases() || owned_at(at))
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
