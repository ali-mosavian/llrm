//! `llrm-mir FILE`: reads and verifies MIR and writes it back, as
//! `tools/mir-oracle.sh` compares with LLVM's own reading. `llrm-mir --run
//! [--entry NAME] [--fuel N] FILE` runs its `@main` (or `@NAME`) on at most
//! N instructions instead, and prints what it returns. `llrm-mir --ivs FILE`
//! prints, per function and innermost loop, how many of the loop's header
//! phis ScalarEvolution proves step by a nonzero amount: its induction
//! variables, as tools/loops counts them. `llrm-mir --loops FILE` prints
//! `print<loops>`'s text for each function. `llrm-mir --unroll N FILE`
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
    let scev = arguments.first().is_some_and(|one| one == "--scev");
    if scev {
        arguments.remove(0);
    }
    let values = arguments.first().is_some_and(|one| one == "--scev-values");
    if values {
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
        .and_then(|(_, one, _)| one.name.clone())
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
