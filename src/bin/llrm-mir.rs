//! `llrm-mir FILE`: reads and verifies MIR and writes it back, as
//! `tools/mir-oracle.sh` compares with LLVM's own reading. `llrm-mir --run
//! [--entry NAME] [--fuel N] FILE` runs its `@main` (or `@NAME`) on at most
//! N instructions instead, and prints what it returns. `llrm-mir --ivs FILE`
//! prints, per function and innermost loop, how many of the loop's header
//! phis ScalarEvolution proves step by a nonzero amount: its induction
//! variables, as tools/loops counts them. `llrm-mir --loops FILE` prints
//! `print<loops>`'s text for each function.

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
    let scev = arguments.first().is_some_and(|one| one == "--scev");
    if scev {
        arguments.remove(0);
    }
    let loops = arguments.first().is_some_and(|one| one == "--loops");
    if loops {
        arguments.remove(0);
    }
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
        Ok(module) => {
            let problems = llrm_mir::verify::verify(&module);
            if !problems.is_empty() {
                problems.iter().for_each(|one| eprintln!("{path}: {one}"));
                return ExitCode::FAILURE;
            }
            if scev {
                print!("{}", scalar_evolution(&module));
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
    let tree = llrm_mir::dominators::DominatorTree::new(function);
    let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
    let assumptions = llrm_mir::valuetracking::AssumptionCache::new(module, function);
    let cx = llrm_analysis::scev::Cx {
        module,
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
        let loops =
            llrm_mir::loopinfo::LoopInfo::analyze(function, || llrm_mir::dominators::DominatorTree::new(function));
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
