//! Prints what the context-derived ValueTracking functions answer on a module,
//! in the format of `tools/llvmport/ctxdump.cpp`.
//! usage: ctxdump FILE.ll...

use llrm_mir::context::ConstantKind;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::knownbits::KnownBits;
use llrm_mir::module::{GlobalKind, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::parse;
use llrm_mir::valuetracking::context::{
    compute_known_bits_from_context, find_values_affected_by_condition, is_implied_by_dom_condition_cmp,
    is_implied_by_dom_condition_cond, is_implied_condition, is_valid_assume_for_context, will_not_free_between,
};
use llrm_mir::valuetracking::known::compute_known_bits;
use llrm_mir::valuetracking::pattern::CmpPredicate;
use llrm_mir::valuetracking::{AssumptionCache, CondContext, DomConditionCache, Query};

fn tri(r: Option<bool>) -> &'static str {
    match r {
        Some(true) => "T",
        Some(false) => "F",
        None => "-",
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
            dump(&module, function, global.name.as_deref().unwrap_or(""), &layout);
        }
    }
}

fn dump(
    module: &Module,
    function: &llrm_mir::module::Function,
    name: &str,
    layout: &DataLayout,
) {
    let dt = DomTree::of(function);
    let ac = AssumptionCache::new(module, function);
    let dc = DomConditionCache::new(module, function);
    let insts: Vec<InstId> = function.walk().map(|(_, inst)| inst).collect();
    let base = Query::new(module, function, layout);
    let nm = |v: Operand| -> String {
        match v {
            Operand::Value(id) => match function.value(id).def {
                ValueDef::Argument(k) => format!("a{k}"),
                ValueDef::Instruction(inst) => {
                    insts.iter().position(|one| *one == inst).map_or("?".into(), |k| format!("i{k}"))
                }
            },
            Operand::Constant(id) => match module.context.get(id).kind {
                ConstantKind::Global(g) => format!("@{}", module.globals[g.0 as usize].name.as_deref().unwrap_or("")),
                _ => "?".into(),
            },
            Operand::Block(_) => "?".into(),
        }
    };
    let inm = |inst: InstId| format!("i{}", insts.iter().position(|one| *one == inst).expect("placed"));
    let result = |inst: InstId| function.instruction(inst).result.map(Operand::Value);
    let mut bools: Vec<Operand> = function.parameters().iter().map(|p| Operand::Value(*p)).collect();
    bools.retain(|v| base.scalar_bits(*v) == 1 && !base.is_pointer(*v));
    bools.extend(insts.iter().filter_map(|i| result(*i)).filter(|v| base.scalar_bits(*v) == 1 && !base.is_pointer(*v)));
    let assumes: Vec<InstId> = insts.iter().copied().filter(|i| ac.assumes.contains(i)).collect();
    let query = |cxt: Option<InstId>| {
        let mut q = base.with_cxt(cxt);
        q.dt = Some(&dt);
        q.assumptions = Some(&ac);
        q.dom_conditions = Some(&dc);
        q
    };

    for &i in &insts {
        let q = query(Some(i));
        let mut seen: Vec<Operand> = Vec::new();
        for &op in &function.instruction(i).operands {
            let Operand::Value(_) = op else { continue };
            let w = q.scalar_bits(op);
            if w == 0 || w > 128 || seen.contains(&op) {
                continue;
            }
            seen.push(op);
            let mut known = KnownBits::unknown(w);
            compute_known_bits_from_context(&q, op, &mut known, 0);
            println!("{name} ctx {} {} kb={}", inm(i), nm(op), known.print());
        }
    }
    let q = query(None);
    if bools.len() <= 40 {
        for &a in &bools {
            for &b in &bools {
                for t in 0..2 {
                    println!("{name} imp {} {} {t} {}", nm(a), nm(b), tri(is_implied_condition(&q, a, b, t == 1, 0)));
                }
            }
        }
    }
    for &i in &insts {
        let q = query(Some(i));
        for &c in &bools {
            println!("{name} dom {} {} {}", inm(i), nm(c), tri(is_implied_by_dom_condition_cond(&q, c, i)));
        }
        for &j in &insts {
            let instruction = function.instruction(j);
            if let Opcode::ICmp(pred) = instruction.opcode {
                let same_sign = instruction.flags.contains(llrm_mir::opcode::Flags::SAMESIGN);
                let r = is_implied_by_dom_condition_cmp(
                    &q,
                    CmpPredicate { pred, same_sign },
                    instruction.operands[0],
                    instruction.operands[1],
                    i,
                );
                println!("{name} domp {} {} {}", inm(i), inm(j), tri(r));
            }
        }
    }
    for &c in &bools {
        for assume in 0..2 {
            let mut list = String::new();
            find_values_affected_by_condition(&q, c, assume == 1, &mut |v| list += &format!(" {}", nm(v)));
            println!("{name} aff {} {assume}{list}", nm(c));
        }
    }
    for &u in &insts {
        let instruction = function.instruction(u);
        let c = match instruction.opcode {
            Opcode::Select => instruction.operands[0],
            Opcode::Br if instruction.operands.len() == 3 => instruction.operands[0],
            _ => continue,
        };
        if base.scalar_bits(c) != 1 || base.is_pointer(c) {
            continue;
        }
        let mut cc = CondContext::new(c);
        find_values_affected_by_condition(&q, c, false, &mut |v| {
            cc.affected_values.insert(v);
        });
        for inv in 0..2 {
            cc.invert = inv == 1;
            let q = query(Some(u)).with_cond_context(&cc);
            let mut vals: Vec<Operand> = cc.affected_values.iter().copied().collect();
            if instruction.opcode == Opcode::Select {
                vals.push(instruction.operands[1]);
                vals.push(instruction.operands[2]);
            }
            let mut sorted: Vec<(String, Operand)> = Vec::new();
            for v in vals {
                let w = q.scalar_bits(v);
                if w == 0 || w > 128 || !matches!(v, Operand::Value(_)) {
                    continue;
                }
                if !sorted.iter().any(|(n, _)| *n == nm(v)) {
                    sorted.push((nm(v), v));
                }
            }
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (n, v) in sorted {
                println!("{name} cc {} {} {inv} {n} kb={}", nm(c), inm(u), compute_known_bits(&q, v, 0).print());
            }
        }
    }
    let acache = |v: Operand| {
        let list: String = ac.assumptions_for(v).iter().map(|a| format!(" {}", inm(*a))).collect();
        if !list.is_empty() {
            println!("{name} acache {}{list}", nm(v));
        }
    };
    for p in function.parameters() {
        acache(Operand::Value(*p));
    }
    for &i in &insts {
        if let Some(v) = result(i) {
            acache(v);
        }
    }
    for &a in &assumes {
        for &i in &insts {
            println!("{name} free {} {} {}", inm(a), inm(i), u8::from(will_not_free_between(&base, a, i)));
            let with_dt = query(None);
            println!(
                "{name} valid {} {} 1 {}",
                inm(a),
                inm(i),
                u8::from(is_valid_assume_for_context(&with_dt, a, i, false))
            );
            println!(
                "{name} valid {} {} 0 {}",
                inm(a),
                inm(i),
                u8::from(is_valid_assume_for_context(&base, a, i, false))
            );
        }
    }
}
