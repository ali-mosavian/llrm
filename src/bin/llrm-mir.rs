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
    let loops = arguments.first().is_some_and(|one| one == "--loops");
    if loops {
        arguments.remove(0);
    }
    let unroll = {
        let at = arguments.iter().position(|one| one == "--unroll");
        at.and_then(|at| {
            let count = arguments.get(at + 1).and_then(|one| one.parse::<u32>().ok());
            arguments.drain(at..(at + 2).min(arguments.len()));
            count
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
            if let Some(count) = unroll {
                unrolled_innermost(&mut module, count);
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

/// Every innermost loop with one exiting conditional block, unrolled completely
/// with `count` copies.
fn unrolled_innermost(
    module: &mut llrm_mir::module::Module,
    count: u32,
) {
    use llrm_analysis::cfg;
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, one)| !one.layout().is_empty())
        .filter_map(|(_, one, _)| one.name.clone())
        .collect();
    for name in names {
        let (_, function) = module.function_mut(&name).expect("a function");
        llrm_transforms::lcssa::closed(function).expect("closed SSA form");
        let shape = cfg::Shape::of(function);
        for loop_ in shape.loops.iter().filter(|one| {
            !shape.loops.iter().any(|other| other.header != one.header && one.body.contains(&other.header))
        }) {
            let exiting: Vec<_> = loop_
                .body
                .iter()
                .map(|&at| cfg::block(at))
                .filter(|&at| function.successors(at).iter().any(|&to| !loop_.body.contains(&cfg::id(to))))
                .collect();
            let [block] = exiting[..] else { continue };
            let options = llrm_transforms::loopunroll::Options {
                count,
                max_trip_count: count,
                max_or_zero: false,
                exits: vec![llrm_transforms::loopunroll::Exit { block, trip_count: count }],
            };
            llrm_transforms::loopunroll::unroll_loop(function, loop_, &options).expect("an unroll");
        }
    }
}
