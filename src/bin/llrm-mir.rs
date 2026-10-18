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

/// `function<TAB>answer<TAB>%a<TAB>%b` per pair of the pointer values that
/// load, store and call operands name (a value, in order of first use), by what
/// the points-to facts of the body say: `NoAlias` where no object they point
/// into is shared (offsets and widths not read, as aa-eval asks without a size), `MustAlias` for one value, else `MayAlias`.
fn aliases(module: &llrm_mir::module::Module) -> String {
    use llrm_analysis::memory::Unit;
    use llrm_mir::module::{Operand, ValueId};
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
        let mut pointers: Vec<ValueId> = Vec::new();
        let note = |operand: Operand, pointers: &mut Vec<ValueId>| {
            if let Operand::Value(value) = operand
                && function.operand_type(&module.context, operand).is_some_and(|ty| matches!(module.context.types.get(ty), llrm_mir::types::Type::Pointer(_)))
                && !pointers.contains(&value)
            {
                pointers.push(value);
            }
        };
        for (_, inst) in function.walk() {
            let op = function.instruction(inst);
            match &op.opcode {
                Opcode::Load { .. } => note(op.operands[0], &mut pointers),
                Opcode::Store { .. } => note(op.operands[1], &mut pointers),
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    for &operand in &op.operands {
                        note(operand, &mut pointers);
                    }
                }
                _ => {}
            }
        }
        for (at, &one) in pointers.iter().enumerate() {
            for &other in &pointers[at + 1..] {
                // aa-eval asks with no size (`beforeOrAfterPointer`), so any byte of an
                // object a pointer is into may be the one: NoAlias is a pair with no
                // object in common, offsets and widths not read.
                let objects = |value: &ValueId| {
                    facts.values.get(value).map(|provenance| {
                        provenance.slices.iter().map(|one| one.object).collect::<std::collections::BTreeSet<_>>()
                    })
                };
                let answer = match (objects(&one), objects(&other)) {
                    (Some(left), Some(right))
                        if left.is_disjoint(&right)
                            && !left.iter().chain(&right).any(|object| {
                                matches!(object.kind, llrm_analysis::memory::MemoryKind::Unknown)
                            }) =>
                    {
                        "NoAlias"
                    }
                    _ => "MayAlias",
                };
                out += &format!(
                    "{}\t{answer}\t%{}\t%{}\n",
                    global.name.as_deref().unwrap_or("?"),
                    names[&one],
                    names[&other]
                );
            }
        }
    }
    out
}
