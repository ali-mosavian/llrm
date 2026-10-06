//! A phi whose value is a cell's: what the optimizer proved of a loop-carried value that the program also stores,
//! kept for selection as `!llrm.home` on the phi, naming one store of the cell. Where such a value is spilled the
//! backend reads the cell: no store of its own, and a restore that is the load promotion removed. GCC's
//! `REG_EQUIV` by store, which LLVM has no counterpart of. Last, as `spares`, so the ids it names are the ones
//! selection sees.
//!
//! A phi `p` is in cell `c` where each of its inputs `v` was stored to `c` and nothing has written `c` since, at
//! the end of the block `v` comes from, and nothing writes `c` while `p` is live: `p` is then what `c` holds at
//! every point it is read. A store of `p` itself writes what is there.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg;
use llrm_analysis::liveness;
use llrm_analysis::memory::MemRef;
use llrm_analysis::memoryssa::Accesses;
use llrm_mir::context::GlobalId;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, MetadataId, MetadataNode, MetadataOperand, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

/// The metadata kind: the operand is the id of a store of the phi's cell.
pub const KIND: &str = "llrm.home";

pub struct Homes;

impl ModulePass for Homes {
    fn name(&self) -> &'static str {
        "homes"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let layout = analyses.program().layout.clone();
        let outer = analyses.outer(module);
        let mut found: BTreeMap<GlobalId, Vec<(InstId, InstId)>> = BTreeMap::new();
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            let manager = analyses.manager(id, &outer);
            let Ok(accesses) = Accesses::managed(&module.context, &layout, function, manager) else { continue };
            let program = Some(analyses.program().as_ref());
            for (phi, store) in homed(function, &accesses, program) {
                found.entry(id).or_default().push((phi, store));
            }
        }
        let i32 = module.context.types.int(32);
        for (id, homes) in &found {
            for &(phi, store) in homes {
                let operand = MetadataOperand::Constant(module.context.int(i32, i128::from(store.0)));
                module.metadata.push(MetadataNode { distinct: false, operands: vec![operand] });
                let node = MetadataId(module.metadata.len() as u32 - 1);
                if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                    function.annotate(phi, KIND, node);
                }
            }
        }
        found.into_keys().collect()
    }
}

/// An object's own bytes at a constant offset: the cell as one reference, whatever pointer named it.
fn exact(reference: &MemRef) -> bool {
    reference.object && reference.base.is_none() && reference.root.is_some() && !reference.volatile
}

/// Whether two references name the same bytes.
fn same(one: &MemRef, other: &MemRef) -> bool {
    one.root == other.root && one.disp == other.disp && one.width == other.width && one.space == other.space && one.selector == other.selector
}

/// Each phi of `function` that is a cell's, with a store of the cell.
pub fn homed(function: &Function, accesses: &Accesses, program: Option<&llrm_mir::program::ProgramProxy>) -> Vec<(InstId, InstId)> {
    // The stores of an exact cell, by the value stored.
    let mut all: Vec<(Operand, InstId, &MemRef)> = Vec::new();
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        if let (Opcode::Store { volatile: false, .. }, Some(reference)) = (&instruction.opcode, accesses.references.get(&inst))
            && exact(reference)
        {
            all.push((instruction.operands[0], inst, reference));
        }
    }
    if all.is_empty() {
        return Vec::new();
    }
    let stores = |value: &Operand| -> Vec<(InstId, &MemRef)> { all.iter().filter(|(stored, ..)| stored == value).map(|&(_, inst, reference)| (inst, reference)).collect() };
    let graph = cfg::graph(function);
    let dominance = cfg::Dominance::of(function);
    let found = liveness::live(function);
    let overlaps = |cell: &MemRef, inst: InstId| accesses.writes(inst).is_none_or(|writes| writes.iter().any(|wrote| llrm_analysis::regions::overlapping(cell, wrote, None, None, program).unwrap_or(true)));
    let mut homes = Vec::new();
    for &block in function.layout() {
        for &inst in function.block(block).instructions() {
            let phi = function.instruction(inst);
            let (Opcode::Phi, Some(result)) = (&phi.opcode, phi.result) else { continue };
            let inputs: Vec<(Operand, BlockId)> = phi.operands.chunks(2).filter_map(|pair| if let Operand::Block(from) = pair[1] { Some((pair[0], from)) } else { None }).collect();
            // The cells every input is stored to: the stores of an input that is a value, not a constant, name the
            // candidates, as a constant's may be built from narrower stores.
            let named_by = inputs.iter().find(|(value, _)| matches!(value, Operand::Value(one) if *one != result)).or_else(|| inputs.iter().find(|(value, _)| *value != Operand::Value(result)));
            let Some(first) = named_by.map(|(value, _)| stores(value)) else { continue };
            'cells: for &(named, cell) in &first {
                let mut place = None;
                for (value, from) in &inputs {
                    if *value == Operand::Value(result) {
                        continue;
                    }
                    let each = stores(value);
                    // A store of this cell, whose value no write has changed by the end of `from`.
                    let held = each.iter().filter(|(_, other)| same(cell, other)).any(|&(store, _)| survives(function, &graph, &dominance, &overlaps, cell, store, *from));
                    if !held {
                        continue 'cells;
                    }
                    place = place.or(Some(named));
                }
                if place.is_none() {
                    continue;
                }
                // Nothing writes the cell while the phi is live, but a store of the phi's own value.
                let disturbed = function.layout().iter().any(|&other| {
                    liveness::live_points(function, &found, other).into_iter().any(|(at, _, across)| {
                        across.contains(&result) && overlaps(cell, at) && !writes_only(function, at, Operand::Value(result))
                    })
                });
                if !disturbed {
                    homes.push((inst, named));
                    break;
                }
            }
        }
    }
    homes
}

/// Whether `inst` is a store of `value`: it leaves in the cell what the cell holds.
fn writes_only(function: &Function, inst: InstId, value: Operand) -> bool {
    let instruction = function.instruction(inst);
    matches!(instruction.opcode, Opcode::Store { .. }) && instruction.operands.first() == Some(&value)
}

/// Whether no instruction that may write `cell` runs between `store` and the end of `at`, on any path.
fn survives(function: &Function, graph: &[cfg::Block], dominance: &cfg::Dominance, overlaps: &dyn Fn(&MemRef, InstId) -> bool, cell: &MemRef, store: InstId, at: BlockId) -> bool {
    let home = function.parent(store).expect("a placed store");
    if !dominance.dominates(cfg::id(home), cfg::id(at)) {
        return false;
    }
    let after = |block: BlockId, from: Option<InstId>| {
        let insts = function.block(block).instructions();
        let start = from.and_then(|one| insts.iter().position(|&other| other == one)).map_or(0, |index| index + 1);
        insts[start..].iter().all(|&one| !overlaps(cell, one))
    };
    // What follows the store in its block, then every block on a path from it to `at` that does not return to it.
    if !after(home, Some(store)) && home != at {
        return false;
    }
    if home == at {
        return after(home, Some(store));
    }
    let by_block: BTreeMap<i64, &cfg::Block> = graph.iter().map(|block| (block.at, block)).collect();
    // Blocks reachable from the store's block without passing through it again.
    let mut reached = BTreeSet::new();
    let mut work: Vec<i64> = by_block[&cfg::id(home)].succ.clone();
    while let Some(next) = work.pop() {
        if next == cfg::id(home) || !reached.insert(next) {
            continue;
        }
        work.extend(by_block[&next].succ.iter().copied());
    }
    // Of those, the ones `at` can be reached from, again without the store's block.
    let mut reaches: BTreeSet<i64> = BTreeSet::from([cfg::id(at)]);
    loop {
        let joining: Vec<i64> = graph.iter().filter(|block| reached.contains(&block.at) && !reaches.contains(&block.at) && block.succ.iter().any(|next| reaches.contains(next))).map(|block| block.at).collect();
        if joining.is_empty() {
            break;
        }
        reaches.extend(joining);
    }
    reached.intersection(&reaches).all(|&block| after(cfg::block(block), None))
}

#[cfg(test)]
#[path = "homes_tests.rs"]
mod tests;
