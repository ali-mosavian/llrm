//! `ScalarEvolution::print`'s per-value part (`print<scalar-evolution>`):
//! each instruction's expression and ranges, in the text `tools/scev/values.py`
//! compares with `opt`.

use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;

use super::expr::{Kind, Scev, Use};
use super::print::Labels;
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14396
    /// The "Classifying expressions" part of `print`: for each SCEVable
    /// instruction that is not a compare, `%name<TAB>` and the text LLVM
    /// writes after `-->`. The order of the queries is LLVM's: each one can
    /// make the next see more or less of what has been computed, and so can
    /// decide flags and ranges.
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
                out += &self.print_value_name(labels, value);
                out.push('\t');
                let sv = self.get_scev(cx, value);
                out += &self.print(cx, labels, sv);
                if !matches!(self.node(sv).kind, Kind::CouldNotCompute) {
                    out += &self.print_ranges(cx, sv);
                }

                let l = cx.loops.get_loop_for(block).map(|one| one.get_header());
                let at_use = self.get_scev_at_scope_use(cx, sv, l);
                if at_use != Use::of(sv) {
                    out += "  -->  ";
                    out += &self.print_use(cx, labels, at_use);
                    if !matches!(self.node(at_use.scev).kind, Kind::CouldNotCompute) {
                        out += &self.print_ranges(cx, at_use.scev);
                    }
                }

                if let Some(l) = l {
                    let loop_ = super::disposition::loop_of(cx, l);
                    out += "\t\tExits: ";
                    let parent = loop_.get_parent_loop().map(|one| one.get_header());
                    let exit_value = self.get_scev_at_scope_use(cx, sv, parent);
                    if !self.is_loop_invariant(cx, exit_value.scev, Some(l)) {
                        out += "<<Unknown>>";
                    } else {
                        out += &self.print_use(cx, labels, exit_value);
                    }

                    let mut headers = vec![];
                    let mut at = Some(loop_);
                    while let Some(one) = at {
                        headers.push(one.get_header());
                        at = one.get_parent_loop();
                    }
                    let mut inner = Vec::new();
                    loop_.get_inner_loops_in_preorder(&mut inner);
                    headers.extend(inner.into_iter().map(|one| one.get_header()));
                    let mut separator = "\t\tLoopDispositions: { ";
                    for header in headers {
                        out += separator;
                        separator = ", ";
                        out += &format!(
                            "%{}: {}",
                            labels.block_label(header),
                            match self.get_loop_disposition(cx, sv, Some(header)) {
                                super::disposition::LoopDisposition::Variant => "Variant",
                                super::disposition::LoopDisposition::Invariant => "Invariant",
                                super::disposition::LoopDisposition::Computable => "Computable",
                                super::disposition::LoopDisposition::Uniform => "Uniform",
                            }
                        );
                    }
                    out += " }";
                }
                out.push('\n');
            }
        }
        out
    }

    /// ` U: <range> S: <range>`.
    fn print_ranges(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> String {
        let unsigned = self.get_unsigned_range(cx, s);
        let signed = self.get_signed_range(cx, s);
        format!(" U: {} S: {}", unsigned.print(), signed.print())
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
        let tree = llrm_mir::domtree::DomTree::of(function);
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

impl ScalarEvolution {
    /// `PrintSCEVWithTypeHint`.
    fn print_with_type_hint(
        &self,
        cx: &Cx,
        labels: &Labels,
        s: Scev,
    ) -> String {
        let text = self.print(cx, labels, s);
        match self.node(s).kind {
            Kind::Constant(_) => format!("{} {text}", super::print::type_text(self.type_of(s))),
            _ => text,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14185
    /// `PrintLoopInfo`: the loop's counts, the inner loops first.
    fn print_loop_info(
        &mut self,
        cx: &Cx,
        labels: &Labels,
        header: llrm_mir::module::BlockId,
    ) -> String {
        use super::backedge::ExitCountKind;
        let l = cx.loops.get_loop_for(header).expect("a loop");
        let mut out = String::new();
        for sub in l.get_sub_loops() {
            out += &self.print_loop_info(cx, labels, sub.get_header());
        }
        let name = format!("Loop %{}: ", labels.block_label(header));
        let exiting = l.get_exiting_blocks(cx.function);
        let unnamed = |block| cx.function.block(block).name.clone().unwrap_or_default();
        out += &name;
        if exiting.len() != 1 {
            out += "<multiple exits> ";
        }
        let hint = |se: &Self, s: Scev| se.print_with_type_hint(cx, labels, s);
        let btc = self.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
        let cnc = self.get_could_not_compute();
        if btc != cnc {
            out += &format!("backedge-taken count is {}", hint(self, btc));
        } else {
            out += "Unpredictable backedge-taken count.";
        }
        out.push('\n');
        if exiting.len() > 1 {
            for &block in &exiting {
                let ec = self.get_exit_count(cx, header, block, ExitCountKind::Exact);
                out += &format!("  exit count for {}: {}\n", unnamed(block), hint(self, ec));
            }
        }
        out += &name;
        let max_or_zero = self.is_backedge_taken_count_max_or_zero(cx, header);
        let constant_max = self.get_backedge_taken_count(cx, header, ExitCountKind::ConstantMaximum);
        if constant_max != cnc {
            out += &format!("constant max backedge-taken count is {}", hint(self, constant_max));
            if max_or_zero {
                out += ", actual taken count either this or zero.";
            }
        } else {
            out += "Unpredictable constant max backedge-taken count. ";
        }
        out.push('\n');
        out += &name;
        let symbolic = self.get_backedge_taken_count(cx, header, ExitCountKind::SymbolicMaximum);
        if symbolic != cnc {
            out += &format!("symbolic max backedge-taken count is {}", hint(self, symbolic));
            if max_or_zero {
                out += ", actual taken count either this or zero.";
            }
        } else {
            out += "Unpredictable symbolic max backedge-taken count. ";
        }
        out.push('\n');
        if exiting.len() > 1 {
            for &block in &exiting {
                let ec = self.get_exit_count(cx, header, block, ExitCountKind::SymbolicMaximum);
                out += &format!("  symbolic max exit count for {}: {}\n", unnamed(block), hint(self, ec));
            }
        }
        if self.has_loop_invariant_backedge_taken_count(cx, header) {
            out += &name;
            out += &format!("Trip multiple is {}\n", self.get_small_constant_trip_multiple(cx, header));
        }
        out
    }
}

/// `print<scalar-evolution>`'s "Determining loop execution counts" sections.
pub fn print_module_counts(module: &llrm_mir::module::Module) -> String {
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
        let tree = llrm_mir::domtree::DomTree::of(function);
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
            "Determining loop execution counts for: @{}\n",
            super::print::printed_name(&llrm_mir::print::quoted(global.name.as_deref().unwrap_or("?")))
        );
        for top in loops.top_level_loops() {
            out += &se.print_loop_info(&cx, &labels, top.get_header());
        }
    }
    out
}
