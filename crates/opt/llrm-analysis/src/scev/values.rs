//! `ScalarEvolution::print`'s per-value part (`print<scalar-evolution>`):
//! each instruction's expression and ranges, in the text `tools/scev/values.py`
//! compares with `opt`.

use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;

use super::print::Labels;
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14220
    /// The "Classifying expressions" lines of `print`: for each SCEVable
    /// instruction that is not a compare, `%name<TAB>expr<TAB>U: range<TAB>S:
    /// range`.
    pub fn print_values(
        &mut self,
        cx: &Cx,
        labels: &Labels,
    ) -> String {
        let mut out = String::new();
        for &block in cx.function.layout() {
            for &inst in cx.function.block(block).instructions() {
                let instruction = cx.function.instruction(inst);
                let Some(result) = instruction.result else { continue };
                if matches!(instruction.opcode, Opcode::ICmp(_) | Opcode::FCmp(_)) {
                    continue;
                }
                let value = Operand::Value(result);
                if !self.is_value_scevable(cx, value) {
                    continue;
                }
                let sv = self.get_scev(cx, value);
                let unsigned = self.get_unsigned_range(cx, sv);
                let signed = self.get_signed_range(cx, sv);
                out += &format!(
                    "{}\t{}\tU: {}\tS: {}\n",
                    self.print_value_name(labels, value),
                    self.print(cx, labels, sv),
                    unsigned.print(),
                    signed.print()
                );
            }
        }
        out
    }
}

/// `print<scalar-evolution>`'s per-value lines for each function of `module`
/// that has a body: `@name`, then `print_values`.
pub fn print_module_values(module: &llrm_mir::module::Module) -> String {
    let layout = module
        .datalayout
        .as_deref()
        .and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok())
        .unwrap_or_default();
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let tree = llrm_mir::dominators::DominatorTree::new(function);
        let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
        let assumptions = llrm_mir::valuetracking::AssumptionCache::new(module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let labels = Labels::of(function);
        let mut se = ScalarEvolution::new();
        out += &format!(
            "@{}\n",
            super::print::printed_name(&llrm_mir::print::quoted(global.name.as_deref().unwrap_or("?")))
        );
        out += &se.print_values(&cx, &labels);
    }
    out
}
