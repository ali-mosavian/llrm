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
//! tools/aliascompare.py.

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
            // A pass input is a module of one function body and the declarations it
            // names, which the verifier does not take for a whole module.
            if aliases_wanted {
                print!("{}", aliases(&module));
                return ExitCode::SUCCESS;
            }
            if memops_wanted {
                print!("{}", memops(&module));
                return ExitCode::SUCCESS;
            }
            let problems = llrm_mir::verify::verify(&module);
            if !problems.is_empty() {
                problems.iter().for_each(|one| eprintln!("{path}: {one}"));
                return ExitCode::FAILURE;
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
        let tree = llrm_mir::dominators::DominatorTree::new(function);
        let loops = llrm_mir::loops::LoopInfo::new(function, &tree);
        let evolution = llrm_mir::scalarevolution::Evolution::new(&module.context, function, &loops);
        let inner = (0..loops.loops.len()).filter(|&at| !loops.loops.iter().any(|one| one.parent == Some(at)));
        for (number, at) in inner.enumerate() {
            let header = loops.loops[at].header;
            let count = evolution.counted(&module.context, function, &loops, header);
            out += &format!("{}\t{number}\t{count}\n", global.name.as_deref().unwrap_or("?"));
        }
    }
    out
}

/// `function<TAB>answer<TAB>%a<TAB>bytes<TAB>%b<TAB>bytes` per pair of the pointer values that
/// load, store and call operands name (a value, in order of first use), by what
/// the points-to facts of the body say: `NoAlias` where no object they point
/// into is shared (offsets and widths not read, as aa-eval asks without a size), `MustAlias` for one value, else `MayAlias`.
fn aliases(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::{MemRef, Unit};
    use llrm_mir::context::ConstantKind;
    use llrm_mir::module::Operand;
    use llrm_mir::opcode::Opcode;
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(llrm_mir::datalayout::DataLayout::default, |text| {
            llrm_mir::datalayout::DataLayout::parse(text).expect("a datalayout")
        });
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
        let names = llrm_mir::print::value_names(function);
        // A pointer value or a global's address, as aa-eval counts them; what the
        // text calls it.
        let label = |operand: Operand| -> Option<String> {
            match operand {
                Operand::Value(value) => Some(format!("%{}", names.get(&value)?)),
                Operand::Constant(id) => match module.context.get(id).kind {
                    ConstantKind::Global(at) => {
                        Some(format!("@{}", llrm_mir::print::quoted(module.globals.get(at.0 as usize)?.name.as_deref()?)))
                    }
                    _ => None,
                },
                Operand::Block(_) => None,
            }
        };
        // aa-eval's pointers: the pointer operand of each load and store, with
        // the size of what is accessed; one entry for each pointer and size.
        let mut entries: Vec<(String, u32, MemRef)> = Vec::new();
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            let pointer = match &op.opcode {
                Opcode::Load { .. } => op.operands[0],
                Opcode::Store { .. } => op.operands[1],
                _ => continue,
            };
            let (Some(text), Some(mut reference)) = (label(pointer), MemRef::of(&unit, inst)) else { continue };
            if entries.iter().any(|(had, width, _)| *had == text && *width == reference.width) {
                continue;
            }
            // The bytes this access reaches, as the consumers of the facts read them.
            reference.provenance = facts.reference(&unit, &reference).or(reference.provenance.take());
            // Without the type tags: what BasicAA alone could know (`LLRM_ALIASES_NO_TBAA=1`).
            if std::env::var_os("LLRM_ALIASES_NO_TBAA").is_some() {
                reference.typed = None;
            }
            entries.push((text, reference.width, reference));
        }
        for (at, one) in entries.iter().enumerate() {
            for other in &entries[at + 1..] {
                let answer = match llrm_analysis::regions::overlapping(&one.2, &other.2, None, None, None) {
                    Ok(false) => "NoAlias",
                    _ => "MayAlias",
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

/// `function<TAB>answer` per pair `aa-eval -evaluate-aa-metadata` lists of the loads
/// and stores of a function, in its order: the answer `regions::overlapping` gives the two
/// accesses, with the type tags unless `LLRM_ALIASES_NO_TBAA` is set.
fn memops(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::{MemRef, Unit};
    use llrm_mir::opcode::Opcode;
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(llrm_mir::datalayout::DataLayout::default, |text| {
            llrm_mir::datalayout::DataLayout::parse(text).expect("a datalayout")
        });
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
        let mut accesses: Vec<(bool, Option<MemRef>)> = Vec::new();
        for (_, inst) in function.walk() {
            let store = match function.instruction(inst).opcode {
                Opcode::Load { .. } => false,
                Opcode::Store { .. } => true,
                _ => continue,
            };
            let Some(mut reference) = MemRef::of(&unit, inst) else {
                // Numbered all the same: a pair with it is not asked.
                accesses.push((store, None));
                continue;
            };
            reference.provenance = facts.reference(&unit, &reference).or(reference.provenance.take());
            if std::env::var_os("LLRM_ALIASES_NO_TBAA").is_some() {
                reference.typed = None;
            }
            accesses.push((store, Some(reference)));
        }
        // aa-eval's order: each load against each store, then each store against
        // the stores before it; one line a pair, `?` where an access has no
        // reference, so that the n-th line is the n-th pair it lists.
        let answer = |one: &Option<MemRef>, other: &Option<MemRef>| match (one, other) {
            (Some(one), Some(other)) => match llrm_analysis::regions::overlapping(one, other, None, None, None) {
                Ok(false) => "NoAlias",
                _ => "MayAlias",
            },
            _ => "?",
        };
        let loads: Vec<_> = accesses.iter().filter(|one| !one.0).collect();
        let stores: Vec<_> = accesses.iter().filter(|one| one.0).collect();
        let name = global.name.as_deref().unwrap_or("?");
        for load in &loads {
            for store in &stores {
                out += &format!("{name}\t{}\n", answer(&load.1, &store.1));
            }
        }
        for (at, store) in stores.iter().enumerate() {
            for earlier in &stores[..at] {
                out += &format!("{name}\t{}\n", answer(&store.1, &earlier.1));
            }
        }
    }
    out
}
