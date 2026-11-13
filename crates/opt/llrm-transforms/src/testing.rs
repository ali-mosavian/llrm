//! What the tests share: a module read from its text, printed once it
//! verifies, and run.

use llrm_mir::GlobalId;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::{Function, Module};

pub fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// `module`'s text, which must verify.
pub fn printed(module: &Module) -> String {
    let text = llrm_mir::print::module(module);
    assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new(), "{text}");
    text
}

/// @f of `module`.
pub fn f(module: &mut Module) -> &mut Function {
    module.function_mut("f").expect("@f").1
}

/// What @f returns for each of `inputs`, each argument an integer of the
/// width `@f` declares; it must return.
pub fn results(
    module: &Module,
    inputs: &[&[i128]],
) -> Vec<Val> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let widths = function
        .parameters()
        .iter()
        .map(|&one| module.context.types.int_bits(function.value(one).ty).expect("an integer"))
        .collect::<Vec<_>>();
    inputs
        .iter()
        .map(|input| {
            let arguments = input
                .iter()
                .zip(&widths)
                .map(|(&value, &width)| Val::Int { bits: llrm_mir::context::mask(width) & value as u128, width })
                .collect();
            interpret::run(module, "f", arguments, 100_000).unwrap_or_else(|trap| panic!("@f{input:?}: {trap:?}"))
        })
        .collect()
}

/// @f's @g holds 7 across a call to a `readonly` @peek: only @peek's
/// declaration, read of the module, says the call leaves @g alone.
pub const ACROSS_READONLY_CALL: &str = "@g = global i16 0

declare i16 @peek() readonly

define i16 @f() {
b0:
  store i16 7, ptr @g
  %p = call i16 @peek()
  %v = load i16, ptr @g
  ret i16 %v
}
";

/// `module` through `pass` under the pass manager, printed.
pub fn managed(
    module: &mut Module,
    pass: impl llrm_mir::passes::FunctionPass + 'static,
) -> String {
    managed_on(module, pass, Tuned::default())
}

/// `managed` for `machine`.
pub fn managed_on(
    module: &mut Module,
    pass: impl llrm_mir::passes::FunctionPass + 'static,
    machine: Tuned,
) -> String {
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(pass);
    manager.run_module(module, std::rc::Rc::new(machine)).unwrap();
    printed(module)
}

/// @h writes only what its argument points to, which only its summary
/// says: a call to it without `Summaries` may write anything.
pub const WRITES_ITS_ARGUMENT: &str = "@g = global [64 x i8] zeroinitializer
@k = global i16 0

define void @h(ptr %s) {
b0:
  store i16 9, ptr %s
  ret void
}

";

/// What the pipeline's `FunctionAttrs` has stated of `module`'s functions, as
/// `Summaries` reads them.
pub fn attributed(module: &mut Module) {
    let layout = llrm_analysis::testing::layout(module);
    llrm_analysis::functionattrs::infer_module(
        module,
        &layout,
        llrm_mir::target::Machine::spaces(&llrm_mir::target::Neutral),
    );
}

/// `module` through `pass` under a pass manager that requires `Summaries`
/// where `summaries`, as LLVM's `RequireAnalysisPass`, or a bare one;
/// printed. @f computes what it did on `inputs`.
pub fn summarized(
    module: &Module,
    pass: impl llrm_mir::passes::FunctionPass + 'static,
    summaries: bool,
    inputs: &[&[i128]],
) -> String {
    let mut after = module.clone();
    if summaries {
        attributed(&mut after);
    }
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.verify_each = true;
    if summaries {
        manager.require::<llrm_analysis::manager::Summaries>();
    }
    manager.add(pass);
    manager.run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let text = printed(&after);
    assert_eq!(results(&after, inputs), results(module, inputs), "{text}");
    text[text.find("@f(").expect("@f")..].to_owned()
}

/// Every function of `module` that has a body.
pub fn bodies(module: &Module) -> Vec<GlobalId> {
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect()
}

/// A target of the given prices, registers and address forms, and no
/// foreign memory. No forms given are `Neutral`'s.
#[derive(Default)]
pub struct Tuned {
    pub costs: crate::profit::OperationCosts,
    pub sizes: crate::profit::OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
    pub float_registers: i64,
    pub far_access: i64,
    pub segments: i64,
    pub address_forms: Vec<llrm_mir::target::AddressForm>,
    /// What a multiply by each constant costs, where not a multiply.
    pub multiplies: std::collections::BTreeMap<i64, i64>,
    /// `Machine::huge_window`.
    pub window: Option<(u32, i64)>,
    /// The address spaces, where not real mode's.
    pub spaces: Option<llrm_mir::spaces::Spaces>,
    /// `Lowering::is_mask_and_cmp0_folding_beneficial`.
    pub folds_masks: bool,
    /// `Machine::two_address`.
    pub two_address: bool,
    /// The target's library has `memset_pattern16`, as a Darwin one does.
    pub memset_pattern: bool,
    /// A global is never a base, and a register and an index are the only
    /// forms.
    pub no_global_base: bool,
    /// `Tti::prefers_exit_ne`.
    pub exit_ne: bool,
    /// `Tti::narrows_counters`.
    pub narrows_counters: bool,
    /// `Tti::narrows_counters_at_speed` answers no.
    pub narrows_counters_for_size_only: bool,
    /// `Tti::unswitch_threshold_override`.
    pub unswitch_threshold: Option<i64>,
    /// `Tti::sinks_unused_invariants`.
    pub sinks_invariants: bool,
    /// `ctlz` costs one unit, as on a target with a count-leading-zeros
    /// instruction.
    pub cheap_ctlz: bool,
    /// `FSQRT` is legal on `double`.
    pub fast_sqrt: bool,
    /// `Tti::is_fcmp_ord_cheaper_than_fcmp_zero` is false, as X86's.
    pub ord_dear: bool,
    /// `Tuning::partial_libcalls_skip_at_size`.
    pub skip_partial_at_size: bool,
    /// `Tuning::price_fills`.
    pub price_fills: bool,
    /// No constant memset is stored inline: every fill is a `rep stos`.
    pub rep_fills: bool,
    /// `Tti::second_ipsccp`.
    pub second_ipsccp: bool,
}

use llrm_mir::tti;

/// `Basic`'s prices, with the named hooks on.
#[derive(Default)]
pub struct Flagged {
    pub exit_ne: bool,
    pub narrows_counters: bool,
    pub narrows_counters_for_size_only: bool,
    pub unswitch_threshold: Option<i64>,
    pub sinks_invariants: bool,
    /// `ctlz` costs one unit, as on a target with a count-leading-zeros
    /// instruction.
    pub cheap_ctlz: bool,
    pub fast_sqrt: bool,
    pub ord_dear: bool,
    pub skip_partial_at_size: bool,
    pub no_cmov: bool,
    pub no_global_base: bool,
    pub second_ipsccp: bool,
}

impl llrm_mir::tti::Tti for Flagged {
    fn second_ipsccp(&self) -> bool {
        self.second_ipsccp
    }

    fn have_fast_sqrt(
        &self,
        ty: tti::Ty,
    ) -> bool {
        self.fast_sqrt && matches!(ty, tti::Ty::Double | tti::Ty::Float)
    }

    fn partial_libcalls_skip_at_size(&self) -> bool {
        self.skip_partial_at_size
    }

    fn is_fcmp_ord_cheaper_than_fcmp_zero(
        &self,
        _ty: tti::Ty,
    ) -> bool {
        !self.ord_dear
    }

    fn prefers_exit_ne(&self) -> bool {
        self.exit_ne
    }

    fn narrows_counters(&self) -> bool {
        self.narrows_counters
    }

    fn narrows_counters_at_speed(&self) -> bool {
        !self.narrows_counters_for_size_only
    }

    fn unswitch_threshold_override(&self) -> Option<i64> {
        self.unswitch_threshold
    }

    fn sinks_unused_invariants(&self) -> bool {
        self.sinks_invariants
    }

    fn has_cmov(&self) -> bool {
        !self.no_cmov
    }

    fn intrinsic_cost(
        &self,
        name: &str,
        ret: Option<tti::Ty>,
        args: &[tti::Ty],
        kind: tti::CostKind,
    ) -> tti::Cost {
        if self.cheap_ctlz && name.starts_with("llvm.ctlz.") {
            return tti::Cost::BASIC;
        }
        tti::Basic.intrinsic_cost(name, ret, args, kind)
    }

    fn cmp_sel_cost(
        &self,
        kind: tti::CmpSel,
        ty: tti::Ty,
        cost: tti::CostKind,
    ) -> tti::Cost {
        tti::Basic.cmp_sel_cost(kind, ty, cost)
    }

    fn memory_op_cost(
        &self,
        op: tti::MemoryOp,
        ty: tti::Ty,
        cost: tti::CostKind,
        volatile: bool,
    ) -> tti::Cost {
        tti::Basic.memory_op_cost(op, ty, cost, volatile)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: tti::CostKind,
    ) -> tti::Cost {
        tti::Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> tti::Cost {
        tti::Basic.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: tti::ImmUser,
        index: u32,
        value: i128,
        bits: u32,
    ) -> tti::Cost {
        tti::Basic.int_imm_cost_inst(user, index, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: tti::Ty,
        to: tti::Ty,
    ) -> bool {
        tti::Basic.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        global: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        if self.no_global_base {
            return !global && scale <= 1;
        }
        tti::Basic.is_legal_addressing_mode(global, offset, base, scale)
    }

    fn cast_cost(
        &self,
        kind: tti::CastKind,
        from: tti::Ty,
        to: tti::Ty,
        cost: tti::CostKind,
    ) -> tti::Cost {
        tti::Basic.cast_cost(kind, from, to, cost)
    }

    fn arithmetic_cost(
        &self,
        op: tti::ArithOp,
        ty: tti::Ty,
        cost: tti::CostKind,
        lhs: tti::OperandInfo,
        rhs: tti::OperandInfo,
    ) -> tti::Cost {
        tti::Basic.arithmetic_cost(op, ty, cost, lhs, rhs)
    }
}

/// A library of the pin's defaults for an unknown OS, with `memset_pattern16`.
struct WithMemsetPattern;

impl llrm_mir::libfunc::Library for WithMemsetPattern {
    fn int_bits(&self) -> u32 {
        32
    }

    fn size_t_bits(&self) -> u32 {
        32
    }

    fn availability(
        &self,
        func: llrm_mir::libfunc::LibFunc,
    ) -> llrm_mir::libfunc::Availability {
        if func == llrm_mir::libfunc::LibFunc::MemsetPattern16 {
            llrm_mir::libfunc::Availability::Standard
        } else {
            llrm_mir::libfunc::Generic { int_bits: 32, size_t_bits: 32 }.availability(func)
        }
    }
}

struct FoldsMasks;

impl llrm_mir::lowering::Lowering for FoldsMasks {
    fn is_mask_and_cmp0_folding_beneficial(&self) -> bool {
        true
    }
}

impl llrm_mir::target::Machine for Tuned {
    fn library(&self) -> Option<std::rc::Rc<dyn llrm_mir::libfunc::Library>> {
        self.memset_pattern.then(|| std::rc::Rc::new(WithMemsetPattern) as _)
    }

    fn tuning(&self) -> llrm_mir::target::Tuning {
        let inline = if self.rep_fills { 0 } else { llrm_mir::target::Tuning::PIN.max_stores_per_memset };
        let inline_size = if self.rep_fills { 0 } else { llrm_mir::target::Tuning::PIN.max_stores_per_memset_opt_size };
        llrm_mir::target::Tuning {
            price_fills: self.price_fills,
            max_stores_per_memset: inline,
            max_stores_per_memset_opt_size: inline_size,
            ..llrm_mir::target::Tuning::PIN
        }
    }

    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        (self.exit_ne
            || self.narrows_counters
            || self.sinks_invariants
            || self.no_global_base
            || self.cheap_ctlz
            || self.fast_sqrt
            || self.second_ipsccp
            || self.ord_dear
            || self.unswitch_threshold.is_some()
            || self.skip_partial_at_size)
            .then(|| {
                std::rc::Rc::new(Flagged {
                    exit_ne: self.exit_ne,
                    narrows_counters: self.narrows_counters,
                    narrows_counters_for_size_only: self.narrows_counters_for_size_only,
                    unswitch_threshold: self.unswitch_threshold,
                    sinks_invariants: self.sinks_invariants,
                    no_cmov: false,
                    no_global_base: self.no_global_base,
                    cheap_ctlz: self.cheap_ctlz,
                    fast_sqrt: self.fast_sqrt,
                    second_ipsccp: self.second_ipsccp,
                    ord_dear: self.ord_dear,
                    skip_partial_at_size: self.skip_partial_at_size,
                }) as _
            })
    }

    fn lowering(&self) -> Option<std::rc::Rc<dyn llrm_mir::lowering::Lowering>> {
        self.folds_masks.then(|| std::rc::Rc::new(FoldsMasks) as _)
    }

    fn two_address(&self) -> bool {
        self.two_address
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.spaces.unwrap_or_else(llrm_x86_m16::spaces)
    }

    fn foreign_span(
        &self,
        _: (i64, i64),
        _: (i64, i64),
        _: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> crate::profit::OperationCosts {
        self.costs.clone()
    }

    fn size_costs(&self) -> crate::profit::OperationCosts {
        self.sizes.clone()
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }

    fn float_registers(&self) -> i64 {
        self.float_registers
    }

    fn far_access_registers(&self) -> i64 {
        self.far_access
    }

    fn segment_registers(&self) -> i64 {
        self.segments
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        if self.address_forms.is_empty() {
            llrm_mir::target::Neutral.address_forms()
        } else {
            self.address_forms.clone()
        }
    }

    fn multiply_by(
        &self,
        factor: i64,
    ) -> i64 {
        self.multiplies.get(&factor).copied().unwrap_or(self.costs.multiply)
    }

    fn huge_window(&self) -> Option<(u32, i64)> {
        self.window
    }
}

/// `interprocedural::stamped` over `module` alone, for no target.
pub fn stamped(module: &mut Module) -> Result<Vec<GlobalId>, String> {
    let mut analyses = llrm_mir::passes::ModuleAnalyses::of(module, std::rc::Rc::new(llrm_mir::target::Neutral));
    crate::interprocedural::stamped(module, &mut analyses)
}

/// The data layout the tests that need one share.
pub fn layout() -> llrm_mir::datalayout::DataLayout {
    llrm_mir::datalayout::DataLayout::parse("e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128").expect("a layout")
}

/// @f of `module` closed in LCSSA form; whether anything changed.
pub fn closed(module: &mut Module) -> Result<bool, String> {
    closed_named(module, "f")
}

/// The function `name` of `module` closed in LCSSA form.
pub fn closed_named(
    module: &mut Module,
    name: &str,
) -> Result<bool, String> {
    let (context, function) = module.function_mut(name).expect("the function");
    crate::lcssa::closed(context, &layout(), function)
}

/// Asks for the manager's MemorySSA and says nothing else; `forgets` returns
/// `none()`, as a pass that edited without naming what it kept.
pub struct AsksMssa {
    pub forgets: bool,
}

impl llrm_mir::passes::FunctionPass for AsksMssa {
    fn name(&self) -> &'static str {
        "asks-mssa"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::MemorySsa>(unit.context, unit.layout, unit.function);
        if self.forgets {
            llrm_mir::passes::PreservedAnalyses::none()
        } else {
            llrm_mir::passes::PreservedAnalyses::all()
        }
    }
}

/// The MemorySSA computations (hits, misses) a pipeline of `passes` makes over
/// `text`, each pass checked by `verify_invalidation` against a fresh build.
pub fn mssa_misses_over(
    text: &str,
    passes: Vec<Box<dyn llrm_mir::passes::FunctionPass>>,
) -> (u64, u64) {
    let mut module = parsed(text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.verify_invalidation = true;
    for pass in passes {
        manager.add_boxed(pass);
    }
    let before = llrm_mir::counts::analysed("mssa");
    manager.run_module(&mut module, std::rc::Rc::new(Tuned::default())).unwrap();
    let after = llrm_mir::counts::analysed("mssa");
    (after.0 - before.0, after.1 - before.1)
}
