//! Lists, for each float value `fpdump` numbers, what its operand tree holds
//! that `valuetracking::fp` does not translate yet, as `<function> <n> <why>`:
//! `intrinsic:NAME`, `bitcast`, `vector`, `load`, `call`, `assume` (the
//! function has an `llvm.assume`), `select` (a select arm, narrowed by its
//! condition in the pin), or `-` for none (a gap in the translated cases).
//! usage: fpwhy FILE.ll...

use std::collections::BTreeSet;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{GlobalKind, Operand};
use llrm_mir::opcode::{CastOp, Opcode};
use llrm_mir::parse;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::{AssumptionCache, Query};

fn walk(
    q: &Query,
    v: Operand,
    seen: &mut BTreeSet<Operand>,
    why: &mut BTreeSet<String>,
    depth: u32,
) {
    if depth > 16 || !seen.insert(v) {
        return;
    }
    if let Some(ty) = q.type_of(v)
        && matches!(q.context().types.get(ty), Type::Vector { .. })
    {
        why.insert("vector".into());
    }
    let Some((inst, instruction)) = q.instruction_of(v) else { return };
    match &instruction.opcode {
        Opcode::Cast(CastOp::BitCast) => {
            why.insert("bitcast".into());
        }
        Opcode::ExtractValue(_) => {
            why.insert("vector".into());
        }
        Opcode::Load { .. } => {
            why.insert("load".into());
        }
        Opcode::Select => {
            why.insert("select".into());
        }
        Opcode::Call(_) => match q.callee_name(inst) {
            Some(name) if name.starts_with("llvm.") => {
                let base = name.trim_start_matches("llvm.");
                let base = base.split('.').next().unwrap_or(base);
                if !matches!(base, "fabs" | "copysign") {
                    why.insert(format!("intrinsic:{base}"));
                }
            }
            _ => {
                why.insert("call".into());
            }
        },
        _ => {}
    }
    for &operand in &instruction.operands {
        walk(q, operand, seen, why, depth + 1);
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("a readable file");
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
            let has_assume = function
                .walk()
                .any(
                    |(_, inst)| {
                        let q = Query::new(&module, function, &layout);
                        q.callee_name(inst).is_some_and(|n| n.starts_with("llvm.assume"))
                    },
                );
            let mut values: Vec<Operand> = function.parameters().iter().map(|p| Operand::Value(*p)).collect();
            for (_, inst) in function.walk() {
                if let Some(result) = function.instruction(inst).result {
                    values.push(Operand::Value(result));
                }
            }
            let mut n = 0;
            for value in values {
                let mut query = Query::new(&module, function, &layout);
                query.dt = Some(&dt);
                query.assumptions = Some(&assumptions);
                let Some(ty) = query.type_of(value) else { continue };
                let Type::Float(kind) = *query.context().types.get(ty) else { continue };
                if kind == llrm_mir::types::FloatKind::X86Fp80 {
                    continue;
                }
                let mut why = BTreeSet::new();
                walk(&query, value, &mut BTreeSet::new(), &mut why, 0);
                if has_assume {
                    why.insert("assume".into());
                }
                let why = if why.is_empty() { "-".to_owned() } else { why.into_iter().collect::<Vec<_>>().join(",") };
                println!("{name} {n} {why}");
                n += 1;
            }
        }
    }
}
