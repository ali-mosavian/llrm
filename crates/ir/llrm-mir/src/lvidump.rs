//! What LazyValueInfo answers for every value, edge, icmp and use of a module,
//! in the format of `tools/llvmport/lvi_dump.cpp`: the Rust side of the oracle
//! (`examples/lvidump.rs`, `tools/llvmport/lvicompare.py`) and of the tests
//! that hold LLVM's answers (`tests/lvi`).

use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::hash::HashMap;
use crate::lazyvalueinfo::{Env, LazyValueInfo, Subject};
use crate::module::{BlockId, GlobalKind, InstId, Operand, Use, ValueDef};
use crate::opcode::{IntPredicate, Opcode};
use crate::parse;
use crate::valuelattice::LatticeConstant;
use crate::valuetracking::{AssumptionCache, Query};
use crate::{ConstantKind, Context};

fn show(range: &crate::constantrange::ConstantRange) -> String {
    format!("i{}:{}", range.width, range.print())
}

fn tf(answer: Option<bool>) -> char {
    match answer {
        None => '-',
        Some(true) => 'T',
        Some(false) => 'F',
    }
}

fn cst(
    context: &Context,
    globals: &[crate::GlobalValue],
    c: Option<LatticeConstant>,
) -> String {
    match c {
        None => "-".to_owned(),
        Some(LatticeConstant::Int { width, bits }) => format!("i{width}:{bits}"),
        Some(c) if c.is_null_value() => "null".to_owned(),
        Some(LatticeConstant::Other { kind: ConstantKind::Global(global), .. }) => {
            let _ = context;
            format!("@{}", globals[global.0 as usize].name.owned().unwrap_or_default())
        }
        Some(_) => "?".to_owned(),
    }
}

/// The lines `lvi_dump` prints for the module in `text`.
pub fn dump(text: &str) -> Result<String, String> {
    let mut out = String::new();
    {
        let mut module = parse::module(text).map_err(|why| why.to_string())?;
        let layout =
            module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a datalayout")).unwrap_or_default();
        let count = module.globals.len();
        for at in 0..count {
            let GlobalKind::Function(function) = &module.globals[at].kind else { continue };
            if function.is_declaration() {
                continue;
            }
            let name = module.globals[at].name.owned().unwrap_or_default();
            let dt = DomTree::of(function);
            let assumptions = AssumptionCache::new(&module, function);
            let mut lvi = LazyValueInfo::new();
            let globals = &module.globals;
            let metadata = &module.metadata;
            let context = &mut module.context;
            let GlobalKind::Function(function) = &globals[at].kind else { unreachable!("a function") };

            let scalar = |context: &Context, op: Operand| -> bool {
                let q = Query::bare(context, function, &layout);
                (1..=128).contains(&q.scalar_bits(op))
            };
            let is_int = |context: &Context, op: Operand| -> bool {
                let ty = function.operand_type(context, op).expect("a value");
                matches!(context.types.get(ty), crate::Type::Int(bits) if *bits <= 128)
            };
            let positions = function.positions();
            let mut number: HashMap<Operand, u32> = HashMap::default();
            let mut index = crate::dense::IdMap::<InstId, u32>::new();
            let mut block = crate::dense::IdMap::<BlockId, u32>::new();
            let (mut n, mut u) = (0, 0);
            for &parameter in function.parameters() {
                if scalar(context, Operand::Value(parameter)) {
                    number.insert(Operand::Value(parameter), n);
                    n += 1;
                }
            }
            for (b, &bb) in function.layout().iter().enumerate() {
                block.insert(bb, b as u32);
                for inst in function.iter_block(bb) {
                    index.insert(inst, u);
                    u += 1;
                    if let Some(result) = function.instruction(inst).result
                        && scalar(context, Operand::Value(result))
                    {
                        number.insert(Operand::Value(result), n);
                        n += 1;
                    }
                }
            }
            let parent_of = |op: Operand| match op {
                Operand::Value(v) => match function.value(v).def {
                    ValueDef::Instruction(i) => function.parent(i),
                    ValueDef::Argument(_) => None,
                },
                _ => None,
            };
            let dominates_block = |op: Operand, at: BlockId| parent_of(op).is_none_or(|def| dt.dominates(def, at));
            let entry_first =
                function.block(function.entry().expect("an entry")).first().expect("a block with an instruction");

            let mut env = Env::new(context, globals, metadata, function, &layout, &assumptions);
            let mut dump_value = |env: &mut Env, op: Operand, cxt: InstId| -> Option<String> {
                let &number = number.get(&op)?;
                if let Some(def) = parent_of(op)
                    && !dt.is_reachable(def)
                {
                    return None;
                }
                let mut line = format!("val {name} {number}");
                if is_int(env.context, op) {
                    line += &format!(" cr={}", show(&lvi.get_constant_range(env, op, cxt, false)));
                } else {
                    line += " cr=-";
                }
                let c = lvi.get_constant(env, op, cxt);
                line += &format!(" cst={}", cst(env.context, globals, c));
                if Query::bare(env.context, function, &layout).is_pointer(op) {
                    let ty = function.operand_type(env.context, op).expect("a pointer");
                    let null = env.context.constant(crate::Constant { ty, kind: ConstantKind::Null });
                    line += &format!(" pn={}", tf(lvi.get_predicate_at(env, IntPredicate::Ne, op, null, cxt, true)));
                } else {
                    line += " pn=-";
                }
                Some(line)
            };
            for &parameter in function.parameters() {
                out.extend(dump_value(&mut env, Operand::Value(parameter), entry_first).map(|line| line + "\n"));
            }
            for &bb in function.layout() {
                if dt.is_reachable(bb) {
                    for inst in function.iter_block(bb) {
                        if let Some(result) = function.instruction(inst).result {
                            out.extend(dump_value(&mut env, Operand::Value(result), inst).map(|line| line + "\n"));
                        }
                    }
                }
            }

            // The same values, last first, in a fresh analysis.
            let mut fresh = LazyValueInfo::new();
            for &bb in function.layout().iter().rev() {
                if !dt.is_reachable(bb) {
                    continue;
                }
                for inst in function.iter_block_rev(bb) {
                    let Some(result) = function.instruction(inst).result else { continue };
                    let op = Operand::Value(result);
                    if !is_int(env.context, op) {
                        continue;
                    }
                    let range = fresh.get_constant_range(&mut env, op, inst, false);
                    out += &format!("rval {name} {} cr={}\n", number[&op], show(&range));
                }
            }

            // Edges.
            for &bb in function.layout() {
                if !dt.is_reachable(bb) {
                    continue;
                }
                for succ in function.successors(bb) {
                    let mut done: Vec<Operand> = Vec::new();
                    for inst in function.iter_block(succ) {
                        let instruction = function.instruction(inst);
                        for (k, &v) in instruction.operands.iter().enumerate() {
                            if !matches!(v, Operand::Value(_)) || !is_int(env.context, v) || !dominates_block(v, bb) {
                                continue;
                            }
                            if instruction.opcode == Opcode::Phi && instruction.operands[k + 1] != Operand::Block(bb) {
                                continue;
                            }
                            if done.contains(&v) {
                                continue;
                            }
                            done.push(v);
                            let range = lvi.get_constant_range_on_edge(&mut env, v, bb, succ, None);
                            out += &format!(
                                "edge {name} {} {} {} cr={}\n",
                                block[&bb],
                                block[&succ],
                                number[&v],
                                show(&range)
                            );
                        }
                    }
                }
            }

            // Predicates at each icmp.
            for &bb in function.layout() {
                if !dt.is_reachable(bb) {
                    continue;
                }
                for inst in function.iter_block(bb) {
                    let instruction = function.instruction(inst);
                    let Opcode::ICmp(pred) = instruction.opcode else { continue };
                    let (a, b) = (instruction.operands[0], instruction.operands[1]);
                    if !scalar(env.context, a) || !dominates_block(a, bb) || !dominates_block(b, bb) {
                        continue;
                    }
                    let with = lvi.get_predicate_at_values(&mut env, pred, a, b, inst, true);
                    let without = lvi.get_predicate_at_values(&mut env, pred, a, b, inst, false);
                    out += &format!("pred {name} {} a={} b={}\n", index[&inst], tf(with), tf(without));
                }
            }

            // Predicates on the edges of conditional branches.
            for &bb in function.layout() {
                if !dt.is_reachable(bb) {
                    continue;
                }
                let Some(term) = function.terminator(bb) else { continue };
                let branch = function.instruction(term);
                if branch.opcode != Opcode::Br || branch.operands.len() != 3 {
                    continue;
                }
                let Some((cmp_inst, cmp)) = (match branch.operands[0] {
                    Operand::Value(v) => match function.value(v).def {
                        ValueDef::Instruction(i) => Some((i, function.instruction(i))),
                        _ => None,
                    },
                    _ => None,
                }) else {
                    continue;
                };
                let Opcode::ICmp(mut pred) = cmp.opcode else { continue };
                if !scalar(env.context, cmp.operands[0]) {
                    continue;
                }
                let (v, c) = if let Operand::Constant(c) = cmp.operands[1] {
                    (cmp.operands[0], c)
                } else if let Operand::Constant(c) = cmp.operands[0] {
                    pred = pred.swapped();
                    (cmp.operands[1], c)
                } else {
                    continue;
                };
                if matches!(v, Operand::Constant(_)) || !dominates_block(v, bb) {
                    continue;
                }
                for s in 0..2 {
                    let Operand::Block(succ) = branch.operands[1 + s] else { unreachable!("a block") };
                    let answer = lvi.get_predicate_on_edge(&mut env, pred, v, c, bb, succ, None);
                    out += &format!("epred {name} {} {s} {} {}\n", block[&bb], index[&cmp_inst], tf(answer));
                }
            }

            // Uses.
            for &bb in function.layout() {
                if !dt.is_reachable(bb) {
                    continue;
                }
                for inst in function.iter_block(bb) {
                    let instruction = function.instruction(inst);
                    for (k, &v) in instruction.operands.iter().enumerate() {
                        if !matches!(v, Operand::Value(_)) || !is_int(env.context, v) {
                            continue;
                        }
                        if let Some(def_block) = parent_of(v) {
                            let Operand::Value(value) = v else { unreachable!("a value") };
                            let ValueDef::Instruction(def) = function.value(value).def else { unreachable!() };
                            let bad = if def_block == bb {
                                instruction.opcode == Opcode::Phi
                                    || positions[def.0 as usize] >= positions[inst.0 as usize]
                            } else {
                                !dt.dominates(def_block, bb)
                            };
                            if bad {
                                continue;
                            }
                        }
                        let range = lvi.get_constant_range_at_use(&mut env, Use { user: inst, index: k as u32 }, false);
                        // LLVM numbers a phi's operands by incoming value.
                        let slot = if instruction.opcode == Opcode::Phi { k / 2 } else { k };
                        out += &format!("use {name} {} {slot} cr={}\n", index[&inst], show(&range));
                    }
                }
            }

            // printLVI, where every block, argument and result has a name.
            let named = function.parameters().iter().all(|&p| function.value(p).name.is_some())
                && function.layout().iter().all(|&bb| {
                    function.block(bb).name.is_some()
                        && function
                            .iter_block(bb)
                            .all(|i| function.instruction(i).result.is_none_or(|r| function.value(r).name.is_some()))
                });
            if named {
                let annotations = match lvi.get_impl() {
                    Some(imp) => imp.annotations(&mut env, &dt),
                    None => Vec::new(),
                };
                let mut lines: Vec<String> = annotations
                    .iter()
                    .map(|a| match a.subject {
                        Subject::Argument(arg) => {
                            let k = function.parameters().iter().position(|&p| p == arg).expect("a parameter");
                            format!("lat {name} arg{k} bb{} {}", block[&a.block], a.lattice)
                        }
                        Subject::Instruction(i) => {
                            format!("lat {name} i{} bb{} {}", index[&i], block[&a.block], a.lattice)
                        }
                    })
                    .collect();
                lines.sort();
                for line in lines {
                    out += &line;
                    out += "\n";
                }
            }
        }
    }
    Ok(out)
}
