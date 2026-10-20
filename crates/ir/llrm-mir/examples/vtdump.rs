//! Prints what ValueTracking proves of every integer and pointer value of a
//! module, one line each, in the format of `tools/llvmport/vtdump.cpp`:
//! `<function> <n> kb=... sb=...`, where `<n>` numbers the function's arguments
//! and then its instructions that have a scalar integer or pointer result.
//! usage: vtdump COLS FILE.ll...     COLS: a comma list, in this order, of kb
//! sb nz p2 p2z gp nn ng cru crs pat ov

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{GlobalKind, Operand};
use llrm_mir::parse;
use llrm_mir::valuetracking::{AssumptionCache, Query};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cols: Vec<&str> = args[0].split(',').collect();
    for path in &args[1..] {
        let text = std::fs::read_to_string(path).expect("a readable file");
        let module = parse::module(&text).unwrap_or_else(|why| panic!("{path}: {why}"));
        let layout =
            module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a datalayout")).unwrap_or_default();
        for global in &module.globals {
            let GlobalKind::Function(function) = &global.kind else { continue };
            if function.is_declaration() {
                continue;
            }
            let name = global.name.owned().unwrap_or_default();
            let dt = DomTree::of(function);
            let assumptions = AssumptionCache::new(&module, function);
            let mut values: Vec<(Operand, Option<llrm_mir::module::InstId>)> = Vec::new();
            let entry_first = function.block(function.entry().expect("an entry")).first();
            for parameter in function.parameters() {
                values.push((Operand::Value(*parameter), entry_first));
            }
            for (_, inst) in function.walk() {
                if let Some(result) = function.instruction(inst).result {
                    values.push((Operand::Value(result), Some(inst)));
                }
            }
            let mut n = 0;
            for (value, cxt) in values {
                let mut query = Query::new(&module, function, &layout).with_cxt(cxt);
                query.dt = Some(&dt);
                query.assumptions = Some(&assumptions);
                if query.scalar_bits(value) == 0 || query.scalar_bits(value) > 128 {
                    continue;
                }
                let mut line = format!("{name} {n}");
                n += 1;
                for col in &cols {
                    line.push_str(&llrm_mir::valuetracking::column(&query, value, col));
                }
                println!("{line}");
            }
        }
    }
}
