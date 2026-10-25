//! `alias_pairs DUMP.ll [--cap N]`: the alias stack's answers for a module
//! dump, in the form `llrm_analysis::aliasdiff::compare` reads (see its
//! header). `alias_pairs --diff OLD NEW` compares two recordings and exits 1
//! on a gained proof or an unmatched row. Functions with more than `--cap`
//! (default 250) loads and stores are cut to their first `cap`.
use llrm_analysis::aliasdiff;
use llrm_analysis::memory::Unit;
use llrm_analysis::memoryssa::Accesses;
use llrm_analysis::mssa::alias::{AliasQuery, AliasResult};
use llrm_analysis::testing::layout;
use llrm_mir::opcode::Opcode;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--diff") {
        let read =
            |at: usize| std::fs::read_to_string(&args[at]).unwrap_or_else(|error| panic!("{}: {error}", args[at]));
        let report = aliasdiff::compare(&read(2), &read(3));
        println!(
            "same {} lost {} changed {} gained {} unmatched {}",
            report.same,
            report.lost,
            report.changed,
            report.gained.len(),
            report.unmatched.len()
        );
        for line in report.gained.iter().chain(&report.unmatched).take(40) {
            println!("FAIL {line}");
        }
        std::process::exit(i32::from(!report.passes()));
    }
    let cap = args.iter().position(|arg| arg == "--cap").map_or(250, |at| args[at + 1].parse().expect("a number"));
    let text = std::fs::read_to_string(&args[1]).expect("a readable dump");
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let layout = layout(&module);
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let name = global.name.owned().unwrap_or_default();
        let unit = llrm_analysis::testing::with_registers(Unit::of(&module, &layout, function));
        let accesses = Accesses::plain(&unit, &Default::default());
        let query = AliasQuery::new(&unit, &accesses);
        let mut places = Vec::new();
        let mut calls = Vec::new();
        for (at, (_, inst)) in function.walk().enumerate() {
            match function.instruction(inst).opcode {
                Opcode::Load { .. } | Opcode::Store { .. } => {
                    if places.len() < cap
                        && let Some(location) = query.location_of(inst)
                    {
                        places.push((at, inst, location));
                    }
                }
                Opcode::Call(_) | Opcode::Invoke(_) => calls.push((at, inst)),
                _ => {}
            }
        }
        for (i, (at, _, one)) in places.iter().enumerate() {
            let row: String = places[i + 1..]
                .iter()
                .map(|(_, _, other)| match query.alias(one, other) {
                    AliasResult::No => 'N',
                    AliasResult::May => 'M',
                    AliasResult::Partial => 'P',
                    AliasResult::Must => 'U',
                })
                .collect();
            println!("A {name} {at} {row}");
        }
        for (at, call) in &calls {
            let row: String = places
                .iter()
                .map(|(_, _, location)| {
                    let found = query.get_mod_ref_info(*call, location);
                    match (found.reads, found.writes) {
                        (false, false) => '0',
                        (true, false) => 'R',
                        (false, true) => 'W',
                        (true, true) => 'B',
                    }
                })
                .collect();
            println!("C {name} {at} {row}");
        }
    }
}
