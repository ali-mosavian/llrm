//! `llrm-mir FILE`: reads and verifies MIR and writes it back, as
//! `tools/mir-oracle.sh` compares with LLVM's own reading. `llrm-mir --run
//! [--entry NAME] [--fuel N] FILE` runs its `@main` (or `@NAME`) on at most
//! N instructions instead, and prints what it returns. `llrm-mir --ivs FILE`
//! prints, per function and innermost loop, how many of the loop's header
//! phis ScalarEvolution proves step by a nonzero amount: its induction
//! variables, as tools/loops counts them. `llrm-mir --aliases FILE` prints, per
//! function, the answer today's points-to facts give for every pair of pointer
//! operands of its loads, stores and calls, one `NoAlias|MayAlias|MustAlias
//! %a %b` line a pair, in the order `opt -passes=aa-eval` lists them, for
//! tools/aliascompare.py. `llrm-mir --loops FILE` prints `print<loops>`'s text
//! for each function. `llrm-mir --unroll N FILE`
//! writes the module with every innermost loop that leaves from one
//! conditional block unrolled completely with `N` copies
//! (`tools/unroll_oracle.py` compares it with the pinned `opt`).

use std::process::ExitCode;

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let run = arguments.first().is_some_and(|one| one == "--run");
    if run {
        arguments.remove(0);
    }
    let ivs = arguments.first().is_some_and(|one| one == "--ivs");
    if ivs {
        arguments.remove(0);
    }
    let aliases_wanted = arguments.first().is_some_and(|one| one == "--aliases");
    if aliases_wanted {
        arguments.remove(0);
    }
    let memops_wanted = arguments.first().is_some_and(|one| one == "--memops");
    if memops_wanted {
        arguments.remove(0);
    }
    let calls_wanted = arguments.first().is_some_and(|one| one == "--calls");
    if calls_wanted {
        arguments.remove(0);
    }
    let scev = arguments.first().is_some_and(|one| one == "--scev");
    if scev {
        arguments.remove(0);
    }
    let values = arguments.first().is_some_and(|one| one == "--scev-values");
    if values {
        arguments.remove(0);
    }
    let division = arguments.first().is_some_and(|one| one == "--scev-division");
    if division {
        arguments.remove(0);
    }
    let delinearize = arguments.first().is_some_and(|one| one == "--delinearize");
    if delinearize {
        arguments.remove(0);
    }
    let da = arguments.first().is_some_and(|one| one == "--da");
    let mut da_print = llrm_analysis::depanalysis::print::PrintOptions::default();
    if da {
        arguments.remove(0);
        match llrm_analysis::depanalysis::print::PrintOptions::take_flags(&mut arguments) {
            Ok(found) => da_print = found,
            Err(why) => {
                eprintln!("--da: {why}");
                return ExitCode::FAILURE;
            }
        }
    }
    let counts = arguments.first().is_some_and(|one| one == "--scev-counts");
    if counts {
        arguments.remove(0);
    }
    let branch_prob = arguments.first().is_some_and(|one| one == "--branch-prob");
    if branch_prob {
        arguments.remove(0);
    }
    let loops = arguments.first().is_some_and(|one| one == "--loops");
    if loops {
        arguments.remove(0);
    }
    let mut exits = Vec::<(String, u32)>::new();
    while let Some(at) = arguments.iter().position(|one| one == "--exit") {
        let spec = arguments
            .get(at + 1)
            .and_then(|one| one.split_once('='))
            .and_then(|(block, trips)| Some((block.to_owned(), trips.parse::<u32>().ok()?)));
        arguments.drain(at..(at + 2).min(arguments.len()));
        exits.extend(spec);
    }
    let unroll = {
        let at = arguments.iter().position(|one| one == "--unroll");
        at.and_then(|at| {
            let spec = arguments
                .get(at + 1)
                .and_then(|one| one.split_once(':'))
                .and_then(|(header, count)| Some((header.to_owned(), count.parse::<u32>().ok()?)));
            arguments.drain(at..(at + 2).min(arguments.len()));
            spec
        })
    };
    let rotate = {
        let at = arguments.iter().position(|one| one == "--rotate");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let dse = {
        let at = arguments.iter().position(|one| one == "--dse");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let mode = {
        let at = arguments.iter().position(|one| one == "--mode");
        at.map(|at| {
            let value = arguments.get(at + 1).cloned().unwrap_or_default();
            arguments.drain(at..(at + 2).min(arguments.len()));
            value
        })
    };
    let non_trivial = {
        let at = arguments.iter().position(|one| one == "--non-trivial");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let simple_loop_unswitch = {
        let at = arguments.iter().position(|one| one == "--simple-loop-unswitch");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let loop_deletion = {
        let at = arguments.iter().position(|one| one == "--loop-deletion");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let loop_interchange = {
        let at = arguments.iter().position(|one| one == "--loop-interchange");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let interchange_options = if loop_interchange {
        match llrm_transforms::loopinterchange::Options::take_flags(&mut arguments) {
            Ok(found) => found,
            Err(why) => {
                eprintln!("--loop-interchange: {why}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        Default::default()
    };
    let loop_unroll_and_jam = {
        let at = arguments.iter().position(|one| one == "--loop-unroll-and-jam");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let jam_options = if loop_unroll_and_jam {
        match llrm_transforms::loopunrollandjam::JamOptions::take_flags(&mut arguments) {
            Ok(found) => found,
            Err(why) => {
                eprintln!("--loop-unroll-and-jam: {why}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        Default::default()
    };
    let unroll_full_os = {
        let at = arguments.iter().position(|one| one == "--unroll-full-os");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let unroll_full = {
        let at = arguments.iter().position(|one| one == "--unroll-full");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let memcpyopt = {
        let at = arguments.iter().position(|one| one == "--memcpyopt");
        at.inspect(|&at| {
            arguments.remove(at);
        })
        .is_some()
    };
    let mut option = |flag: &str| {
        let at = arguments.iter().position(|one| one == flag)?;
        let value = arguments.get(at + 1).cloned();
        arguments.drain(at..(at + 2).min(arguments.len()));
        value
    };
    let entry = option("--entry").unwrap_or_else(|| "main".to_owned());
    let Ok(fuel) = option("--fuel").map_or(Ok(10_000_000), |one| one.parse::<u64>()) else {
        eprintln!("llrm-mir: --fuel takes a count");
        return ExitCode::from(2);
    };
    let Some(path) = arguments.first().cloned() else {
        eprintln!("usage: llrm-mir [--run [--entry NAME] [--fuel N]] FILE");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(2);
        }
    };
    match llrm_mir::parse::module(&text) {
        Ok(mut module) => {
            // A pass input is a module of one function body and the
            // declarations it names, which the verifier does not
            // take for a whole module.
            if aliases_wanted {
                print!("{}", aliases(&module));
                return ExitCode::SUCCESS;
            }
            if memops_wanted {
                print!("{}", memops(&module));
                return ExitCode::SUCCESS;
            }
            if calls_wanted {
                print!("{}", calls(&module));
                return ExitCode::SUCCESS;
            }
            let problems = llrm_mir::verify::verify(&module);
            if !problems.is_empty() {
                problems.iter().for_each(|one| eprintln!("{path}: {one}"));
                return ExitCode::FAILURE;
            }
            if scev {
                print!("{}", scalar_evolution(&module));
                return ExitCode::SUCCESS;
            }
            if values {
                print!("{}", llrm_analysis::scev::values::print_module_values(&module));
                return ExitCode::SUCCESS;
            }
            if let Some((header, count)) = unroll {
                if let Err(error) = unrolled_loop(&mut module, &header, count, &exits) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if rotate {
                let tti = llrm_x86::tti::X86::of_cpu("i486").expect("a cpu");
                let options = llrm_transforms::looprotate::Options {
                    max_header_size: 16,
                    rotation_only: false,
                    is_util_mode: false,
                    prepare_for_lto: false,
                };
                // Every function with a body.
                let names: Vec<String> = module
                    .functions()
                    .filter(|(_, _, one)| !one.layout().is_empty())
                    .filter_map(|(_, one, _)| one.name.owned())
                    .collect();
                for name in names {
                    // The caller has made the loops simplified and LCSSA, as
                    // the loop pass manager does.
                    if let Err(error) = llrm_transforms::looprotate::rotate_function(&mut module, &name, &tti, options)
                    {
                        eprintln!("{path}: @{name}: {error}");
                        return ExitCode::FAILURE;
                    }
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if dse {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::deadstore::DeadStoreElimination);
                if let Err(error) = manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if simple_loop_unswitch {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::simpleloopunswitch::SimpleLoopUnswitch { non_trivial });
                // `--mode N`: the target's datalayout and TTI price the loop,
                // as in llrm-pass.
                let mut machine: std::rc::Rc<dyn llrm_mir::target::Machine> =
                    std::rc::Rc::new(llrm_mir::target::Neutral);
                if let Some(mode) = &mode {
                    match llrm_driver::mode_target(mode, false) {
                        Ok(target) => {
                            module.datalayout = Some(target.datalayout);
                            machine = target.machine;
                        }
                        Err(error) => {
                            eprintln!("llrm-mir: {error}");
                            return ExitCode::from(2);
                        }
                    }
                }
                if let Err(error) = manager.run_module(&mut module, machine) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if loop_unroll_and_jam {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::loopunrollandjam::LoopUnrollAndJam {
                    opt_level: 2,
                    opt_size: false,
                    options: jam_options.clone(),
                });
                let tti: std::rc::Rc<dyn llrm_mir::target::Machine> = std::rc::Rc::new(llrm_mir::target::Neutral);
                if let Err(error) = manager.run_module(&mut module, tti) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if loop_interchange {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::loopinterchange::Interchange { options: interchange_options.clone() });
                if let Err(error) = manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if loop_deletion {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::loopdeletion::LoopDeletion);
                if let Err(error) = manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if unroll_full || unroll_full_os {
                let tti = llrm_x86::tti::X86::of_cpu("i486").expect("a cpu");
                let options = llrm_transforms::loopunrollpass::Options {
                    opt_level: 2,
                    opt_size: unroll_full_os,
                    min_size: false,
                    only_when_forced: false,
                    prepare_for_lto: false,
                };
                let names: Vec<String> = module
                    .functions()
                    .filter(|(_, _, one)| !one.layout().is_empty())
                    .filter_map(|(_, one, _)| one.name.owned())
                    .collect();
                for name in names {
                    if let Err(error) =
                        llrm_transforms::loopunrollpass::unroll_function(&mut module, &name, &tti, options)
                    {
                        eprintln!("{path}: @{name}: {error}");
                        return ExitCode::FAILURE;
                    }
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if memcpyopt {
                let mut manager = llrm_mir::passes::PassManager::default();
                manager.add(llrm_transforms::memcpyoptimizer::MemCpyOptimizer);
                if let Err(error) = manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)) {
                    eprintln!("{path}: {error}");
                    return ExitCode::FAILURE;
                }
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            if division {
                print!("{}", llrm_analysis::scev::division::print_module_division(&module));
                return ExitCode::SUCCESS;
            }
            if delinearize {
                print!("{}", llrm_analysis::scev::delinearize::print_module_delinearization(&module));
                return ExitCode::SUCCESS;
            }
            if da {
                print!("{}", llrm_analysis::depanalysis::print::print_module_da(&module, &da_print));
                return ExitCode::SUCCESS;
            }
            if counts {
                print!("{}", llrm_analysis::scev::values::print_module_counts(&module));
                return ExitCode::SUCCESS;
            }
            if branch_prob {
                print!("{}", llrm_analysis::bpi::print_module_branch_prob(&module));
                return ExitCode::SUCCESS;
            }
            if loops {
                print!("{}", llrm_mir::loopinfo::print_loops(&module));
                return ExitCode::SUCCESS;
            }
            if ivs {
                print!("{}", induction_variables(&module));
                return ExitCode::SUCCESS;
            }
            if !run {
                print!("{}", llrm_mir::print::module(&module));
                return ExitCode::SUCCESS;
            }
            match llrm_mir::interpret::run(&module, &entry, Vec::new(), fuel) {
                Ok(llrm_mir::interpret::Val::Int { bits, .. }) => {
                    println!("{bits}");
                    ExitCode::SUCCESS
                }
                Ok(other) => {
                    eprintln!("{path}: @{entry} returned {other:?}");
                    ExitCode::FAILURE
                }
                Err(trap) => {
                    eprintln!("{path}: {trap:?}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(error) => {
            eprintln!("{path}:{error}");
            ExitCode::FAILURE
        }
    }
}

/// The expressions on stdin, one per line, in the language of
/// `tools/scev/oracle.cpp`, evaluated by the first function with a body.
fn scalar_evolution(module: &llrm_mir::module::Module) -> String {
    use std::io::BufRead;
    let (_, _, function) =
        module.functions().find(|(_, _, function)| !function.layout().is_empty()).expect("a function");
    let layout = module
        .datalayout
        .as_deref()
        .and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok())
        .unwrap_or_default();
    let tree = llrm_mir::domtree::DomTree::of(function);
    let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
    let assumptions = llrm_mir::valuetracking::AssumptionCache::new(module, function);
    let cx = llrm_analysis::scev::Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let labels = llrm_analysis::scev::print::Labels::of(function);
    let mut scalar_evolution = llrm_analysis::scev::ScalarEvolution::new();
    let mut out = String::new();
    for line in std::io::stdin().lock().lines() {
        out += &scalar_evolution.eval_line(&cx, &labels, &line.expect("a line"));
        out.push('\n');
    }
    out
}

/// `function<TAB>loop<TAB>count` per innermost loop, loops in LoopInfo's order.
fn induction_variables(module: &llrm_mir::module::Module) -> String {
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || llrm_mir::domtree::DomTree::of(function));
        let evolution = llrm_mir::scalarevolution::Evolution::new(&module.context, function, &loops);
        // Larger loops first, by header among the same size: the order the
        // numbers in tools/loops were counted in.
        let mut inner: Vec<_> = loops.get_loops_in_preorder().into_iter().filter(|one| one.is_innermost()).collect();
        inner.sort_by_key(|one| (std::cmp::Reverse(one.get_num_blocks()), one.get_header()));
        for (number, one) in inner.into_iter().enumerate() {
            let count = evolution.counted(&module.context, function, &loops, one.get_header());
            out += &format!("{}\t{number}\t{count}\n", global.name.as_deref().unwrap_or("?"));
        }
    }
    out
}

/// `function<TAB>answer<TAB>%a<TAB>bytes<TAB>%b<TAB>bytes` per pair of the
/// pointer values that load, store and call operands name (a value, in order of
/// first use), by what the points-to facts of the body say: `NoAlias` where no
/// object they point into is shared (offsets and widths not read, as aa-eval
/// asks without a size), `MustAlias` for one value, else `MayAlias`.
fn aliases(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::{MemRef, Unit};
    use llrm_mir::context::ConstantKind;
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(
            llrm_mir::datalayout::DataLayout::default,
            |text| llrm_mir::datalayout::DataLayout::parse(text).expect("a datalayout"),
        );
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let plain = Unit::of(module, &layout, function);
        let known = llrm_analysis::consts::known(&Unit { registers: None, ..plain });
        let shape = llrm_analysis::cfg::Shape::of(function);
        let unit = plain.with_registers(&known).with_shape(&shape);
        let Ok(facts) = llrm_analysis::alias::points_to(&unit, None, None) else { continue };
        let names = llrm_mir::print::value_labels(function);
        // A pointer value or a global's address, as aa-eval counts them; what
        // the text calls it.
        let label = |operand: Operand| -> Option<String> {
            match operand {
                Operand::Value(value) => Some(format!("%{}", names.get(&value)?)),
                Operand::Constant(id) => match module.context.get(id).kind {
                    ConstantKind::Global(at) => Some(format!(
                        "@{}",
                        llrm_mir::print::quoted(module.globals.get(at.0 as usize)?.name.as_deref()?)
                    )),
                    _ => None,
                },
                Operand::Block(_) => None,
            }
        };
        // aa-eval's pointers: the pointer operand of each load and store, with
        // the size of what is accessed; one entry for each pointer and size.
        let mut entries: Vec<(String, u32, MemRef, Operand)> = Vec::new();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let pointer = match &op.opcode {
                Opcode::Load { .. } => op.operands[0],
                Opcode::Store { .. } => op.operands[1],
                _ => continue,
            };
            let (Some(text), Some(mut reference)) = (label(pointer), MemRef::of(&unit, inst)) else { continue };
            if entries.iter().any(|(had, width, ..)| *had == text && *width == reference.width) {
                continue;
            }
            // The bytes this access reaches, as the consumers of the facts read
            // them.
            reference.provenance = facts.reference(&unit, &reference).or(reference.provenance.take());
            // Without the type tags: what BasicAA alone could know
            // (`LLRM_ALIASES_NO_TBAA=1`).
            if std::env::var_os("LLRM_ALIASES_NO_TBAA").is_some() {
                reference.typed = None;
            }
            entries.push((text, reference.width, reference, pointer));
        }
        // The translation of BasicAA instead of today's facts
        // (`LLRM_ALIASES_BASICAA=1`).
        let basic = std::env::var_os("LLRM_ALIASES_BASICAA").is_some();
        let mut basicaa = llrm_analysis::basicaa::BasicAA::new(&unit);
        let mut batch = llrm_analysis::aa::AAQueryInfo::default();
        for (at, one) in entries.iter().enumerate() {
            for other in &entries[at + 1..] {
                let answer = if basic {
                    use llrm_analysis::aa::{AAResults, AliasResult, LocationSize, MemoryLocation};
                    let (first, second) = (
                        MemoryLocation::new(one.3, LocationSize::Precise(u64::from(one.1))),
                        MemoryLocation::new(other.3, LocationSize::Precise(u64::from(other.1))),
                    );
                    match basicaa.alias(&first, &second, &mut batch, None) {
                        AliasResult::NoAlias => "NoAlias",
                        AliasResult::MustAlias => "MustAlias",
                        AliasResult::PartialAlias { .. } => "PartialAlias",
                        AliasResult::MayAlias => "MayAlias",
                    }
                } else {
                    match llrm_analysis::regions::overlapping(&one.2, &other.2, None, None, None) {
                        Ok(false) => "NoAlias",
                        _ => "MayAlias",
                    }
                };
                out += &format!(
                    "{}\t{answer}\t{}\t{}\t{}\t{}\n",
                    global.name.as_deref().unwrap_or("?"),
                    one.0,
                    one.1,
                    other.0,
                    other.1
                );
            }
        }
    }
    out
}

/// `function<TAB>answer` per pair `aa-eval -evaluate-aa-metadata` lists of the
/// loads and stores of a function, in its order: the answer
/// `regions::overlapping` gives the two accesses, with the type tags unless
/// `LLRM_ALIASES_NO_TBAA` is set.
fn memops(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::{MemRef, Unit};
    use llrm_mir::opcode::Opcode;
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(
            llrm_mir::datalayout::DataLayout::default,
            |text| llrm_mir::datalayout::DataLayout::parse(text).expect("a datalayout"),
        );
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let plain = Unit::of(module, &layout, function);
        let known = llrm_analysis::consts::known(&Unit { registers: None, ..plain });
        let shape = llrm_analysis::cfg::Shape::of(function);
        let unit = plain.with_registers(&known).with_shape(&shape);
        let Ok(facts) = llrm_analysis::alias::points_to(&unit, None, None) else { continue };
        let mut accesses: Vec<(bool, Option<MemRef>, Option<llrm_analysis::aa::MemoryLocation>)> = Vec::new();
        for (_, inst) in function.walk() {
            let store = match function.instruction(inst).opcode {
                Opcode::Load { .. } => false,
                Opcode::Store { .. } => true,
                _ => continue,
            };
            let Some(mut reference) = MemRef::of(&unit, inst) else {
                // Numbered all the same: a pair with it is not asked.
                accesses.push((store, None, None));
                continue;
            };
            reference.provenance = facts.reference(&unit, &reference).or(reference.provenance.take());
            if std::env::var_os("LLRM_ALIASES_NO_TBAA").is_some() {
                reference.typed = None;
            }
            accesses.push((store, Some(reference), llrm_analysis::aa::MemoryLocation::get(&unit, inst)));
        }
        // aa-eval's order: each load against each store, then each store
        // against the stores before it; one line a pair, `?` where an
        // access has no reference, so that the n-th line is the n-th
        // pair it lists.
        let basic = std::env::var_os("LLRM_ALIASES_BASICAA").is_some();
        let mut basicaa = llrm_analysis::basicaa::BasicAA::new(&unit);
        let mut batch = llrm_analysis::aa::AAQueryInfo::default();
        let mut answer =
            |one: &(bool, Option<MemRef>, Option<llrm_analysis::aa::MemoryLocation>),
             other: &(bool, Option<MemRef>, Option<llrm_analysis::aa::MemoryLocation>)| {
                if basic {
                    use llrm_analysis::aa::{AAResults, AliasResult};
                    let (Some(first), Some(second)) = (&one.2, &other.2) else { return "?" };
                    return match basicaa.alias(first, second, &mut batch, None) {
                        AliasResult::NoAlias => "NoAlias",
                        AliasResult::MustAlias => "MustAlias",
                        AliasResult::PartialAlias { .. } => "PartialAlias",
                        AliasResult::MayAlias => "MayAlias",
                    };
                }
                match (&one.1, &other.1) {
                    (Some(one), Some(other)) => match llrm_analysis::regions::overlapping(one, other, None, None, None)
                    {
                        Ok(false) => "NoAlias",
                        _ => "MayAlias",
                    },
                    _ => "?",
                }
            };
        let loads: Vec<_> = accesses.iter().filter(|one| !one.0).collect();
        let stores: Vec<_> = accesses.iter().filter(|one| one.0).collect();
        let name = global.name.as_deref().unwrap_or("?");
        for load in &loads {
            for store in &stores {
                out += &format!("{name}\t{}\n", answer(load, store));
            }
        }
        for (at, store) in stores.iter().enumerate() {
            for earlier in &stores[..at] {
                out += &format!("{name}\t{}\n", answer(store, earlier));
            }
        }
    }
    out
}

/// `function<TAB>answer` per mod/ref pair `aa-eval` lists, in its order: each
/// call against each pointer (the pointer operands of the loads and stores, one
/// per pointer and size), then each call against every other call. The answer
/// is what `calls_by_attributes` says of the other access: `Mod` where the
/// call may write what it touches, `Ref` where it may read it.
fn calls(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::{MemRef, Unit};
    use llrm_mir::opcode::Opcode;
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(
            llrm_mir::datalayout::DataLayout::default,
            |text| llrm_mir::datalayout::DataLayout::parse(text).expect("a datalayout"),
        );
    let module = module.clone();
    let names: Vec<String> = module.functions().map(|(_, global, _)| global.name.owned().unwrap_or_default()).collect();
    let mut out = String::new();
    for name in names {
        let Some((_, _, function)) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name.as_str()))
        else {
            continue;
        };
        let plain = Unit::of(&module, &layout, function);
        let known = llrm_analysis::consts::known(&Unit { registers: None, ..plain });
        let shape = llrm_analysis::cfg::Shape::of(function);
        let unit = plain.with_registers(&known).with_shape(&shape);
        let Ok(facts) = llrm_analysis::alias::points_to(&unit, None, None) else { continue };
        let Ok(values) = llrm_analysis::alias::point_values(&unit) else { continue };
        let effects = llrm_analysis::alias::calls_by_attributes(&unit.with_point_values(&values));
        let reference = |inst| {
            let mut reference = MemRef::of(&unit, inst)?;
            reference.provenance = facts.reference(&unit, &reference).or(reference.provenance.take());
            Some(reference)
        };
        // aa-eval's pointers: one entry for each pointer and size, in the order
        // of first use.
        let mut entries: Vec<(llrm_mir::module::Operand, u32, Option<MemRef>)> = Vec::new();
        let mut call_sites = Vec::new();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let pointer = match &op.opcode {
                Opcode::Load { .. } => op.operands[0],
                Opcode::Store { .. } => op.operands[1],
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    call_sites.push(inst);
                    continue;
                }
                _ => continue,
            };
            let reference = reference(inst);
            let width = reference.as_ref().map_or(0, |one| one.width);
            if entries.iter().any(|(had, had_width, _)| *had == pointer && *had_width == width) {
                continue;
            }
            entries.push((pointer, width, reference));
        }
        let touches = |inst| effects.get(&inst);
        let overlaps = |list: &[MemRef], other: &MemRef| {
            list.iter()
                .any(|one| !matches!(
                    llrm_analysis::regions::overlapping(one, other, None, None, None),
                    Ok(false)
                ))
        };
        let answer = |writes: bool, reads: bool| match (writes, reads) {
            (false, false) => "NoModRef",
            (false, true) => "Ref",
            (true, false) => "Mod",
            (true, true) => "ModRef",
        };
        let label = name.as_str();
        for &call in &call_sites {
            for (_, _, other) in &entries {
                let (writes, reads) = match (touches(call), other) {
                    (Some(effect), Some(other)) => (
                        overlaps(&effect.stores, other)
                            || effect.fills.iter().any(|fill| overlaps(std::slice::from_ref(fill), other)),
                        overlaps(&effect.loads, other),
                    ),
                    _ => {
                        out += &format!("{label}\t?\n");
                        continue;
                    }
                };
                out += &format!("{label}\t{}\n", answer(writes, reads));
            }
        }
        for &call in &call_sites {
            for &other in &call_sites {
                if call == other {
                    continue;
                }
                let (Some(one), Some(two)) = (touches(call), touches(other)) else {
                    out += &format!("{label}\t?\n");
                    continue;
                };
                let set = |effect: &llrm_analysis::alias::Effect, writes: bool| -> Vec<MemRef> {
                    if writes {
                        effect.stores.iter().cloned().chain(effect.fills.iter().cloned()).collect()
                    } else {
                        effect.loads.to_vec()
                    }
                };
                // What `call` does to the memory `other` touches: Mod if it may
                // write what the other reads or writes, Ref if
                // it may read what the other writes.
                let touched: Vec<MemRef> = set(two, true).into_iter().chain(set(two, false)).collect();
                let writes = set(one, true).iter().any(|mine| overlaps(&touched, mine));
                let reads = set(one, false).iter().any(|mine| overlaps(&set(two, true), mine));
                out += &format!("{label}\t{}\n", answer(writes, reads));
            }
        }
    }
    out
}

/// The loop headed by block `header` of the module's one function, unrolled
/// completely with `count` copies.
fn unrolled_loop(
    module: &mut llrm_mir::module::Module,
    header: &str,
    count: u32,
    exits: &[(String, u32)],
) -> Result<(), String> {
    use llrm_analysis::cfg;
    let name = module
        .functions()
        .find(|(_, _, one)| !one.layout().is_empty())
        .and_then(|(_, one, _)| one.name.owned())
        .ok_or("no function")?;
    let (_, function) = module.function_mut(&name).ok_or("no function")?;
    llrm_transforms::lcssa::closed(function)?;
    let named = |function: &llrm_mir::module::Function, label: &str| {
        function
            .layout()
            .iter()
            .copied()
            .find(|&one| function.block(one).name.as_deref() == Some(label))
            .ok_or(format!("no block {label}"))
    };
    let at = cfg::id(named(function, header)?);
    let shape = cfg::Shape::of(function);
    let loop_ = shape.loops.iter().find(|one| one.header == at).ok_or("no loop at that header")?;
    let exits = exits
        .iter()
        .map(|(label, trip_count)| {
            Ok(llrm_transforms::loopunroll::Exit { block: named(function, label)?, trip_count: *trip_count })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let options = llrm_transforms::loopunroll::Options { count, max_trip_count: count, max_or_zero: false, exits };
    llrm_transforms::loopunroll::unroll_loop(function, loop_, &options)?;
    Ok(())
}
