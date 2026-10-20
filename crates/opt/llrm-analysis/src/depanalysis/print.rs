//! `print<da>`: `dumpExampleDependence` for each function of a module.
//!
//! # How it works
//! Every pair of instructions that may read or write memory, the first not
//! after the second, is given to [`DependenceInfo::depends`] with run-time
//! assumptions allowed; the answer is printed as LLVM prints it. An
//! instruction is its line of the printed function, as MIR prints it.
//!
//! # Example
//! ```text
//! Src:  store i32 1, ptr %p --> Dst:  %v = load i32, ptr %p
//!   da analyze - flow [|<]!
//! ```

use std::fmt::Write as _;

use llrm_mir::module::{InstId, Module};
use llrm_mir::valuetracking::guarantee::{may_read_from_memory, may_write_to_memory};

use super::{Access, DependenceInfo, Options, TestType};
use crate::aggregate::AggregateAA;
use crate::basicaa::BasicAA;
use crate::memory::Unit;
use crate::scev::print::Labels;
use crate::scev::{Cx, ScalarEvolution};
use crate::typebasedaa::TypeBasedAA;

/// The lines of the printed module's function bodies, one list per defined
/// function in module order: each is an instruction, `"  ..."`.
fn instruction_lines(module: &Module) -> Vec<Vec<String>> {
    let printed = llrm_mir::print::module(module);
    let mut functions: Vec<Vec<String>> = Vec::new();
    let mut inside = false;
    for line in printed.lines() {
        if line.starts_with("define ") {
            functions.push(Vec::new());
            inside = true;
        } else if line == "}" {
            inside = false;
        } else if inside && line.starts_with("  ") {
            functions.last_mut().expect("a function").push(line.to_owned());
        }
    }
    functions
}

/// What a `print<da>` run is given: the pass's options, whether results are
/// normalized (`print<da<normalized-results>>`) and whether type-based alias
/// analysis joins basic alias analysis (`-aa-pipeline=basic-aa,tbaa`; the
/// default pipeline also has scoped-noalias, which MIR has no metadata for).
#[derive(Clone, Copy, Debug, Default)]
pub struct PrintOptions {
    pub options: Options,
    pub normalize: bool,
    pub tbaa: bool,
}

impl PrintOptions {
    /// The flags `--normalize`, `--tbaa`, `--no-delinearize`,
    /// `--disable-delinearization-checks`, `--miv-max-level N` and `--test
    /// NAME`, anywhere in `arguments`, which are removed from it.
    pub fn take_flags(arguments: &mut Vec<String>) -> Result<Self, String> {
        let mut out = Self::default();
        while let Some(at) = arguments.iter().position(|one| {
            matches!(
                one.as_str(),
                "--normalize"
                    | "--tbaa"
                    | "--no-delinearize"
                    | "--disable-delinearization-checks"
                    | "--miv-max-level"
                    | "--test"
            )
        }) {
            let flag = arguments.remove(at);
            match flag.as_str() {
                "--normalize" => out.normalize = true,
                "--tbaa" => out.tbaa = true,
                "--no-delinearize" => out.options.delinearize = false,
                "--disable-delinearization-checks" => out.options.disable_delinearization_checks = true,
                "--miv-max-level" => {
                    out.options.miv_max_level_threshold =
                        arguments.get(at).and_then(|one| one.parse().ok()).ok_or("--miv-max-level takes a number")?;
                    arguments.remove(at);
                }
                _ => {
                    out.options.enable_test = arguments
                        .get(at)
                        .and_then(|one| TestType::named(one))
                        .ok_or("--test takes a dependence test name")?;
                    arguments.remove(at);
                }
            }
        }
        Ok(out)
    }
}

/// `print<da>` of every function of `module` that has a body.
pub fn print_module_da(
    module: &Module,
    print: &PrintOptions,
) -> String {
    let PrintOptions { options, normalize, tbaa } = *print;
    let layout = module
        .datalayout
        .as_deref()
        .and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok())
        .unwrap_or_default();
    let lines = instruction_lines(module);
    let mut out = String::new();
    let mut defined = 0;
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let function_lines = &lines[defined];
        defined += 1;
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
        let unit = Unit::of(module, &layout, function);
        let labels = Labels::of(function);
        let mut se = ScalarEvolution::new();
        // `-aa-pipeline=basic-aa`, or with `tbaa` (the default pipeline has
        // scoped-noalias too, which MIR has no metadata for).
        let mut aa = AggregateAA::new();
        aa.add_aa_result(BasicAA::new(&unit));
        if tbaa {
            aa.add_aa_result(TypeBasedAA::new(&unit));
        }
        let mut info = DependenceInfo::new(&mut aa, &unit, &cx, options);
        let text: Vec<(InstId, &String)> = function.walk().map(|(_, inst)| inst).zip(function_lines).collect();
        let query = cx.query(None);
        let access =
            |inst| Access { reads: may_read_from_memory(&query, inst), writes: may_write_to_memory(&query, inst) };

        let _ = writeln!(
            out,
            "Printing analysis 'Dependence Analysis' for function '{}':",
            global.name.as_deref().unwrap_or("?")
        );
        for (at, &(src, src_text)) in text.iter().enumerate() {
            let src_access = access(src);
            if !(src_access.reads || src_access.writes) {
                continue;
            }
            for &(dst, dst_text) in &text[at..] {
                let dst_access = access(dst);
                if !(dst_access.reads || dst_access.writes) {
                    continue;
                }
                let _ = writeln!(out, "Src:{src_text} --> Dst:{dst_text}");
                out += "  da analyze - ";
                match info.depends(&mut se, src, dst, true) {
                    Some(mut dependence) => {
                        // Normalize negative direction vectors if required by
                        // clients.
                        if normalize && dependence.normalize(&mut se, &cx) {
                            out += "normalized - ";
                        }
                        // A normalized dependence has swapped its ends.
                        let (first, second) = (access(dependence.src()), access(dependence.dst()));
                        out += &dependence.dump(&se, &cx, &labels, &first, &second);
                    }
                    None => out += "none!\n",
                }
            }
        }
    }
    out
}
