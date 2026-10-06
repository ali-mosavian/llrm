//! What the optimizer proved of a fixed-cell load, kept for selection: each write that leaves its bytes as
//! they were, as `!llrm.spares` on the load. LLVM's `MachineMemOperand` keeps the IR value for the same use,
//! alias queries at the machine level (`MachineInstr::mayAlias`). Last, so the instruction ids it names are the
//! ones selection sees.

use std::collections::BTreeMap;

use llrm_analysis::cfg::Shape;
use llrm_analysis::memoryssa::{self, Accesses};
use llrm_mir::module::{GlobalKind, InstId, MetadataId, MetadataNode, MetadataOperand, Module};
use llrm_mir::context::GlobalId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

/// The metadata kind: operands are the ids of the writes that leave the load's cell alone.
pub const KIND: &str = "llrm.spares";

pub struct Spares;

impl ModulePass for Spares {
    fn name(&self) -> &'static str {
        "spares"
    }

    fn run(&mut self, module: &mut Module, analyses: &mut ModuleAnalyses) -> Vec<GlobalId> {
        let layout = analyses.program().layout.clone();
        let outer = analyses.outer(module);
        let mut found: BTreeMap<GlobalId, Vec<(InstId, Vec<InstId>)>> = BTreeMap::new();
        for (id, _, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            let manager = analyses.manager(id, &outer);
            if manager.get::<Shape>(&module.context, &layout, function).loops.is_empty() {
                continue;
            }
            let Ok(accesses) = Accesses::managed(&module.context, &layout, function, manager) else { continue };
            let writers: Vec<InstId> = function.walk().map(|(_, inst)| inst).filter(|&inst| accesses.writes(inst).is_none_or(|writes| !writes.is_empty())).collect();
            let program = Some(analyses.program().as_ref());
            for (_, inst) in function.walk() {
                let instruction = function.instruction(inst);
                let (Opcode::Load { volatile: false, .. }, Some(read)) = (&instruction.opcode, accesses.references.get(&inst)) else { continue };
                // An object's own bytes at a constant offset: the same cell on every trip.
                if !read.object || read.base.is_some() {
                    continue;
                }
                let spared: Vec<InstId> = writers.iter().copied().filter(|&write| write != inst && memoryssa::spares(&accesses, program, read, write)).collect();
                if !spared.is_empty() {
                    found.entry(id).or_default().push((inst, spared));
                }
            }
        }
        let i32 = module.context.types.int(32);
        for (id, loads) in &found {
            for (inst, spared) in loads {
                let operands = spared.iter().map(|write| MetadataOperand::Constant(module.context.int(i32, i128::from(write.0)))).collect();
                module.metadata.push(MetadataNode { distinct: false, operands });
                let node = MetadataId(module.metadata.len() as u32 - 1);
                if let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind {
                    function.annotate(*inst, KIND, node);
                }
            }
        }
        found.into_keys().collect()
    }
}
