//! LLVM's inline cost: what inlining one call site costs, and the threshold
//! it is held to, translated from `InlineCost.cpp` and `InlineAdvisor.cpp`.
//!
//! # How it works
//! `inline_cost` is `getInlineCost`: a callee's attributes decide first
//! (`attribute_decision`); else `CallAnalyzer` walks the callee's live blocks
//! from the entry, breadth first, as the call site would leave them (a
//! conditional branch on a constant argument has one live successor, and the
//! blocks only the other reached are dead), adds the cost of every instruction
//! that does not simplify away, and compares it with the threshold:
//!
//! - an instruction costs `INSTR_COST` (5) unless the walk proves it free: it
//!   folds to a constant on the site's arguments, is a free cast or address, a
//!   branch that folds, the first `ret`, a load or store that SROA of a caller
//!   alloca removes, a load of an address loaded before;
//! - a call it cannot fold costs its argument setup and `CALL_PENALTY` (25);
//! - the threshold starts at 225 (`-O2`; 250 `-O3`) and is raised by
//!   `inlinehint` (325), lowered at an `optsize` caller (50) or `minsize` (5)
//!   or a cold site (45), raised at a hot one; half of it again is a bonus
//!   while the callee stays one block, and the last call to a local function
//!   takes 15000 off the cost;
//! - the call and its arguments disappear: `callsite_cost` is taken off.
//!
//! A site is inlined where `cost < max(1, threshold)`. The caller of this
//! module asks `inline_cost` and acts on the verdict; nothing is changed
//! here.
//!
//! # Example
//! A callee of one block, `%a = load @g; %b = add %a, %x; store %b, @g; ret
//! %b`, called from `@f` (not the only call): the single-block bonus raises the
//! threshold to 337 and the load, add and store cost 5 each less the call
//! they replace, which comes to -20, as `opt -passes=inline` says.
//!
//! From unit test `test_a_single_block_callee_costs_what_opt_says` in
//! `inlinecost_tests.rs`.
//!
//! # Runs
//! Off by default. With `LLRM_EXP_INLINECOST` the interprocedural stage prints
//! each site's cost and threshold beside the existing decision; with
//! `LLRM_EXP_INLINEDECIDE` it decides each call site by them.
//!
//! # Deviations from LLVM (numbered, measured when the diff against
//! `opt -passes=inline` has rows)
//! - D1: block frequencies: `BlockFrequencyInfo` over `BranchProbabilityInfo`,
//!   both translated (`llrm_analysis::blockfreq`, `llrm_analysis::bpi`). The
//!   caller passes the site's frequency relative to its entry
//!   (`Site::relative_frequency`), a ratio where LLVM compares
//!   `BlockFrequency`s.
//! - D2: `TargetTransformInfo`: `getInstructionCost`, `isLoweredToCall`,
//!   `getFPOpCost`, `getEstimatedNumberOfCaseClusters` are the generic
//!   (BasicTTIImpl) answers with the x86 legal addressing modes, in
//!   `free_instruction`, `lowered_to_call` and `basic_case_clusters`
//!   (`tti.rs`).
//! - D3: constant folding is integer, float and `select`; vectors and constant
//!   expressions are not folded.
//! - D4: no PGO, ThinLTO, ML advisor, `optnone`, `flatten`, coroutines, strict
//!   FP or target-feature checks, no `call-inline-cost` style string
//!   attributes, no cost-benefit analysis (off by default in LLVM).
//! - D5: ephemeral values (operands only an `llvm.assume` reads) are not
//!   skipped; assumes are free instructions.
//! - D6: `simplifyCmpInstForRecCall` and `llvm.objectsize` / `llvm.is.constant`
//!   are not translated (a missed simplification costs `INSTR_COST`).
//! - D8: -Os and -Oz share one pipeline here, so every caller at -Os is
//!   `optsize` (clang's `OptimizeForSize`, InlineCost.cpp:2125) and none
//!   `minsize` (:2117).
//! - D11: at -Os a callee that stays is inlined only if it does not grow the
//!   code (`Tti::opt_size_inline_by_code_size`, X86: yes): the target's
//!   `CodeSize` of the callee's instructions that the walk did not fold must
//!   not exceed the `CodeSize` of the call and the pushes of its arguments.
//!   LLVM's `OptSizeThreshold` 50 allows growth; a fixed bound (25) copied a
//!   14-instruction loop into ten sites (`idioms.bas`, +690 B). The last call
//!   of a local callee, which goes with it, is the pin's. No shape of callee
//!   but the loop and the one smaller than its call is tested.
//! - D12 (SZ2): a call the -Os bound (D11) alone refuses, that passes the
//!   address of a stack slot or a global the caller stored a constant to and
//!   whose callee loads through that parameter, is inlined if the caller,
//!   simplified again with the call inlined, is smaller by the target's size
//!   cost (`walk.rs`, callers of at most 300 instructions; the sites of a
//!   private callee that dies with its last call are tried together, its body
//!   credited; so are the sites of a private callee called only from this
//!   caller, whatever they pass). The pin's cost does not follow a load through
//!   a pointer argument, so it never sees the loop or the arithmetic fold.
//! - D9: `noipa` (GCC's; LLVM has no IR form) is `Never` before any other
//!   attribute decision.
//! - D7: the minsize loop penalty counts retreating-edge headers, not
//!   `LoopInfo`.
//! - D10 (`-finline-recursive`, off by default): a callee that calls itself is
//!   priced as any (LLVM: "recursive", `IsRecursiveCall`, unless
//!   `AllowRecursiveCall`), and `decide` leaves a call of the caller itself to
//!   the copies `walk::SelfInline` makes (gcc's `recursive_inlining`,
//!   ipa-inline.cc:1645: depth `MAX_INLINE_RECURSIVE_DEPTH` 3 against gcc's 8,
//!   `MAX_INLINE_INSNS_RECURSIVE` 150 against 450, the call's frequency above
//!   10%; none at -Os; calls of a function that reads and writes nothing with
//!   the same arguments count once; only a tree, at least one self-call per
//!   call). They follow ipa-cp's second round, as the pin's specializer
//!   precedes its inliner (PassBuilderPipelines.cpp:1228 against :1331), and so
//!   does the tail-call elimination of such a function (the pin: inside its
//!   function simplification, :688). Why: -O2 clocks/bytes, main | this: fib
//!   227171/537 | 154398/392, rectwo 612458/538 | 597725/552, recchop -O3
//!   5946/1450 | 5932/5113; gcc's 8 and 450 cost 3-10x main's bytes. Owner:
//!   w-6. Exit: the copies chosen by gcc's badness heap with its growth limits,
//!   tail-call elimination at the pin's slot; open rows hanoi, bintree,
//!   quicksort, x_mergesort, recsum (docs/architecture/recursion-rows.md). A
//!   recursive callee inlined into another function is also peeled
//!   (ipa-inline.cc:1033): its own calls inlined in the copy while the calls of
//!   it to itself per call, to the power of the copies deep, are below 1 - 1/8
//!   (squared per copy where the body has more than one call to itself), a copy
//!   adds at most 15 `gcc_insns` and the copies of a call at most 450; a tree
//!   is never peeled; not at -Os.
//!
//! # See also
//! [`crate::inline`], the decision this is to replace; the plan is
//! the cgscc plan page in docs/architecture.

use std::collections::BTreeSet;

use llrm_mir::context::{ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Linkage, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{AtomicKind, Attribute, BinaryOp, CallInfo, CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::tti::{CastKind, CostKind, Ty};
use llrm_mir::types::{FloatKind, Type};
use llrm_support::hash::HashMap;

// InlineCost.cpp:58-186, the `cl::opt` defaults.
/// `inline-instr-cost`.
pub const INSTR_COST: i64 = 5;
/// `inline-call-penalty`.
pub const CALL_PENALTY: i64 = 25;
/// `inlinedefault-threshold` / `inline-threshold`.
pub const DEFAULT_THRESHOLD: i64 = 225;
/// `inlinehint-threshold`.
pub const HINT_THRESHOLD: i64 = 325;
/// `inline-cold-callsite-threshold`.
pub const COLD_CALLSITE_THRESHOLD: i64 = 45;
/// `inlinecold-threshold`.
pub const COLD_THRESHOLD: i64 = 45;
/// `hot-callsite-threshold`.
pub const HOT_CALLSITE_THRESHOLD: i64 = 3000;
/// `locally-hot-callsite-threshold`.
pub const LOCALLY_HOT_CALLSITE_THRESHOLD: i64 = 525;
/// `cold-callsite-rel-freq`, a percentage.
pub const COLD_CALLSITE_REL_FREQ: f64 = 2.0;
/// `hot-callsite-rel-freq`, a multiple.
pub const HOT_CALLSITE_REL_FREQ: f64 = 60.0;
/// `inline-memaccess-cost`.
pub const MEM_ACCESS_COST: i64 = 0;
// InlineCost.h:36-52, `InlineConstants`.
pub const OPT_SIZE_THRESHOLD: i64 = 50;
pub const OPT_MIN_SIZE_THRESHOLD: i64 = 5;
pub const OPT_AGGRESSIVE_THRESHOLD: i64 = 250;
pub const INDIRECT_CALL_THRESHOLD: i64 = 100;
pub const LOOP_PENALTY: i64 = 25;
pub const COLDCC_PENALTY: i64 = 2000;
pub const TOTAL_ALLOCA_SIZE_RECURSIVE_CALLER: u64 = 1024;
pub const MAX_SIMPLIFIED_DYNAMIC_ALLOCA_TO_INLINE: u64 = 1 << 16;
// TargetTransformInfoImpl.h:91-111, the generic answers.
const LAST_CALL_TO_STATIC_BONUS: i64 = 15000;
const VECTOR_BONUS_PERCENT: i64 = 150;
/// `Function::getCallingConv() == Cold`.
const COLD_CC: u32 = 9;

/// `InlineParams` (InlineCost.h:200-240) as `getInlineParams` makes them
/// (InlineCost.cpp:3376-3433, 3435).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineParams {
    pub default_threshold: i64,
    pub hint_threshold: i64,
    pub opt_size_hint_threshold: i64,
    pub cold_threshold: i64,
    pub cold_callsite_threshold: i64,
    pub hot_callsite_threshold: i64,
    pub locally_hot_callsite_threshold: Option<i64>,
    pub opt_min_size_threshold: i64,
    pub opt_size_threshold: i64,
    /// `ComputeFullInlineCost`: the whole cost, past the threshold too (an
    /// optimisation remark asks for it).
    pub compute_full_inline_cost: bool,
    /// `AllowRecursiveCall`: a callee that calls itself is priced, not
    /// refused. The pin's is false; D10 (`-finline-recursive`) sets it.
    pub allow_recursive_call: bool,
    /// With `allow_recursive_call`, the largest recursive callee (`gcc_insns`)
    /// priced rather than refused.
    pub recursive_callee_limit: i64,
    /// The `LastCallToStatic` bonus applies. The pin's always does;
    /// `-fno-inline-functions-called-once` turns it off.
    pub last_call_to_static: bool,
    /// Which target's answers `getInstructionCost` and its kin give (D2).
    pub tti: Tti,
}

/// The cost model of the target (D2): the generic one is what `opt` has
/// without a triple, the oracle of this translation; `Basic` is
/// `BasicTTIImpl`'s, which every real target extends.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Tti {
    #[default]
    Generic,
    Basic,
}

impl InlineParams {
    /// `getInlineParams(Threshold)`.
    pub fn new(threshold: i64) -> Self {
        Self {
            default_threshold: threshold,
            hint_threshold: HINT_THRESHOLD,
            opt_size_hint_threshold: HINT_THRESHOLD,
            cold_threshold: COLD_THRESHOLD,
            cold_callsite_threshold: COLD_CALLSITE_THRESHOLD,
            hot_callsite_threshold: HOT_CALLSITE_THRESHOLD,
            locally_hot_callsite_threshold: None,
            opt_min_size_threshold: OPT_MIN_SIZE_THRESHOLD,
            opt_size_threshold: OPT_SIZE_THRESHOLD,
            compute_full_inline_cost: false,
            allow_recursive_call: false,
            recursive_callee_limit: i64::MAX,
            last_call_to_static: true,
            tti: Tti::Generic,
        }
    }

    /// `getInlineParamsFromOptLevel(OptLevel)`: the speedup level of the
    /// pipeline (`-Os` and `-Oz` are 2).
    pub fn from_level(speedup: u32) -> Self {
        let mut params = Self::new(if speedup > 2 { OPT_AGGRESSIVE_THRESHOLD } else { DEFAULT_THRESHOLD });
        if speedup > 2 {
            params.locally_hot_callsite_threshold = Some(LOCALLY_HOT_CALLSITE_THRESHOLD);
        }
        params
    }
}

/// What the caller says of its own size, `Function::hasOptSize` and
/// `hasMinSize`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CallerSize {
    #[default]
    Speed,
    OptSize,
    MinSize,
}

/// A call site to price: the caller, the call, the callee and what the module
/// says of them.
pub struct Site<'a> {
    pub module: &'a Module,
    pub layout: &'a DataLayout,
    pub caller: &'a Function,
    pub caller_id: GlobalId,
    pub call: InstId,
    pub callee: &'a Function,
    pub callee_id: GlobalId,
    /// The callee has local linkage and this call is its only use
    /// (`isSoleCallToLocalFunction`).
    pub sole_call_to_local: bool,
    /// The caller calls itself (`IsCallerRecursive`).
    pub caller_recursive: bool,
    pub caller_size: CallerSize,
    /// The site's block frequency over the caller's entry block's (D1).
    pub relative_frequency: Option<f64>,
    /// The target's LLVM cost model (`Machine::tti`); none: the generic rules.
    pub tti: Option<&'a dyn llrm_mir::tti::Tti>,
}

/// `InlineCost` (InlineCost.h:120-190).
#[derive(Clone, Debug, PartialEq)]
pub enum InlineCost {
    Always(&'static str),
    Never(String),
    /// The cost and threshold the call was held to.
    Cost {
        cost: i64,
        threshold: i64,
        static_bonus: i64,
        /// Refused by the target's -Os bound alone (D11): the pin's threshold
        /// would have allowed it.
        blocked: bool,
    },
}

impl InlineCost {
    /// `explicit operator bool` (InlineCost.h:138): `Cost < Threshold`. The
    /// `std::max(1, Threshold)` is `finalizeAnalysis`'s, which decides whether
    /// the walk succeeded, not whether the verdict pays.
    pub fn is_ok(&self) -> bool {
        match self {
            Self::Always(_) => true,
            Self::Never(_) => false,
            Self::Cost { cost, threshold, .. } => cost < threshold,
        }
    }
}

fn has_flag(
    attrs: &[Attribute],
    name: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
}

/// Whether the function `id` is declared `frame_bound`.
fn frame_bound(
    module: &Module,
    id: GlobalId,
) -> bool {
    module.global(id).function().is_some_and(|function| has_flag(&function.attrs, "frame_bound"))
}

fn call_info(
    function: &Function,
    inst: InstId,
) -> Option<&CallInfo> {
    match &function.instruction(inst).opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => Some(info),
        _ => None,
    }
}

/// `getAttributeBasedInliningDecision` (InlineCost.cpp:3191): some verdict the
/// callee's and call's attributes settle, or none.
///
/// D4: no coroutine, target-feature, strict FP, `optnone`, `flatten` checks.
pub fn attribute_decision(site: &Site) -> Option<Result<(), String>> {
    let call = call_info(site.caller, site.call)?;
    let callee_attrs = &site.callee.attrs;
    // D9: GCC's `noipa` keeps a routine out of every interprocedural pass.
    if llrm_mir::facts::opaque(site.callee) {
        return Some(Err("noipa function attribute".into()));
    }
    if has_flag(&call.attrs, "alwaysinline") || has_flag(callee_attrs, "alwaysinline") {
        if has_flag(&call.attrs, "noinline") {
            return Some(Err("noinline call site attribute".into()));
        }
        return Some(inline_viable(site.module, site.callee, site.callee_id));
    }
    if has_flag(callee_attrs, "noinline") {
        return Some(Err("noinline function attribute".into()));
    }
    if has_flag(&call.attrs, "noinline") {
        return Some(Err("noinline call site attribute".into()));
    }
    // Don't inline what can be interposed at link time: the body is not the
    // definition that runs.
    if site.module.global(site.callee_id).linkage.is_interposable() {
        return Some(Err("interposable".into()));
    }
    None
}

/// `isInlineViable` (InlineCost.cpp:3320): nothing in the callee stops it
/// being inlined anywhere.
pub fn inline_viable(
    module: &Module,
    callee: &Function,
    callee_id: GlobalId,
) -> Result<(), String> {
    let returns_twice = has_flag(&callee.attrs, "returns_twice");
    for &block in callee.layout() {
        for inst in callee.iter_block(block) {
            let opcode = &callee.instruction(inst).opcode;
            if !matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
                continue;
            }
            let target = llrm_mir::memory::callee(&module.context, callee, inst);
            if target == Some(callee_id) {
                return Err("recursive call".into());
            }
            let info = call_info(callee, inst).expect("a call");
            if !returns_twice && matches!(opcode, Opcode::Call(_)) && has_flag(&info.attrs, "returns_twice") {
                return Err("exposes returns-twice attribute".into());
            }
            if let Some(name) = target.and_then(|one| module.global(one).name.as_deref()) {
                if name == "llvm.icall.branch.funnel" {
                    return Err("disallowed inlining of @llvm.icall.branch.funnel".into());
                }
                if name == "llvm.localescape" {
                    return Err("disallowed inlining of @llvm.localescape".into());
                }
                if target.is_some_and(|one| frame_bound(module, one)) {
                    return Err("disallowed inlining of a frame-bound call".into());
                }
                if name == "llvm.va_start" {
                    return Err("contains VarArgs initialized with va_start".into());
                }
            }
        }
    }
    Ok(())
}

/// `getCallsiteCost` (InlineCost.cpp:3102): what the call and its argument
/// setup cost, which inlining removes.
pub fn callsite_cost(site: &Site) -> i64 {
    let context = &site.module.context;
    let call = site.caller.instruction(site.call);
    let info = call_info(site.caller, site.call).expect("a call");
    let arguments = call.operands.len().saturating_sub(1);
    let pointer_bits = site.layout.pointer(0).bits as u64;
    let mut cost = 0;
    for index in 0..arguments {
        let byval = info
            .argument_attrs
            .get(index)
            .and_then(
                |attrs| attrs
                    .iter()
                    .find_map(
                        |one| match one {
                            Attribute::Type(name, ty) if name == "byval" => Some(*ty),
                            _ => None,
                        },
                    ),
            );
        match byval {
            Some(ty) => {
                let bits = site.layout.size_bits(&context.types, ty) as u64;
                // Ceiling division; more than 8 stores is an inline memcpy.
                let stores = bits.div_ceil(pointer_bits.max(1)).min(8) as i64;
                cost += 2 * stores * INSTR_COST;
            }
            None => cost += INSTR_COST,
        }
    }
    cost += INSTR_COST;
    cost += CALL_PENALTY;
    cost.min(i64::from(i32::MAX))
}

/// What a callee value is, at this site: a constant, or a value of the caller
/// (`SimplifiedValues`, InlineCost.cpp:375).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Simple {
    Int(u32, u128),
    /// A float of the kind, by its IEEE bits (an x87 extended one by the
    /// double nearest it).
    Float(FloatKind, u64),
    /// A pointer constant by its address: `null`, an `inttoptr` of a constant.
    Pointer(u128),
    Const(ConstantId),
    Caller(Operand),
}

/// `CallAnalyzer` and `InlineCostCallAnalyzer` (InlineCost.cpp:211-1250):
/// one analysis of one site.
struct Analyzer<'a> {
    site: &'a Site<'a>,
    params: InlineParams,
    context: &'a Context,
    callee: &'a Function,
    // CallAnalyzer state.
    simplified: IdMap<ValueId, Simple>,
    sroa_values: IdMap<ValueId, ValueId>,
    enabled_sroa: IdSet<ValueId>,
    offsets: IdMap<ValueId, (Operand, i64)>,
    dead_blocks: BTreeSet<u32>,
    known_successors: HashMap<u32, BlockId>,
    load_addrs: BTreeSet<Operand>,
    enable_load_elimination: bool,
    is_recursive_call: bool,
    exposes_returns_twice: bool,
    has_dynamic_alloca: bool,
    contains_noduplicate_call: bool,
    has_return: bool,
    has_indirect_br: bool,
    has_uninlineable_intrinsic: bool,
    inits_vararg: bool,
    allocated_size: u64,
    instructions: u64,
    vector_instructions: u64,
    /// D11: the target's code size of the callee's instructions that cost
    /// (the ones the walk did not fold), in `CostKind::CodeSize` units.
    size_units: i64,
    // InlineCostCallAnalyzer state.
    load_elimination_cost: i64,
    vector_bonus: i64,
    single_bb_bonus: i64,
    threshold: i64,
    static_bonus_applied: i64,
    cost: i64,
    single_bb: bool,
    decided_by_threshold: bool,
    blocked_by_bound: bool,
    sroa_arg_costs: IdMap<ValueId, i64>,
}

impl<'a> Analyzer<'a> {
    fn new(
        site: &'a Site<'a>,
        params: InlineParams,
    ) -> Self {
        Self {
            site,
            params,
            context: &site.module.context,
            callee: site.callee,
            simplified: IdMap::new(),
            sroa_values: IdMap::new(),
            enabled_sroa: IdSet::new(),
            offsets: IdMap::new(),
            dead_blocks: BTreeSet::new(),
            known_successors: HashMap::default(),
            load_addrs: BTreeSet::new(),
            enable_load_elimination: true,
            is_recursive_call: false,
            exposes_returns_twice: false,
            has_dynamic_alloca: false,
            contains_noduplicate_call: false,
            has_return: false,
            has_indirect_br: false,
            has_uninlineable_intrinsic: false,
            inits_vararg: false,
            allocated_size: 0,
            instructions: 0,
            vector_instructions: 0,
            size_units: 0,
            load_elimination_cost: 0,
            vector_bonus: 0,
            single_bb_bonus: 0,
            threshold: params.default_threshold,
            static_bonus_applied: 0,
            cost: 0,
            single_bb: true,
            decided_by_threshold: false,
            blocked_by_bound: false,
            sroa_arg_costs: IdMap::new(),
        }
    }

    /// `addCost` (:684): a capped `int`.
    fn add_cost(
        &mut self,
        increment: i64,
    ) {
        let increment = increment.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
        self.cost = (self.cost + increment).clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    }

    // ---- values -------------------------------------------------------

    /// `getDirectOrSimplifiedValue<Constant>` (:445): what an operand of the
    /// callee is, if a constant here.
    fn constant(
        &self,
        operand: Operand,
    ) -> Option<Simple> {
        match operand {
            Operand::Constant(id) => Some(match self.context.get(id).kind {
                ConstantKind::Int(bits) => match self.context.types.get(self.context.get(id).ty) {
                    Type::Int(width) => Simple::Int(*width, bits),
                    _ => Simple::Const(id),
                },
                ConstantKind::Null => Simple::Pointer(0),
                ConstantKind::Float(bits) => match self.context.types.get(self.context.get(id).ty) {
                    Type::Float(kind) => Simple::Float(*kind, bits),
                    _ => Simple::Const(id),
                },
                _ => Simple::Const(id),
            }),
            Operand::Value(value) => match self.simplified.get(&value) {
                Some(Simple::Int(width, bits)) => Some(Simple::Int(*width, *bits)),
                Some(Simple::Float(kind, bits)) => Some(Simple::Float(*kind, *bits)),
                Some(Simple::Pointer(bits)) => Some(Simple::Pointer(*bits)),
                Some(Simple::Const(id)) => Some(Simple::Const(*id)),
                _ => None,
            },
            Operand::Block(_) => None,
        }
    }

    fn constant_int(
        &self,
        operand: Operand,
    ) -> Option<(u32, u128)> {
        match self.constant(operand)? {
            Simple::Int(width, bits) => Some((width, bits)),
            _ => None,
        }
    }

    /// `getSROAArgForValueOrNull` (:437).
    fn sroa_arg(
        &self,
        operand: Operand,
    ) -> Option<ValueId> {
        let Operand::Value(value) = operand else { return None };
        let alloca = *self.sroa_values.get(&value)?;
        self.enabled_sroa.contains(&alloca).then_some(alloca)
    }

    /// `disableSROAForArg` (:1476).
    fn disable_sroa_for(
        &mut self,
        alloca: ValueId,
    ) {
        // onDisableSROA (:689)
        if let Some(saved) = self.sroa_arg_costs.remove(&alloca) {
            self.add_cost(saved);
        }
        self.enabled_sroa.remove(&alloca);
        self.disable_load_elimination();
    }

    /// `disableSROA` (:1515).
    fn disable_sroa(
        &mut self,
        operand: Operand,
    ) {
        if let Some(alloca) = self.sroa_arg(operand) {
            self.disable_sroa_for(alloca);
        }
    }

    /// `disableLoadElimination` (:1521).
    fn disable_load_elimination(&mut self) {
        if self.enable_load_elimination {
            // onDisableLoadElimination (:699)
            let saved = self.load_elimination_cost;
            self.add_cost(saved);
            self.load_elimination_cost = 0;
            self.enable_load_elimination = false;
        }
    }

    /// `handleSROA` (:371).
    fn handle_sroa(
        &mut self,
        operand: Operand,
        do_not_disable: bool,
    ) -> bool {
        if let Some(alloca) = self.sroa_arg(operand) {
            if do_not_disable {
                // onAggregateSROAUse (:845)
                if let Some(saved) = self.sroa_arg_costs.get_mut(&alloca) {
                    *saved += INSTR_COST;
                }
                return true;
            }
            self.disable_sroa_for(alloca);
        }
        false
    }

    /// `isGEPFree` (:1562) over the generic TTI's `getGEPCost`
    /// (TargetTransformInfoImpl.h:1348, D2): the address folds into its uses
    /// where the generic `isLegalAddressingMode` takes it: no global base, no
    /// constant offset, at most one variable index and that of stride 1.
    fn gep_free(
        &self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst);
        let Opcode::GetElementPtr { source } = &operation.opcode else { return false };
        let global_base = matches!(
            operation.operands[0],
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Global(_))
        );
        let indices: Vec<Option<i128>> = operation.operands[1..]
            .iter()
            .map(|one| self.constant_int(*one).map(|(width, bits)| signed(width, bits)))
            .collect();
        if indices.is_empty() {
            return !global_base;
        }
        let (offset, variable) = self.site.layout.collect_offset(&self.context.types, *source, &indices);
        // The target's `isLegalAddressingMode` (the base is a register unless
        // it is a global), else the generic one: no global, no offset,
        // scale 0 or 1.
        let legal = |scale: i64| match self.site.tti {
            // A scaled index with instructions of its own is not free.
            Some(tti) => {
                (scale <= 1 || tti.scaled_index_insns() == 0)
                    && tti.is_legal_addressing_mode(global_base, offset as i64, !global_base, scale)
            }
            None => !global_base && offset == 0 && scale <= 1,
        };
        match variable[..] {
            [] => legal(0),
            [(_, scale)] => legal(scale as i64),
            _ => false,
        }
    }

    /// `accumulateGEPOffset` (:1532): the GEP's constant offset, or none.
    fn gep_offset(
        &self,
        inst: InstId,
    ) -> Option<i64> {
        let operation = self.callee.instruction(inst);
        let Opcode::GetElementPtr { source } = &operation.opcode else { return None };
        let indices: Vec<Option<i128>> = operation.operands[1..]
            .iter()
            .map(|one| self.constant_int(*one).map(|(width, bits)| signed(width, bits)))
            .collect();
        if indices.iter().any(Option::is_none) {
            return None;
        }
        let (constant, _) = self.site.layout.collect_offset(&self.context.types, *source, &indices);
        Some(constant as i64)
    }

    /// `stripAndComputeInBoundsConstantOffsets` (:2786): the base of a caller
    /// pointer and its constant offset, if every step is an inbounds GEP.
    fn strip_offsets(
        &self,
        pointer: Operand,
    ) -> Option<(Operand, i64)> {
        let caller = self.site.caller;
        let mut at = pointer;
        let mut offset = 0_i64;
        let mut seen = IdSet::<ValueId>::new();
        loop {
            let Operand::Value(value) = at else { break };
            if !seen.insert(value) {
                break;
            }
            let ValueDef::Instruction(def) = caller.value(value).def else { break };
            let operation = caller.instruction(def);
            let Opcode::GetElementPtr { source } = &operation.opcode else { break };
            if !operation.flags.contains(llrm_mir::opcode::Flags::INBOUNDS) {
                return None;
            }
            let mut indices = Vec::new();
            for index in &operation.operands[1..] {
                let Operand::Constant(id) = index else { return None };
                let ConstantKind::Int(bits) = self.context.get(*id).kind else { return None };
                let Type::Int(width) = self.context.types.get(self.context.get(*id).ty) else { return None };
                indices.push(Some(signed(*width, bits)));
            }
            let (constant, _) = self.site.layout.collect_offset(&self.context.types, *source, &indices);
            offset += constant as i64;
            at = operation.operands[0];
        }
        Some((at, offset))
    }

    // ---- simplification ----------------------------------------------

    /// `simplifyInstruction` (:1820): fold the instruction when every operand
    /// is a constant here. D3: integers and `select`.
    fn simplify(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst);
        let Some(result) = operation.result else { return false };
        let mut operands = Vec::with_capacity(operation.operands.len());
        for operand in &operation.operands {
            match operand {
                Operand::Block(_) => operands.push(None),
                other => match self.constant(*other) {
                    Some(one) => operands.push(Some(one)),
                    None => return false,
                },
            }
        }
        let width_of = |ty| match self.context.types.get(ty) {
            Type::Int(width) => Some(*width),
            _ => None,
        };
        let int = |at: usize| match operands.get(at).copied().flatten() {
            Some(Simple::Int(width, bits)) => Some((width, bits)),
            _ => None,
        };
        let float = |at: usize| match operands.get(at).copied().flatten() {
            Some(Simple::Float(kind, bits)) => Some((kind, bits)),
            _ => None,
        };
        let folded = match &operation.opcode {
            Opcode::Binary(op) => {
                if let (Some((kind, a)), Some((_, b))) = (float(0), float(1)) {
                    fold_float(*op, kind, a, b).map(|bits| Simple::Float(kind, bits))
                } else {
                    let (width, a) = int(0).unwrap_or((0, 0));
                    let (_, b) = int(1).unwrap_or((0, 0));
                    if int(0).is_none() || int(1).is_none() {
                        return false;
                    }
                    fold_binary(*op, width, a, b).map(|bits| Simple::Int(width, bits))
                }
            }
            Opcode::FNeg => float(0).map(|(kind, bits)| Simple::Float(kind, negated(kind, bits))),
            Opcode::ICmp(predicate) => match (int(0), int(1)) {
                (Some((width, a)), Some((_, b))) => Some(Simple::Int(1, u128::from(compare(*predicate, width, a, b)))),
                _ => None,
            },
            Opcode::FCmp(predicate) => match (float(0), float(1)) {
                (Some((kind, a)), Some((_, b))) => {
                    Some(Simple::Int(1, u128::from(compare_float(*predicate, kind, a, b))))
                }
                _ => None,
            },
            Opcode::Cast(op) => {
                let to = width_of(operation.ty);
                let to_float = match self.context.types.get(operation.ty) {
                    Type::Float(kind) => Some(*kind),
                    _ => None,
                };
                let null = match operands.first().copied().flatten() {
                    Some(Simple::Pointer(bits)) => Some(bits),
                    _ => None,
                };
                match (op, int(0), float(0), to, to_float) {
                    (CastOp::PtrToInt, _, _, Some(to), _) if null.is_some() => {
                        Some(Simple::Int(to, mask(to, null.unwrap_or(0))))
                    }
                    (CastOp::Trunc, Some((_, bits)), _, Some(to), _) => Some(Simple::Int(to, mask(to, bits))),
                    (CastOp::ZExt, Some((_, bits)), _, Some(to), _) => Some(Simple::Int(to, bits)),
                    (CastOp::IntToPtr, Some((_, bits)), _, _, _) => Some(Simple::Pointer(bits)),
                    (CastOp::SExt, Some((from, bits)), _, Some(to), _) => {
                        Some(Simple::Int(to, mask(to, signed(from, bits) as u128)))
                    }
                    (CastOp::SIToFP, Some((from, bits)), _, _, Some(kind)) => {
                        Some(Simple::Float(kind, float_of_integer(kind, signed(from, bits))))
                    }
                    (CastOp::UIToFP, Some((_, bits)), _, _, Some(kind)) => {
                        i128::try_from(bits).ok().map(|value| Simple::Float(kind, float_of_integer(kind, value)))
                    }
                    (CastOp::FPToSI | CastOp::FPToUI, _, Some((kind, bits)), Some(to), _) => {
                        integer_of_float(kind, bits, to, matches!(op, CastOp::FPToSI)).map(|bits| Simple::Int(to, bits))
                    }
                    (CastOp::FPTrunc | CastOp::FPExt, _, Some((from, bits)), _, Some(kind)) => {
                        Some(Simple::Float(kind, converted_float(from, bits, kind)))
                    }
                    _ => None,
                }
            }
            Opcode::Select => match int(0) {
                Some((_, cond)) => operands[if cond & 1 == 1 { 1 } else { 2 }],
                None => None,
            },
            _ => None,
        };
        match folded {
            Some(one) => {
                self.simplified.insert(result, one);
                true
            }
            None => false,
        }
    }

    // ---- the walk -----------------------------------------------------

    /// `TTI.getInstructionCost(I, TCK_SizeAndLatency) == TCC_Free` for the
    /// generic target (TargetTransformInfoImpl.h:1464-1700, D2).
    fn free_instruction(
        &self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst);
        let layout = self.site.layout;
        let types = &self.context.types;
        match &operation.opcode {
            Opcode::Freeze | Opcode::ExtractValue(_) => true,
            Opcode::GetElementPtr { .. } => self.gep_free(inst),
            Opcode::Cast(op) if self.site.tti.is_some() => {
                // The target's own price: free when it says 0 under
                // `TCK_SizeAndLatency` (a type it does not model is not free).
                let from = self.operand_type(operation.operands[0]);
                let kind = match op {
                    CastOp::Trunc => Some(CastKind::Trunc),
                    CastOp::ZExt => Some(CastKind::ZExt),
                    CastOp::SExt => Some(CastKind::SExt),
                    CastOp::FPTrunc => Some(CastKind::FPTrunc),
                    CastOp::FPExt => Some(CastKind::FPExt),
                    CastOp::FPToUI => Some(CastKind::FPToUI),
                    CastOp::FPToSI => Some(CastKind::FPToSI),
                    CastOp::UIToFP => Some(CastKind::UIToFP),
                    CastOp::SIToFP => Some(CastKind::SIToFP),
                    CastOp::PtrToInt => Some(CastKind::PtrToInt),
                    CastOp::IntToPtr => Some(CastKind::IntToPtr),
                    CastOp::BitCast => Some(CastKind::BitCast),
                    CastOp::FPRound | CastOp::AddrSpaceCast => None,
                };
                match (self.site.tti, kind, from.and_then(|ty| Ty::of(types, ty)), Ty::of(types, operation.ty)) {
                    (Some(tti), Some(kind), Some(src), Some(dst)) => {
                        let cost = tti.cast_cost(kind, dst, src, CostKind::SizeAndLatency);
                        // BasicTTIImpl.h:1298-1311: the extension of a simple
                        // load.
                        let of_load = match operation.operands[0] {
                            Operand::Value(value) => match self.callee.value(value).def {
                                llrm_mir::module::ValueDef::Instruction(def) => {
                                    matches!(
                                        self.callee.instruction(def).opcode,
                                        Opcode::Load { volatile: false, .. }
                                    )
                                }
                                llrm_mir::module::ValueDef::Argument(_) => false,
                            },
                            _ => false,
                        };
                        (cost.valid && cost.value == 0) || (of_load && tti.ext_load_free(kind, dst, src))
                    }
                    _ => false,
                }
            }
            Opcode::Cast(op) => {
                let from = self.operand_type(operation.operands[0]);
                let (source, target) = (
                    from.map_or(0, |ty| layout.size_bits(types, ty)) as u32,
                    layout.size_bits(types, operation.ty) as u32,
                );
                match op {
                    CastOp::IntToPtr => layout.legal_integer(source) && source <= self.pointer_bits_of(operation.ty),
                    CastOp::PtrToInt => {
                        layout.legal_integer(target) && from.is_some_and(|ty| target >= self.pointer_bits_of(ty))
                    }
                    CastOp::BitCast => {
                        from == Some(operation.ty)
                            || (matches!(types.get(operation.ty), Type::Pointer(_))
                                && from.is_some_and(|ty| matches!(types.get(ty), Type::Pointer(_))))
                    }
                    CastOp::Trunc => layout.legal_integer(target),
                    _ => false,
                }
            }
            Opcode::Call(_) | Opcode::Invoke(_) => {
                let target = llrm_mir::memory::callee(self.context, self.callee, inst);
                target.is_some_and(|one| free_intrinsic(self.site.module.global(one).name.as_deref()))
            }
            _ => false,
        }
    }

    /// The bits of a pointer of type `ty`: its address space's.
    fn pointer_bits_of(
        &self,
        ty: llrm_mir::types::TypeId,
    ) -> u32 {
        match self.context.types.get(ty) {
            Type::Pointer(space) => self.site.layout.pointer(*space).bits as u32,
            _ => 0,
        }
    }

    /// The type of an operand of the callee.
    fn operand_type(
        &self,
        operand: Operand,
    ) -> Option<llrm_mir::types::TypeId> {
        match operand {
            Operand::Value(value) => Some(self.callee.value(value).ty),
            Operand::Constant(id) => Some(self.context.get(id).ty),
            Operand::Block(_) => None,
        }
    }

    /// `visitInstruction` (:2722): free to the TTI, or SROA is lost through
    /// every operand.
    fn visit_default(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.free_instruction(inst) {
            return true;
        }
        let operands = self.callee.instruction(inst).operands.clone();
        for operand in operands {
            self.disable_sroa(operand);
        }
        false
    }

    /// `Base::visit(&I)` for the instruction: whether it cost nothing.
    fn visit(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        match &operation.opcode {
            Opcode::Alloca { .. } => self.visit_alloca(inst),
            Opcode::Phi => self.visit_phi(inst),
            Opcode::GetElementPtr { .. } => self.visit_gep(inst),
            Opcode::Cast(CastOp::BitCast) => self.visit_bitcast(inst),
            Opcode::Cast(CastOp::PtrToInt) => self.visit_ptr_to_int(inst),
            Opcode::Cast(CastOp::IntToPtr) => self.visit_int_to_ptr(inst),
            Opcode::Cast(_) => self.visit_cast(inst),
            Opcode::ICmp(_) | Opcode::FCmp(_) => self.visit_cmp(inst),
            Opcode::Binary(BinaryOp::Sub) => self.visit_sub(inst),
            Opcode::Binary(_) => self.visit_binary(inst),
            Opcode::FNeg => self.visit_fneg(inst),
            Opcode::Load { volatile, .. } => self.visit_load(inst, *volatile),
            Opcode::Store { volatile, .. } => self.visit_store(inst, *volatile),
            // `LoadInst` and `StoreInst` that are not simple,
            // then `visitInstruction`.
            Opcode::Atomic(info) => match info.kind {
                AtomicKind::Load => self.visit_load(inst, true),
                AtomicKind::Store => self.visit_store(inst, true),
                _ => self.visit_default(inst),
            },
            Opcode::ExtractValue(_) | Opcode::InsertValue(_) => self.visit_aggregate(inst),
            Opcode::Call(_) | Opcode::Invoke(_) => self.visit_call(inst),
            Opcode::Ret => {
                let free = !self.has_return;
                self.has_return = true;
                free
            }
            Opcode::Br => {
                if operation.operands.len() == 1 {
                    return true;
                }
                self.constant_int(operation.operands[0]).is_some()
            }
            Opcode::Select => self.visit_select(inst),
            Opcode::Switch => self.visit_switch(inst),
            Opcode::Resume => false,
            Opcode::Unreachable => true,
            Opcode::LandingPad { .. } | Opcode::Freeze => self.visit_default(inst),
        }
    }

    /// `visitAlloca` (:1575).
    fn visit_alloca(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst);
        let Opcode::Alloca { allocated, .. } = &operation.opcode else { return false };
        let base = self.site.layout.alloc_size(&self.context.types, *allocated);
        // The array size operand, when written.
        if let Some(count) = operation.operands.first() {
            if let Some((_, size)) = self.constant_int(*count) {
                self.allocated_size = (size as u64).saturating_mul(base).saturating_add(self.allocated_size);
                if self.allocated_size > MAX_SIMPLIFIED_DYNAMIC_ALLOCA_TO_INLINE {
                    self.has_dynamic_alloca = true;
                }
                return false;
            }
        }
        let is_static = match operation.operands.first() {
            None => true,
            Some(count) => matches!(count, Operand::Constant(_)),
        } && self.in_entry_block(inst);
        if is_static {
            self.allocated_size = self.allocated_size.saturating_add(base);
        } else {
            self.has_dynamic_alloca = true;
        }
        false
    }

    fn in_entry_block(
        &self,
        inst: InstId,
    ) -> bool {
        self.callee.layout().first().is_some_and(|&entry| self.callee.parent(inst) == Some(entry))
    }

    /// `visitPHI` (:1618).
    fn visit_phi(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return true };
        let check_sroa = matches!(self.context.types.get(operation.ty), Type::Pointer(_));
        let home = self.callee.parent(inst);
        let mut first_constant: Option<Simple> = None;
        let mut first_pointer: Option<(Operand, i64)> = None;
        let mut first_value: Option<Operand> = None;
        for pair in operation.operands.chunks(2) {
            let (value, Operand::Block(predecessor)) = (pair[0], pair[1]) else { continue };
            if self.dead_blocks.contains(&predecessor.0) {
                continue;
            }
            if let Some(known) = self.known_successors.get(&predecessor.0)
                && Some(*known) != home
            {
                continue;
            }
            if value == Operand::Value(result) {
                continue;
            }
            let constant = self.constant(value);
            let pointer = if constant.is_none() && check_sroa {
                match value {
                    Operand::Value(v) => self.offsets.get(&v).copied(),
                    _ => None,
                }
            } else {
                None
            };
            if constant.is_none() && pointer.is_none() {
                return true;
            }
            if let Some(first) = first_constant {
                if constant == Some(first) {
                    continue;
                }
                return true;
            }
            if first_value.is_some() {
                if first_pointer == pointer {
                    continue;
                }
                return true;
            }
            if constant.is_some() {
                first_constant = constant;
                continue;
            }
            first_value = Some(value);
            first_pointer = pointer;
        }
        if let Some(one) = first_constant {
            self.simplified.insert(result, one);
            return true;
        }
        if let Some(pointer) = first_pointer {
            self.offsets.insert(result, pointer);
            if let Some(alloca) = first_value.and_then(|value| self.sroa_arg(value)) {
                self.sroa_values.insert(result, alloca);
            }
        }
        true
    }

    /// `visitGetElementPtr` (:1732).
    fn visit_gep(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return false };
        let sroa = self.sroa_arg(operation.operands[0]);
        let constant_indices = operation.operands[1..].iter().all(|one| self.constant(*one).is_some());
        // simplifyInstruction needs every operand constant, the base too.
        if self.simplify(inst) {
            return true;
        }
        let inbounds = operation.flags.contains(llrm_mir::opcode::Flags::INBOUNDS);
        let folded = inbounds && self.can_fold_inbounds_gep(inst, result);
        if folded || constant_indices {
            if let Some(alloca) = sroa {
                self.sroa_values.insert(result, alloca);
            }
            return true;
        }
        if let Some(alloca) = sroa {
            self.disable_sroa_for(alloca);
        }
        self.gep_free(inst)
    }

    /// `canFoldInboundsGEP` (:1714).
    fn can_fold_inbounds_gep(
        &mut self,
        inst: InstId,
        result: ValueId,
    ) -> bool {
        let base = self.callee.instruction(inst).operands[0];
        let Operand::Value(pointer) = base else { return false };
        let Some((root, offset)) = self.offsets.get(&pointer).copied() else { return false };
        let Some(add) = self.gep_offset(inst) else { return false };
        self.offsets.insert(result, (root, offset + add));
        true
    }

    /// `visitBitCast` (:1867).
    fn visit_bitcast(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.simplify(inst) {
            return true;
        }
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return true };
        if let Operand::Value(source) = operation.operands[0]
            && let Some(pointer) = self.offsets.get(&source).copied()
        {
            self.offsets.insert(result, pointer);
        }
        if let Some(alloca) = self.sroa_arg(operation.operands[0]) {
            self.sroa_values.insert(result, alloca);
        }
        true
    }

    /// `visitPtrToInt` (:1887).
    fn visit_ptr_to_int(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.simplify(inst) {
            return true;
        }
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return true };
        let width = match self.context.types.get(operation.ty) {
            Type::Int(width) => *width,
            _ => 0,
        };
        let source_bits = self.operand_type(operation.operands[0]).map_or(0, |ty| self.pointer_bits_of(ty));
        if width == source_bits {
            if let Operand::Value(source) = operation.operands[0]
                && let Some(pointer) = self.offsets.get(&source).copied()
            {
                self.offsets.insert(result, pointer);
            }
        }
        if let Some(alloca) = self.sroa_arg(operation.operands[0]) {
            self.sroa_values.insert(result, alloca);
        }
        self.free_instruction(inst)
    }

    /// `visitIntToPtr` (:1917).
    fn visit_int_to_ptr(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.simplify(inst) {
            return true;
        }
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return true };
        let source = operation.operands[0];
        if let Operand::Value(value) = source
            && let Some(pointer) = self.offsets.get(&value).copied()
        {
            self.offsets.insert(result, pointer);
        }
        if let Some(alloca) = self.sroa_arg(source) {
            self.sroa_values.insert(result, alloca);
        }
        self.free_instruction(inst)
    }

    /// `visitCastInst` (:1940).
    fn visit_cast(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.simplify(inst) {
            return true;
        }
        let operation = self.callee.instruction(inst).clone();
        self.disable_sroa(operation.operands[0]);
        // D2: a floating-point cast is a library call on a target with no FPU.
        if matches!(
            operation.opcode,
            Opcode::Cast(
                CastOp::FPTrunc | CastOp::FPExt | CastOp::UIToFP | CastOp::SIToFP | CastOp::FPToUI | CastOp::FPToSI
            )
        ) && self.fp_expensive()
        {
            self.add_cost(CALL_PENALTY);
        }
        self.free_instruction(inst)
    }

    /// `getFPOpCost == TCC_Expensive` (D2): the generic answer, an FP type the
    /// hardware does not have; x86 with an x87 has them all.
    fn fp_expensive(&self) -> bool {
        false
    }

    /// `visitCmpInst` (:2206).
    fn visit_cmp(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        let (lhs, rhs) = (operation.operands[0], operation.operands[1]);
        if self.simplify(inst) {
            return true;
        }
        // D6: simplifyCmpInstForRecCall.
        if matches!(operation.opcode, Opcode::FCmp(_)) {
            return false;
        }
        let Some(result) = operation.result else { return false };
        if let Operand::Value(left) = lhs
            && let Some((base, left_offset)) = self.offsets.get(&left).copied()
            && let Operand::Value(right) = rhs
            && let Some((other, right_offset)) = self.offsets.get(&right).copied()
            && base == other
            && let Opcode::ICmp(predicate) = operation.opcode
        {
            let bits = compare(predicate, 64, left_offset as u64 as u128, right_offset as u64 as u128);
            self.simplified.insert(result, Simple::Int(1, u128::from(bits)));
            return true;
        }
        let null_rhs = matches!(
            rhs,
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Null)
        );
        if matches!(
            operation.opcode,
            Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
        ) && null_rhs {
            if self.known_non_null(lhs) {
                let not_equal = matches!(operation.opcode, Opcode::ICmp(IntPredicate::Ne));
                self.simplified.insert(result, Simple::Int(1, u128::from(not_equal)));
                return true;
            }
        }
        self.handle_sroa(lhs, null_rhs)
    }

    /// `isKnownNonNullInCallee` (:1974).
    fn known_non_null(
        &self,
        operand: Operand,
    ) -> bool {
        if let Operand::Value(value) = operand {
            if let Some(index) = self.callee.parameters().iter().position(|one| *one == value) {
                let attrs = call_info(self.site.caller, self.site.call).and_then(|info| info.argument_attrs.get(index));
                if attrs.is_some_and(|attrs| has_flag(attrs, "nonnull")) {
                    return true;
                }
            }
            return self.sroa_values.contains_key(&value);
        }
        false
    }

    /// `visitSub` (:2262).
    fn visit_sub(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        if let (Operand::Value(left), Operand::Value(right)) = (operation.operands[0], operation.operands[1])
            && let Some((base, a)) = self.offsets.get(&left).copied()
            && let Some((other, b)) = self.offsets.get(&right).copied()
            && base == other
            && let (Some(result), Type::Int(width)) = (operation.result, self.context.types.get(operation.ty))
        {
            self.simplified.insert(result, Simple::Int(*width, mask(*width, (a - b) as u128)));
            return true;
        }
        self.visit_binary(inst)
    }

    /// `visitBinaryOperator` (:2289).
    fn visit_binary(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        // simplifyBinOp also settles x+0, x*1: the identities with one
        // constant operand (D3).
        if self.simplify(inst) || self.identity(inst) {
            return true;
        }
        self.disable_sroa(operation.operands[0]);
        self.disable_sroa(operation.operands[1]);
        if matches!(self.context.types.get(operation.ty), Type::Float(_)) && self.fp_expensive() {
            self.add_cost(CALL_PENALTY);
        }
        false
    }

    /// The algebraic identities `simplifyBinOp` finds with one constant
    /// operand (D3): `x op 0`, `x * 1`, `x & -1`, `x * 0`, `x & 0`.
    fn identity(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst);
        let Opcode::Binary(op) = operation.opcode else { return false };
        let Some(result) = operation.result else { return false };
        let (a, b) = (self.constant_int(operation.operands[0]), self.constant_int(operation.operands[1]));
        let Type::Int(width) = *self.context.types.get(operation.ty) else { return false };
        let all = mask(width, u128::MAX);
        let is = |one: Option<(u32, u128)>, value: u128| one.is_some_and(|(_, bits)| bits == value);
        let zero_result = match op {
            BinaryOp::Mul | BinaryOp::And => is(a, 0) || is(b, 0),
            _ => false,
        };
        if zero_result {
            self.simplified.insert(result, Simple::Int(width, 0));
            return true;
        }
        match op {
            BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor => is(a, 0) || is(b, 0),
            BinaryOp::Sub | BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => is(b, 0),
            BinaryOp::Mul => is(a, 1) || is(b, 1),
            BinaryOp::And => is(a, all) || is(b, all),
            BinaryOp::UDiv | BinaryOp::SDiv => is(b, 1),
            _ => false,
        }
    }

    /// `visitFNeg` (:2324).
    fn visit_fneg(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operand = self.callee.instruction(inst).operands[0];
        self.disable_sroa(operand);
        false
    }

    /// `visitLoad` (:2343).
    fn visit_load(
        &mut self,
        inst: InstId,
        volatile: bool,
    ) -> bool {
        let pointer = self.callee.instruction(inst).operands[0];
        if self.handle_sroa(pointer, !volatile) {
            return true;
        }
        if self.enable_load_elimination && !self.load_addrs.insert(pointer) && !volatile {
            // onLoadEliminationOpportunity (:1149)
            self.load_elimination_cost += INSTR_COST;
            return true;
        }
        self.add_cost(MEM_ACCESS_COST);
        false
    }

    /// `visitStore` (:2360).
    fn visit_store(
        &mut self,
        inst: InstId,
        volatile: bool,
    ) -> bool {
        let pointer = self.callee.instruction(inst).operands[1];
        if self.handle_sroa(pointer, !volatile) {
            return true;
        }
        self.disable_load_elimination();
        self.add_cost(MEM_ACCESS_COST);
        false
    }

    /// `visitExtractValue` / `visitInsertValue` (:2378, :2396).
    fn visit_aggregate(
        &mut self,
        inst: InstId,
    ) -> bool {
        if self.simplify(inst) {
            return true;
        }
        self.visit_default(inst)
    }

    /// `visitCallBase` (:2473).
    fn visit_call(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        let info = call_info(self.callee, inst).expect("a call").clone();
        let arguments = operation.operands.len().saturating_sub(1);
        if has_flag(&info.attrs, "returns_twice") && !has_flag(&self.callee.attrs, "returns_twice") {
            self.exposes_returns_twice = true;
            return false;
        }
        if matches!(operation.opcode, Opcode::Call(_)) && has_flag(&info.attrs, "noduplicate") {
            self.contains_noduplicate_call = true;
        }
        let target = llrm_mir::memory::callee(self.context, self.callee, inst);
        let Some(target) = target else {
            // Indirect: argument setup, the memory it may write.
            self.add_cost(arguments as i64 * INSTR_COST);
            if !only_reads_memory(&info) {
                self.disable_load_elimination();
            }
            return self.visit_default(inst);
        };
        // `F == Call.getCalledFunction()`: "This flag will fully abort the
        // analysis" unless the params allow a recursive call (:2481).
        if target == self.site.callee_id
            && (!self.params.allow_recursive_call
                || crate::inline::gcc_insns(self.callee) > self.params.recursive_callee_limit)
        {
            self.is_recursive_call = true;
            return false;
        }
        // simplifyCallSite: D3, no library folding.
        let name = self.site.module.global(target).name.clone();
        // A declaration flagged `frame_bound` (the QB error-handler
        // registration): its effect belongs to the frame that runs it, so the
        // callee is not copied into another frame (the `returns_twice` rule of
        // `visitCallBase`, InlineCost.cpp:2473).
        if frame_bound(self.site.module, target) {
            self.has_uninlineable_intrinsic = true;
            return false;
        }
        if let Some(name) = name.as_deref().filter(|name| name.starts_with("llvm.")) {
            let family = |prefix: &str| name.starts_with(prefix);
            if family("llvm.load.relative") {
                self.add_cost(3 * INSTR_COST);
                return false;
            }
            if family("llvm.memset") || family("llvm.memcpy") || family("llvm.memmove") {
                self.disable_load_elimination();
                return false;
            }
            if family("llvm.icall.branch.funnel") || family("llvm.localescape") {
                self.has_uninlineable_intrinsic = true;
                return false;
            }
            if family("llvm.va_start") {
                self.inits_vararg = true;
                return false;
            }
            if family("llvm.launder.invariant.group") {
                if let Some(alloca) = self.sroa_arg(operation.operands[0])
                    && let Some(result) = operation.result
                {
                    self.sroa_values.insert(result, alloca);
                }
                return true;
            }
            if family("llvm.is.constant") {
                if let Some(result) = operation.result {
                    let known = self.constant(operation.operands[0]).is_some();
                    self.simplified.insert(result, Simple::Int(1, u128::from(known)));
                }
                return true;
            }
            if !only_reads_memory(&info) && !free_intrinsic(Some(name)) {
                self.disable_load_elimination();
            }
            return self.visit_default(inst);
        }
        if lowered_to_call(self.site.module, target) {
            // onLoweredCall (:732), a direct call.
            self.add_cost(arguments as i64 * INSTR_COST);
            self.add_cost(CALL_PENALTY);
        }
        if !only_reads_memory(&info) {
            self.disable_load_elimination();
        }
        self.visit_default(inst)
    }

    /// `visitSelectInst` (:2585).
    fn visit_select(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        let Some(result) = operation.result else { return false };
        let check_sroa = matches!(self.context.types.get(operation.ty), Type::Pointer(_));
        let (condition, yes, no) = (operation.operands[0], operation.operands[1], operation.operands[2]);
        let (yes_constant, no_constant) = (self.constant(yes), self.constant(no));
        let condition_constant = self.constant_int(condition);
        let Some((_, condition_bits)) = condition_constant else {
            if yes_constant.is_some() && yes_constant == no_constant {
                self.simplified.insert(result, yes_constant.expect("some"));
                return true;
            }
            if !check_sroa {
                return self.visit_default(inst);
            }
            let (a, b) = (self.pointer_of(yes), self.pointer_of(no));
            if a.is_some() && a == b {
                self.offsets.insert(result, a.expect("some"));
                if let Some(alloca) = self.sroa_arg(yes) {
                    self.sroa_values.insert(result, alloca);
                }
                return true;
            }
            return self.visit_default(inst);
        };
        let selected = if condition_bits & 1 == 1 { yes } else { no };
        if let Some(one) = self.constant(selected) {
            self.simplified.insert(result, one);
            return true;
        }
        if !check_sroa {
            return true;
        }
        if let Some(pointer) = self.pointer_of(selected) {
            self.offsets.insert(result, pointer);
            if let Some(alloca) = self.sroa_arg(selected) {
                self.sroa_values.insert(result, alloca);
            }
        }
        true
    }

    fn pointer_of(
        &self,
        operand: Operand,
    ) -> Option<(Operand, i64)> {
        match operand {
            Operand::Value(value) => self.offsets.get(&value).copied(),
            _ => None,
        }
    }

    /// `visitSwitchInst` (:2657) with `onFinalizeSwitch` (:762).
    fn visit_switch(
        &mut self,
        inst: InstId,
    ) -> bool {
        let operation = self.callee.instruction(inst).clone();
        if self.constant_int(operation.operands[0]).is_some() {
            return true;
        }
        let cases: Vec<i128> = operation.operands[2..]
            .chunks(2)
            .filter_map(|pair| match pair[0] {
                Operand::Constant(id) => {
                    match (&self.context.get(id).kind, self.context.types.get(self.context.get(id).ty)) {
                        (ConstantKind::Int(bits), Type::Int(width)) => Some(signed(*width, *bits)),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();
        let for_size = self.site.caller_size != CallerSize::Speed;
        let index_bits = self.site.layout.pointer(0).bits;
        let (clusters, table) = match self.site.tti {
            Some(tti) => tti.estimated_number_of_case_clusters(&cases, index_bits, for_size),
            None if self.params.tti == Tti::Generic => (cases.len() as u64, 0),
            None => llrm_mir::tti::basic_case_clusters(&cases, index_bits, for_size),
        };
        let default_unreachable = match operation.operands[1] {
            Operand::Block(default) => self
                .callee
                .terminator(default)
                .is_some_and(|last| matches!(self.callee.instruction(last).opcode, Opcode::Unreachable)),
            _ => false,
        };
        let unreachable = i64::from(default_unreachable);
        if table > 0 {
            if !default_unreachable {
                self.add_cost(2 * INSTR_COST);
            }
            self.add_cost(table as i64 * INSTR_COST + 2 * INSTR_COST);
        } else if clusters <= 3 {
            self.add_cost((clusters as i64 - unreachable) * 2 * INSTR_COST);
        } else {
            let compares = 3 * clusters as i64 / 2 - 1;
            self.add_cost(compares * 2 * INSTR_COST);
        }
        false
    }

    // ---- the analysis ---------------------------------------------------

    /// `findDeadBlocks` (:2876).
    fn find_dead_blocks(
        &mut self,
        current: BlockId,
        next: BlockId,
    ) {
        let callee = self.callee;
        let edge_dead = |dead: &BTreeSet<u32>, known: &HashMap<u32, BlockId>, from: BlockId, to: BlockId| {
            dead.contains(&from.0) || known.get(&from.0).is_some_and(|one| *one != to)
        };
        let newly_dead = |dead: &BTreeSet<u32>, known: &HashMap<u32, BlockId>, block: BlockId| {
            !dead.contains(&block.0)
                && callee.predecessors(block).iter().all(|&from| edge_dead(dead, known, from, block))
        };
        for successor in callee.successors(current) {
            if successor == next || !newly_dead(&self.dead_blocks, &self.known_successors, successor) {
                continue;
            }
            let mut pending = vec![successor];
            while let Some(dead) = pending.pop() {
                if self.dead_blocks.insert(dead.0) {
                    for follower in callee.successors(dead) {
                        if newly_dead(&self.dead_blocks, &self.known_successors, follower) {
                            pending.push(follower);
                        }
                    }
                }
            }
        }
    }

    /// `shouldStop` (:1138).
    fn should_stop(&mut self) -> bool {
        if self.params.compute_full_inline_cost || self.cost < self.threshold {
            return false;
        }
        self.decided_by_threshold = true;
        true
    }

    /// `analyzeBlock` (:2745).
    fn analyze_block(
        &mut self,
        block: BlockId,
    ) -> Result<(), String> {
        for inst in self.callee.iter_block(block) {
            // D5: no ephemeral values; a debug intrinsic costs nothing and is
            // not counted (:2750).
            if let Some(target) = llrm_mir::memory::callee(self.context, self.callee, inst)
                && self.site.module.global(target).name.as_deref().is_some_and(|name| name.starts_with("llvm.dbg."))
            {
                continue;
            }
            let before = self.cost;
            self.instructions += 1;
            if matches!(
                self.context.types.get(self.callee.instruction(inst).ty),
                Type::Vector { .. }
            ) {
                self.vector_instructions += 1;
            }
            if !self.visit(inst) {
                // onMissedSimplification (:835)
                self.add_cost(INSTR_COST);
                self.size_units += self.code_size_of(inst);
            }
            if llrm_support::env_flag!("LLRM_EXP_INLINETRACE") {
                eprintln!(
                    "[inlinetrace] {} {} {}",
                    self.site.module.global(self.site.callee_id).name.as_deref().unwrap_or("?"),
                    match llrm_mir::memory::callee(self.context, self.callee, inst) {
                        Some(target) => format!(
                            "call:{}",
                            self.site.module.global(target).name.as_deref().unwrap_or("?").trim_matches('"')
                        ),
                        None => self.callee.instruction(inst).opcode.mnemonic().to_owned(),
                    },
                    self.cost - before
                );
            }
            if self.is_recursive_call {
                return Err("recursive".into());
            } else if self.exposes_returns_twice {
                return Err("exposes returns twice".into());
            } else if self.has_dynamic_alloca {
                return Err("dynamic alloca".into());
            } else if self.has_indirect_br {
                return Err("indirect branch".into());
            } else if self.has_uninlineable_intrinsic {
                return Err("uninlinable intrinsic".into());
            } else if self.inits_vararg {
                return Err("varargs".into());
            }
            if self.site.caller_recursive && self.allocated_size > TOTAL_ALLOCA_SIZE_RECURSIVE_CALLER {
                return Err("recursive and allocates too much stack space".into());
            }
            if self.should_stop() {
                return Err("Call site analysis is not favorable to inlining.".into());
            }
        }
        Ok(())
    }

    /// `updateThreshold` (:2073) and `onAnalysisStart` (:1153).
    fn start(&mut self) -> Result<(), String> {
        // updateThreshold
        let allow_growth = self.allow_size_growth();
        if !allow_growth {
            self.threshold = 0;
        } else {
            let mut single_bb_percent = 50;
            let mut vector_percent = VECTOR_BONUS_PERCENT;
            let mut last_call_bonus = if self.params.last_call_to_static { LAST_CALL_TO_STATIC_BONUS } else { 0 };
            let caller_size = self.site.caller_size;
            match caller_size {
                CallerSize::MinSize => {
                    self.threshold = self.threshold.min(self.params.opt_min_size_threshold);
                    single_bb_percent = 0;
                    vector_percent = 0;
                }
                CallerSize::OptSize => self.threshold = self.threshold.min(self.params.opt_size_threshold),
                CallerSize::Speed => {}
            }
            if caller_size != CallerSize::MinSize {
                let hint = if caller_size == CallerSize::OptSize {
                    self.params.opt_size_hint_threshold
                } else {
                    self.params.hint_threshold
                };
                if has_flag(&self.callee.attrs, "inlinehint") {
                    self.threshold = self.threshold.max(hint);
                }
                let frequency = self.site.relative_frequency;
                let locally_hot = self
                    .params
                    .locally_hot_callsite_threshold
                    .filter(|_| frequency.is_some_and(|ratio| ratio >= HOT_CALLSITE_REL_FREQ));
                if caller_size != CallerSize::OptSize && locally_hot.is_some() {
                    self.threshold = locally_hot.expect("some");
                } else if frequency.is_some_and(|ratio| ratio < COLD_CALLSITE_REL_FREQ / 100.0) {
                    single_bb_percent = 0;
                    vector_percent = 0;
                    last_call_bonus = 0;
                    self.threshold = self.threshold.min(self.params.cold_callsite_threshold);
                }
            }
            // adjustInliningThreshold and the multiplier: 0 and 1 (D2).
            self.single_bb_bonus = self.threshold * single_bb_percent / 100;
            self.vector_bonus = self.threshold * vector_percent / 100;
            if self.site.sole_call_to_local {
                self.add_cost(-last_call_bonus);
                self.static_bonus_applied = last_call_bonus;
            }
        }
        // onAnalysisStart
        self.threshold += self.single_bb_bonus + self.vector_bonus;
        let call_cost = callsite_cost(self.site);
        self.add_cost(-call_cost);
        if self.callee.calling_convention == COLD_CC {
            self.add_cost(COLDCC_PENALTY);
        }
        if self.cost >= self.threshold && !self.params.compute_full_inline_cost {
            return Err("high cost".into());
        }
        Ok(())
    }

    /// `allowSizeGrowth` (:1996).
    fn allow_size_growth(&self) -> bool {
        let caller = self.site.caller;
        let call = caller.instruction(self.site.call);
        let unreachable = |block: Option<BlockId>| {
            block
                .and_then(|one| caller.terminator(one))
                .is_some_and(|last| matches!(caller.instruction(last).opcode, Opcode::Unreachable))
        };
        match call.opcode {
            Opcode::Invoke(_) => match call.operands.iter().rev().find_map(|one| match one {
                Operand::Block(block) => Some(*block),
                _ => None,
            }) {
                // The normal destination is the first block operand.
                _ => {
                    let normal = call
                        .operands
                        .iter()
                        .find_map(
                            |one| match one {
                                Operand::Block(block) => Some(*block),
                                _ => None,
                            },
                        );
                    !unreachable(normal)
                }
            },
            _ => !unreachable(caller.parent(self.site.call)),
        }
    }

    /// `analyze` (:2916) and `finalizeAnalysis` (:1080).
    fn analyze(&mut self) -> Result<(), String> {
        if llrm_support::env_flag!("LLRM_EXP_INLINETRACE") {
            eprintln!(
                "[inlinetrace-start] {} <- {}",
                self.site.module.global(self.site.callee_id).name.as_deref().unwrap_or("?"),
                self.site.module.global(self.site.caller_id).name.as_deref().unwrap_or("?")
            );
            eprint!("{}", llrm_mir::print::body(self.context, self.callee));
        }
        self.start()?;
        if self.callee.layout().is_empty() {
            return Ok(());
        }
        // Arguments: simplified values, constant-offset pointers, SROA allocas.
        let call = self.site.caller.instruction(self.site.call);
        for (index, &parameter) in self.callee.parameters().iter().enumerate() {
            let Some(&argument) = call.operands.get(index) else { break };
            let known = match argument {
                Operand::Constant(_) => self.constant(argument).unwrap_or(Simple::Caller(argument)),
                other => Simple::Caller(other),
            };
            self.simplified.insert(parameter, known);
            // `stripAndComputeInBoundsConstantOffsets` (:2786) is of a pointer.
            let argument_type = match argument {
                Operand::Value(v) => Some(self.site.caller.value(v).ty),
                Operand::Constant(id) => Some(self.context.get(id).ty),
                Operand::Block(_) => None,
            };
            let pointer = argument_type.is_some_and(|ty| matches!(self.context.types.get(ty), Type::Pointer(_)));
            if pointer && let Some((base, offset)) = self.strip_offsets(argument) {
                {
                    self.offsets.insert(parameter, (base, offset));
                    if let Operand::Value(root) = base
                        && let ValueDef::Instruction(def) = self.site.caller.value(root).def
                        && matches!(
                            self.site.caller.instruction(def).opcode,
                            Opcode::Alloca { .. }
                        )
                    {
                        // onInitializeSROAArg (:837): getCallerAllocaCost is 0
                        // (D2).
                        self.sroa_values.insert(parameter, root);
                        self.enabled_sroa.insert(root);
                        self.sroa_arg_costs.insert(root, 0);
                    }
                }
            }
        }
        let entry = self.callee.layout()[0];
        let mut worklist = vec![entry];
        let mut queued = IdSet::<u32>::new();
        queued.insert(entry.0);
        let mut at = 0;
        while at < worklist.len() {
            if self.should_stop() {
                break;
            }
            let block = worklist[at];
            at += 1;
            if self.callee.block(block).is_empty() {
                continue;
            }
            // onBlockStart: CostAtBBStart unused without cost-benefit (D4).
            self.analyze_block(block)?;
            let terminator = self.callee.terminator(block);
            let mut folded = None;
            if let Some(last) = terminator {
                let operation = self.callee.instruction(last);
                match operation.opcode {
                    Opcode::Br if operation.operands.len() == 3 => {
                        if let Some((_, condition)) = self.constant_int(operation.operands[0]) {
                            if let Operand::Block(next) = operation.operands[if condition & 1 == 1 { 1 } else { 2 }] {
                                folded = Some(next);
                            }
                        }
                    }
                    Opcode::Switch => {
                        if let Some((width, value)) = self.constant_int(operation.operands[0]) {
                            let chosen = operation.operands[2..].chunks(2).find_map(|pair| match (pair[0], pair[1]) {
                                (Operand::Constant(id), Operand::Block(target))
                                    if self.constant(Operand::Constant(id)) == Some(Simple::Int(width, value)) =>
                                {
                                    Some(target)
                                }
                                _ => None,
                            });
                            folded = chosen.or(match operation.operands[1] {
                                Operand::Block(default) => Some(default),
                                _ => None,
                            });
                        }
                    }
                    _ => {}
                }
            }
            if let Some(next) = folded {
                if queued.insert(next.0) {
                    worklist.push(next);
                }
                self.known_successors.insert(block.0, next);
                self.find_dead_blocks(block, next);
                continue;
            }
            for successor in self.callee.successors(block) {
                if queued.insert(successor.0) {
                    worklist.push(successor);
                }
            }
            // onBlockAnalyzed (:855)
            if self.single_bb && self.callee.successors(block).len() > 1 {
                self.threshold -= self.single_bb_bonus;
                self.single_bb = false;
            }
        }
        if !self.site.sole_call_to_local && self.contains_noduplicate_call {
            return Err("noduplicate".into());
        }
        self.finalize()
    }

    /// The target's `CostKind::CodeSize` of `inst` of the callee.
    fn code_size_of(
        &self,
        inst: InstId,
    ) -> i64 {
        let Some(tti) = self.site.tti else { return 0 };
        llrm_mir::instructioncost::instruction_cost(
            tti,
            self.context,
            self.site.layout,
            &self.site.module.globals,
            self.callee,
            inst,
            llrm_mir::tti::CostKind::CodeSize,
        )
        .value
    }

    /// The target's code size of the call and the pushes of its arguments,
    /// where the target says a call is bigger than the instruction it is
    /// counted as.
    fn call_size(&self) -> i64 {
        let Some(tti) = self.site.tti else { return 0 };
        let size = llrm_mir::instructioncost::instruction_cost(
            tti,
            self.context,
            self.site.layout,
            &self.site.module.globals,
            self.site.caller,
            self.site.call,
            llrm_mir::tti::CostKind::CodeSize,
        )
        .value;
        (size * tti.call_code_size_percent() + 50) / 100
    }

    fn finalize(&mut self) -> Result<(), String> {
        if self.site.caller_size == CallerSize::MinSize {
            let loops = loop_headers(self.callee).iter().filter(|header| !self.dead_blocks.contains(&header.0)).count();
            self.add_cost(loops as i64 * LOOP_PENALTY);
        }
        if self.vector_instructions <= self.instructions / 10 {
            self.threshold -= self.vector_bonus;
        } else if self.vector_instructions <= self.instructions / 2 {
            self.threshold -= self.vector_bonus / 2;
        }
        // D11: the analysis above stops at LLVM's threshold; at -Os the verdict
        // is also held to the callee's code size against the call's.
        let pin = self.threshold.max(1);
        let grows = self.site.caller_size != CallerSize::Speed
            && !self.site.sole_call_to_local
            && self.site.tti.is_some_and(|tti| tti.opt_size_inline_by_code_size())
            && self.size_units > self.call_size();
        self.blocked_by_bound = grows && self.cost < pin;
        if grows {
            // The verdict is `cost < threshold`: the call stays.
            self.threshold = self.threshold.min(self.cost);
        }
        self.decided_by_threshold = true;
        if self.cost < self.threshold.max(1) && !grows { Ok(()) } else { Err("Cost over threshold.".into()) }
    }
}

/// `getInlineCost` (InlineCost.cpp:3265): what the call costs, or why it is
/// always or never to be inlined.
pub fn inline_cost(
    site: &Site,
    params: InlineParams,
) -> InlineCost {
    if let Some(decision) = attribute_decision(site) {
        return match decision {
            Ok(()) => InlineCost::Always("always inline attribute"),
            Err(why) => InlineCost::Never(why),
        };
    }
    let mut analyzer = Analyzer::new(site, params);
    let verdict = analyzer.analyze();
    if analyzer.decided_by_threshold {
        return InlineCost::Cost {
            cost: analyzer.cost,
            threshold: analyzer.threshold,
            static_bonus: analyzer.static_bonus_applied,
            blocked: analyzer.blocked_by_bound,
        };
    }
    match verdict {
        Ok(()) => InlineCost::Always("empty function"),
        Err(why) => InlineCost::Never(why),
    }
}

// ---- constants and the generic target (D2, D3) ---------------------------

fn mask(
    width: u32,
    bits: u128,
) -> u128 {
    if width >= 128 { bits } else { bits & ((1_u128 << width) - 1) }
}

fn signed(
    width: u32,
    bits: u128,
) -> i128 {
    if width == 0 || width >= 128 {
        return bits as i128;
    }
    let bits = mask(width, bits);
    if bits >> (width - 1) & 1 == 1 { (bits as i128) - (1_i128 << width) } else { bits as i128 }
}

fn compare(
    predicate: IntPredicate,
    width: u32,
    a: u128,
    b: u128,
) -> bool {
    let (ua, ub, sa, sb) = (mask(width, a), mask(width, b), signed(width, a), signed(width, b));
    match predicate {
        IntPredicate::Eq => ua == ub,
        IntPredicate::Ne => ua != ub,
        IntPredicate::Ugt => ua > ub,
        IntPredicate::Uge => ua >= ub,
        IntPredicate::Ult => ua < ub,
        IntPredicate::Ule => ua <= ub,
        IntPredicate::Sgt => sa > sb,
        IntPredicate::Sge => sa >= sb,
        IntPredicate::Slt => sa < sb,
        IntPredicate::Sle => sa <= sb,
    }
}

/// `ConstantFoldInstOperands` for an integer binary operator (D3); none where
/// the result is poison or undefined (division by zero).
fn fold_binary(
    op: BinaryOp,
    width: u32,
    a: u128,
    b: u128,
) -> Option<u128> {
    let (a, b) = (mask(width, a), mask(width, b));
    let (sa, sb) = (signed(width, a), signed(width, b));
    let bits = match op {
        BinaryOp::Add => a.wrapping_add(b),
        BinaryOp::Sub => a.wrapping_sub(b),
        BinaryOp::Mul => a.wrapping_mul(b),
        BinaryOp::UDiv => a.checked_div(b)?,
        BinaryOp::SDiv => (sa.checked_div(sb)?) as u128,
        BinaryOp::URem => a.checked_rem(b)?,
        BinaryOp::SRem => (sa.checked_rem(sb)?) as u128,
        BinaryOp::Shl => {
            if b >= u128::from(width) {
                return None;
            }
            a << b
        }
        BinaryOp::LShr => {
            if b >= u128::from(width) {
                return None;
            }
            a >> b
        }
        BinaryOp::AShr => {
            if b >= u128::from(width) {
                return None;
            }
            (sa >> b) as u128
        }
        BinaryOp::And => a & b,
        BinaryOp::Or => a | b,
        BinaryOp::Xor => a ^ b,
        _ => return None,
    };
    Some(mask(width, bits))
}

/// Intrinsics the generic cost model makes free (D2): the list of
/// `getIntrinsicInstrCost` (TargetTransformInfoImpl.h:924).
fn free_intrinsic(name: Option<&str>) -> bool {
    name.is_some_and(|name| {
        [
            "llvm.allow.runtime.check",
            "llvm.allow.ubsan.check",
            "llvm.annotation",
            "llvm.assume",
            "llvm.sideeffect",
            "llvm.pseudoprobe",
            "llvm.arithmetic.fence",
            "llvm.dbg.",
            "llvm.invariant.start",
            "llvm.invariant.end",
            "llvm.launder.invariant.group",
            "llvm.is.constant",
            "llvm.lifetime.start",
            "llvm.lifetime.end",
            "llvm.experimental.noalias.scope.decl",
            "llvm.objectsize",
            "llvm.ptr.annotation",
            "llvm.var.annotation",
            "llvm.experimental.gc.result",
            "llvm.experimental.gc.relocate",
            "llvm.coro.",
            "llvm.threadlocal.address",
            "llvm.experimental.widenable.condition",
            "llvm.ssa.copy",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
    })
}

/// `TTI.isLoweredToCall(F)` (TargetTransformInfoImpl.h:223, D2): a call to
/// anything but an intrinsic, a local function or a library routine the
/// target lowers to one node or smaller code.
fn lowered_to_call(
    module: &Module,
    target: GlobalId,
) -> bool {
    let global = module.global(target);
    if matches!(&global.kind, GlobalKind::Variable(_)) {
        return true;
    }
    let Some(name) = global.name.as_deref() else { return true };
    if name.starts_with("llvm.") {
        return false;
    }
    if local_linkage(global.linkage) {
        return true;
    }
    const NODE: [&str; 17] = [
        "copysign", "fabs", "fmin", "fmax", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh",
        "tanh", "sqrt", "exp10", "pow",
    ];
    let stem = name.strip_suffix('f').or_else(|| name.strip_suffix('l')).unwrap_or(name);
    if NODE.contains(&name) || NODE.contains(&stem) {
        return false;
    }
    !matches!(
        name,
        "exp2" | "exp2l" | "exp2f" | "floor" | "floorf" | "ceil" | "round" | "ffs" | "ffsl" | "abs" | "labs" | "llabs"
    )
}

fn only_reads_memory(info: &CallInfo) -> bool {
    !llrm_mir::memory::stated(&info.attrs).writes
}

/// The headers of the callee's loops: targets of a retreating edge (D7).
fn loop_headers(function: &Function) -> IdSet<BlockId> {
    let Some(&entry) = function.layout().first() else { return IdSet::new() };
    let mut headers = IdSet::new();
    let mut state: HashMap<u32, u8> = HashMap::default();
    let mut stack = vec![(entry, function.successors(entry).into_iter().collect::<Vec<_>>())];
    state.insert(entry.0, 1);
    while let Some((block, successors)) = stack.last_mut() {
        match successors.pop() {
            Some(next) => match state.get(&next.0).copied() {
                None => {
                    state.insert(next.0, 1);
                    let following = function.successors(next).into_iter().collect::<Vec<_>>();
                    stack.push((next, following));
                }
                Some(1) => {
                    headers.insert(next);
                }
                _ => {}
            },
            None => {
                state.insert(block.0, 2);
                stack.pop();
            }
        }
    }
    headers
}

/// A call target's linkage, for the caller of this module: the callee has
/// local linkage.
pub fn local_linkage(linkage: Linkage) -> bool {
    matches!(linkage, Linkage::Internal | Linkage::Private)
}

/// The cost of every direct call site of `module` at `-O2`'s threshold, one
/// line each on stderr (`LLRM_EXP_INLINECOST`), to be set beside `opt
/// -passes=inline`'s remarks; nothing is decided from it.
///
/// `counts` is `inline::call_counts` (direct calls by callee) and
/// `addressed` the functions whose address is taken.
pub fn shadow(
    module: &Module,
    layout: &DataLayout,
    counts: &crate::inline::Counter,
    addressed: &BTreeSet<GlobalId>,
    params: InlineParams,
) {
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(caller) = &global.kind else { continue };
        if caller.layout().is_empty() {
            continue;
        }
        let caller_id = GlobalId(at as u32);
        let frequencies = caller_frequencies(module, layout, caller);
        let caller_recursive = caller
            .layout()
            .iter()
            .any(
                |&block| caller
                    .iter_block(block)
                    .any(|inst| llrm_mir::memory::callee(&module.context, caller, inst) == Some(caller_id)),
            );
        let mut ordinal: HashMap<GlobalId, usize> = HashMap::default();
        for &block in caller.layout() {
            for inst in caller.iter_block(block) {
                let Some(callee_id) = llrm_mir::memory::callee(&module.context, caller, inst) else { continue };
                let callee_global = module.global(callee_id);
                let GlobalKind::Function(callee) = &callee_global.kind else { continue };
                let k = ordinal.entry(callee_id).or_insert(0);
                *k += 1;
                if callee.layout().is_empty() {
                    continue;
                }
                let site = Site {
                    module,
                    layout,
                    caller,
                    caller_id,
                    call: inst,
                    callee,
                    callee_id,
                    sole_call_to_local: local_linkage(callee_global.linkage)
                        && counts.get(&callee_id).copied() == Some(1)
                        && !addressed.contains(&callee_id),
                    caller_recursive,
                    caller_size: CallerSize::Speed,
                    relative_frequency: caller.parent(inst).and_then(|block| relative_to_entry(&frequencies, block)),
                    // The shadow is the oracle's: opt without a triple prices
                    // generically.
                    tti: None,
                };
                let leaf = !callee
                    .layout()
                    .iter()
                    .any(
                        |&block| callee
                            .iter_block(block)
                            .any(
                                |one| llrm_mir::memory::callee(&module.context, callee, one).is_some_and(|inner| {
                                    matches!(
                                        &module.global(inner).kind,
                                        GlobalKind::Function(body) if !body.layout().is_empty()
                                    )
                                }),
                            ),
                    );
                let verdict = inline_cost(&site, params);
                let line = match &verdict {
                    InlineCost::Always(why) => format!("always ({why})"),
                    InlineCost::Never(why) => format!("never ({why})"),
                    InlineCost::Cost { cost, threshold, .. } => {
                        format!("cost={cost} threshold={threshold} {}", if verdict.is_ok() { "inline" } else { "no" })
                    }
                };
                eprintln!(
                    "[inlinecost] {} -> {} #{}: {line}{}",
                    caller_global_name(module, caller_id),
                    callee_global.name.as_deref().unwrap_or("?"),
                    k,
                    if leaf { " leaf" } else { "" }
                );
            }
        }
    }
}

/// `opt -pass-remarks=inline`'s line for a decided site (InlineAdvisor.cpp
/// `emitInlinedInto`, Inliner.cpp's missed remark); `spliced` says whether
/// the copy was made, which a decision to inline may still be refused.
pub fn remark(
    caller: &str,
    callee: &str,
    cost: &InlineCost,
    spliced: bool,
) -> String {
    let priced = match cost {
        InlineCost::Always(why) => format!("(cost=always): {why}"),
        InlineCost::Never(why) => format!("(cost=never): {why}"),
        InlineCost::Cost { cost, threshold, .. } => format!("(cost={cost}, threshold={threshold})"),
    };
    match (spliced, cost.is_ok()) {
        (true, _) => format!("'{callee}' inlined into '{caller}' with {priced}"),
        (false, true) => format!("'{callee}' not inlined into '{caller}' because the copy was not made {priced}"),
        (false, false) if matches!(cost, InlineCost::Never(_)) => {
            format!("'{callee}' not inlined into '{caller}' because it should never be inlined {priced}")
        }
        (false, false) => format!("'{callee}' not inlined into '{caller}' because too costly to inline {priced}"),
    }
}

/// `BlockFrequencyInfo` of `caller` as the inliner's `GetBFI` gives it: over
/// `BranchProbabilityInfo` (D1).
pub fn caller_frequencies(
    module: &Module,
    layout: &DataLayout,
    caller: &Function,
) -> llrm_analysis::blockfreq::BlockFrequencies {
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout,
        },
        caller,
    );
    llrm_analysis::blockfreq::BlockFrequencies::of_bpi(caller, &bpi)
}

/// The frequency of each instruction of `function` over its entry's; none
/// where the entry's is 0.
pub fn call_frequencies(
    module: &Module,
    layout: &DataLayout,
    function: &Function,
) -> llrm_support::hash::SparseIdMap<InstId, f64> {
    let found = caller_frequencies(module, layout, function);
    let mut frequencies = llrm_support::hash::SparseIdMap::default();
    if found.get_entry_freq().get_frequency() > 0 {
        for (_, inst) in function.walk() {
            if let Some(block) = function.parent(inst) {
                frequencies.insert(inst, found.relative_to_entry(block));
            }
        }
    }
    frequencies
}

/// `block`'s frequency over the entry's, as `isColdCallSite` and
/// `getHotCallSiteThreshold` compare them; none where the entry's is 0.
fn relative_to_entry(
    frequencies: &llrm_analysis::blockfreq::BlockFrequencies,
    block: BlockId,
) -> Option<f64> {
    (frequencies.get_entry_freq().get_frequency() > 0).then(|| frequencies.relative_to_entry(block))
}

fn caller_global_name(
    module: &Module,
    id: GlobalId,
) -> &str {
    module.global(id).name.as_deref().unwrap_or("?")
}

// ---- float constants (D3): IEEE single and double; x87 by the double ----

fn as_f64(
    kind: FloatKind,
    bits: u64,
) -> f64 {
    match kind {
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
        _ => f64::from_bits(bits),
    }
}

fn from_f64(
    kind: FloatKind,
    value: f64,
) -> u64 {
    match kind {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        _ => value.to_bits(),
    }
}

fn negated(
    kind: FloatKind,
    bits: u64,
) -> u64 {
    match kind {
        FloatKind::Float => bits ^ 0x8000_0000,
        _ => bits ^ (1 << 63),
    }
}

/// `ConstantFoldBinaryInstruction` for the float operators, rounding as the
/// kind does.
fn fold_float(
    op: BinaryOp,
    kind: FloatKind,
    a: u64,
    b: u64,
) -> Option<u64> {
    if kind == FloatKind::Float {
        let (x, y) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
        let r = match op {
            BinaryOp::FAdd => x + y,
            BinaryOp::FSub => x - y,
            BinaryOp::FMul => x * y,
            BinaryOp::FDiv => x / y,
            BinaryOp::FRem => x % y,
            _ => return None,
        };
        return Some(u64::from(r.to_bits()));
    }
    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
    let r = match op {
        BinaryOp::FAdd => x + y,
        BinaryOp::FSub => x - y,
        BinaryOp::FMul => x * y,
        BinaryOp::FDiv => x / y,
        BinaryOp::FRem => x % y,
        _ => return None,
    };
    Some(r.to_bits())
}

fn compare_float(
    predicate: FloatPredicate,
    kind: FloatKind,
    a: u64,
    b: u64,
) -> bool {
    let (x, y) = (as_f64(kind, a), as_f64(kind, b));
    let unordered = x.is_nan() || y.is_nan();
    match predicate {
        FloatPredicate::False => false,
        FloatPredicate::True => true,
        FloatPredicate::Ord => !unordered,
        FloatPredicate::Uno => unordered,
        FloatPredicate::Oeq => !unordered && x == y,
        FloatPredicate::Ogt => !unordered && x > y,
        FloatPredicate::Oge => !unordered && x >= y,
        FloatPredicate::Olt => !unordered && x < y,
        FloatPredicate::Ole => !unordered && x <= y,
        FloatPredicate::One => !unordered && x != y,
        FloatPredicate::Ueq => unordered || x == y,
        FloatPredicate::Ugt => unordered || x > y,
        FloatPredicate::Uge => unordered || x >= y,
        FloatPredicate::Ult => unordered || x < y,
        FloatPredicate::Ule => unordered || x <= y,
        FloatPredicate::Une => unordered || x != y,
    }
}

fn float_of_integer(
    kind: FloatKind,
    value: i128,
) -> u64 {
    match kind {
        FloatKind::Float => u64::from((value as f32).to_bits()),
        _ => (value as f64).to_bits(),
    }
}

/// `fptosi` / `fptoui`: none where the value does not fit (poison in LLVM).
fn integer_of_float(
    kind: FloatKind,
    bits: u64,
    width: u32,
    signed_result: bool,
) -> Option<u128> {
    let value = as_f64(kind, bits);
    if value.is_nan() || width >= 64 {
        return None;
    }
    let truncated = value.trunc();
    let (low, high) = if signed_result {
        (-(2_f64.powi(width as i32 - 1)), 2_f64.powi(width as i32 - 1))
    } else {
        (0.0, 2_f64.powi(width as i32))
    };
    if truncated < low || truncated >= high {
        return None;
    }
    Some(mask(width, truncated as i128 as u128))
}

fn converted_float(
    from: FloatKind,
    bits: u64,
    to: FloatKind,
) -> u64 {
    from_f64(to, as_f64(from, bits))
}

/// The call sites of `caller` that LLVM would inline (`shouldInline`,
/// InlineAdvisor.cpp:412 with `getInlineCost` as the cost), each with the copy
/// to splice: the decision of `LLRM_EXP_INLINEDECIDE`, replacing the per-callee
/// budget. A site `inline::cloneable` refuses stays out whatever its cost.
///
/// D4: no `shouldBeDeferred` yet (D7).
pub fn sites(
    module: &Module,
    layout: &DataLayout,
    caller_id: GlobalId,
    counts: &crate::inline::Counter,
    addressed: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    params: InlineParams,
    size: CallerSize,
    always_only: bool,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    decided: Option<&mut Vec<(InstId, GlobalId, InlineCost)>>,
) -> llrm_support::hash::SparseIdMap<InstId, crate::inline::Candidate> {
    sites_with_trials(
        module,
        layout,
        caller_id,
        counts,
        addressed,
        recursive,
        params,
        size,
        always_only,
        tti,
        decided,
        None,
    )
}

/// `sites`, and the sites the target's -Os bound alone refused whose call
/// passes a constant or a stack slot, or whose private callee has no call but
/// these (SZ2): `trials`
/// takes their copies, for the walk to try inlined against not.
#[allow(clippy::too_many_arguments)]
pub fn sites_with_trials(
    module: &Module,
    layout: &DataLayout,
    caller_id: GlobalId,
    counts: &crate::inline::Counter,
    addressed: &BTreeSet<GlobalId>,
    recursive: &BTreeSet<GlobalId>,
    params: InlineParams,
    size: CallerSize,
    always_only: bool,
    tti: Option<&dyn llrm_mir::tti::Tti>,
    mut decided: Option<&mut Vec<(InstId, GlobalId, InlineCost)>>,
    mut trials: Option<&mut llrm_support::hash::SparseIdMap<InstId, crate::inline::Candidate>>,
) -> llrm_support::hash::SparseIdMap<InstId, crate::inline::Candidate> {
    let mut out = llrm_support::hash::SparseIdMap::default();
    let GlobalKind::Function(caller) = &module.global(caller_id).kind else { return out };
    let context = DecideContext { counts, addressed, recursive, params, size, always_only, tti };
    let frequencies = caller_frequencies(module, layout, caller);
    // The calls to each callee here: a private callee all of whose calls are
    // here goes with the last one inlined.
    let mut here: llrm_support::hash::IndexMap<GlobalId, i64> = Default::default();
    if trials.is_some() {
        for (_, inst) in caller.walk() {
            if let Some(callee) = llrm_mir::memory::callee(&module.context, caller, inst) {
                *here.entry(callee).or_default() += 1;
            }
        }
    }
    let goes = |callee: GlobalId| {
        local_linkage(module.global(callee).linkage)
            && !addressed.contains(&callee)
            && counts.get(&callee).is_some_and(|all| here.get(&callee) == Some(all))
    };
    for (_, inst) in caller.walk() {
        let Some(decision) = decide(module, layout, caller_id, inst, &context, &frequencies) else { continue };
        if let Some(decided) = decided.as_deref_mut() {
            decided.push((inst, decision.callee, decision.cost.clone()));
        }
        let blocked = matches!(decision.cost, InlineCost::Cost { blocked: true, .. });
        let worth = blocked
            && trials.is_some()
            && (goes(decision.callee) || worth_a_trial(module, caller, inst, decision.callee));
        if !decision.inline && !worth {
            continue;
        }
        let GlobalKind::Function(callee) = &module.global(decision.callee).kind else { continue };
        // A callee that calls itself may have those calls peeled in the
        // copy (D10): the frequencies of its calls say how deep.
        let own: Vec<InstId> = callee
            .walk()
            .map(|(_, one)| one)
            .filter(|&one| llrm_mir::memory::callee(&module.context, callee, one) == Some(decision.callee))
            .collect();
        let candidate = crate::inline::Candidate {
            body: std::rc::Rc::new((**callee).clone()),
            frame: crate::inline::grown(&module.context, layout, callee),
            moved: false,
            last_call: local_linkage(module.global(decision.callee).linkage)
                && !addressed.contains(&decision.callee)
                && counts.get(&decision.callee) == Some(&1),
            callee: Some(decision.callee),
            self_sites: own.len(),
            self_calls: (!own.is_empty()).then(|| {
                let frequencies = call_frequencies(module, layout, callee);
                own.iter().map(|one| frequencies.get(one).copied().unwrap_or(0.0)).sum()
            }),
        };
        if decision.inline {
            out.insert(inst, candidate);
        } else if let Some(trials) = trials.as_deref_mut() {
            trials.insert(inst, candidate);
        }
    }
    out
}

/// The stack slot or the parameter a pointer is made from, through casts and
/// offsets.
enum Root {
    Slot(InstId),
    Parameter(llrm_mir::module::ValueId),
}

fn root_of(
    function: &llrm_mir::module::Function,
    operand: llrm_mir::module::Operand,
) -> Option<Root> {
    let llrm_mir::module::Operand::Value(mut at) = operand else { return None };
    loop {
        match function.value(at).def {
            llrm_mir::module::ValueDef::Argument(_) => return Some(Root::Parameter(at)),
            llrm_mir::module::ValueDef::Instruction(def) => {
                let instruction = function.instruction(def);
                match instruction.opcode {
                    Opcode::Alloca { .. } => return Some(Root::Slot(def)),
                    Opcode::GetElementPtr { .. } | Opcode::Cast(_) => match instruction.operands[0] {
                        llrm_mir::module::Operand::Value(next) => at = next,
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }
    }
}

/// The global a constant pointer is the address of, or a step or a cast of.
fn global_of(
    context: &llrm_mir::context::Context,
    operand: llrm_mir::module::Operand,
) -> Option<GlobalId> {
    use llrm_mir::context::{ConstantExpr, ConstantKind};
    let llrm_mir::module::Operand::Constant(mut at) = operand else { return None };
    loop {
        match &context.get(at).kind {
            ConstantKind::Global(global) => return Some(*global),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => at = *operands.first()?,
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => at = *value,
            _ => return None,
        }
    }
}

/// SZ2: whether a site refused by the -Os bound is worth inlining to see: the
/// call passes the address of a stack slot (or of a global) the caller wrote a
/// constant to, and the callee reads through that parameter, which the cost
/// model does not follow (a constant argument it folds itself).
fn worth_a_trial(
    module: &Module,
    caller: &llrm_mir::module::Function,
    call: InstId,
    callee_id: GlobalId,
) -> bool {
    let GlobalKind::Function(callee) = &module.global(callee_id).kind else { return false };
    let operands = &caller.instruction(call).operands;
    let arguments = &operands[..operands.len().saturating_sub(1)];
    let written = |slot: InstId| {
        caller
            .walk()
            .any(
                |(_, inst)| {
                    let store = caller.instruction(inst);
                    matches!(store.opcode, Opcode::Store { .. })
                        && matches!(store.operands[0], llrm_mir::module::Operand::Constant(_))
                        && matches!(
                            root_of(caller, store.operands[1]),
                            Some(Root::Slot(one)) if one == slot
                        )
                },
            )
    };
    // A gate, not dataflow: the store may be to another field, or after the
    // call; the trial decides on bytes.
    let written_global = |global: GlobalId| {
        caller
            .walk()
            .any(
                |(_, inst)| {
                    let store = caller.instruction(inst);
                    matches!(store.opcode, Opcode::Store { .. })
                        && matches!(store.operands[0], llrm_mir::module::Operand::Constant(_))
                        && global_of(&module.context, store.operands[1]) == Some(global)
                },
            )
    };
    let read_through: Vec<llrm_mir::module::ValueId> = callee
        .walk()
        .filter_map(|(_, inst)| {
            let load = callee.instruction(inst);
            match (&load.opcode, root_of(callee, *load.operands.first()?)) {
                (Opcode::Load { .. }, Some(Root::Parameter(parameter))) => Some(parameter),
                _ => None,
            }
        })
        .collect();
    // A callee that reads a global directly (its pointer arguments were
    // propagated into it), one the caller has stored a constant to.
    let reads_written_global = callee.walk().any(|(_, inst)| {
        let load = callee.instruction(inst);
        matches!(load.opcode, Opcode::Load { .. })
            && load.operands.first().and_then(|ptr| global_of(&module.context, *ptr)).is_some_and(written_global)
    });
    reads_written_global
        || arguments.iter().zip(callee.parameters()).any(|(argument, parameter)| {
            read_through.contains(parameter)
                && (matches!(
                    root_of(caller, *argument),
                    Some(Root::Slot(slot)) if written(slot)
                ) || global_of(&module.context, *argument).is_some_and(written_global))
        })
}

/// What `decide` reads of the module besides the site.
pub struct DecideContext<'a> {
    /// Direct calls by callee (`inline::call_counts`).
    pub counts: &'a crate::inline::Counter,
    /// The functions whose address is taken.
    pub addressed: &'a BTreeSet<GlobalId>,
    pub recursive: &'a BTreeSet<GlobalId>,
    pub params: InlineParams,
    pub size: CallerSize,
    /// `-fno-inline-functions`: only `alwaysinline` is inlined.
    pub always_only: bool,
    pub tti: Option<&'a dyn llrm_mir::tti::Tti>,
}

/// One call site's verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub callee: GlobalId,
    pub inline: bool,
    pub cost: InlineCost,
}

/// `InlinerPass`'s question of the call `call` in `caller`: whether to
/// inline it and at what cost (`getInlineCost`, `shouldInline`).
/// `frequencies` is `caller_frequencies` of the caller as it stands. None
/// where `call` is no direct call of a body that can be copied.
pub fn decide(
    module: &Module,
    layout: &DataLayout,
    caller_id: GlobalId,
    call: InstId,
    context: &DecideContext,
    frequencies: &llrm_analysis::blockfreq::BlockFrequencies,
) -> Option<Decision> {
    let GlobalKind::Function(caller) = &module.global(caller_id).kind else { return None };
    let callee_id = llrm_mir::memory::callee(&module.context, caller, call)?;
    let callee_global = module.global(callee_id);
    let GlobalKind::Function(callee) = &callee_global.kind else { return None };
    // A call of the caller itself is `walk`'s bounded self-inlining, not a
    // site priced here.
    if callee.layout().is_empty() || callee_id == caller_id || !crate::inline::copyable(module, callee) {
        return None;
    }
    let site = Site {
        module,
        layout,
        caller,
        caller_id,
        call,
        callee,
        callee_id,
        sole_call_to_local: local_linkage(callee_global.linkage)
            && context.counts.get(&callee_id).copied() == Some(1)
            && !context.addressed.contains(&callee_id),
        caller_recursive: context.recursive.contains(&caller_id),
        caller_size: context.size,
        relative_frequency: caller.parent(call).and_then(|block| relative_to_entry(frequencies, block)),
        tti: context.tti,
    };
    let cost = inline_cost(&site, context.params);
    // `-fno-inline-functions`: clang's pipeline is the always-inliner
    // alone.
    let inline =
        if context.always_only { matches!(cost, InlineCost::Always("always inline attribute")) } else { cost.is_ok() };
    Some(Decision { callee: callee_id, inline, cost })
}
