//! Values nothing with an effect needs removed, as LLVM's ADCE removes
//! them: what has an effect is live, and so is every value a live
//! instruction reads. The rest goes, cycles of phis a loop carries for no
//! one among them.

use std::collections::HashSet;

use crate::context::{Constant, ConstantKind};
use crate::memory;
use crate::module::{InstId, Operand, ValueDef};
use crate::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

pub struct Adce;

impl FunctionPass for Adce {
    fn name(&self) -> &'static str {
        "adce"
    }

    fn run(&mut self, unit: &mut Unit, _: &mut Analyses) -> PreservedAnalyses {
        let function = &*unit.function;
        let mut work: Vec<InstId> = function.walk().map(|(_, inst)| inst).filter(|&inst| !memory::only_value(unit.context, unit.callees, function, inst)).collect();
        let mut live: HashSet<InstId> = work.iter().copied().collect();
        while let Some(inst) = work.pop() {
            for operand in &function.instruction(inst).operands {
                if let Operand::Value(value) = *operand
                    && let ValueDef::Instruction(def) = function.value(value).def
                    && live.insert(def)
                {
                    work.push(def);
                }
            }
        }
        let dead: Vec<InstId> = function.walk().map(|(_, inst)| inst).filter(|inst| !live.contains(inst)).collect();
        if dead.is_empty() {
            return PreservedAnalyses::all();
        }
        // Only the dead read the dead: poison stands in while they go.
        for &inst in &dead {
            let instruction = unit.function.instruction(inst);
            if let Some(result) = instruction.result {
                let poison = unit.context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Poison });
                unit.function.replace_all_uses_with(result, Operand::Constant(poison));
            }
        }
        for inst in dead {
            unit.function.erase(inst).expect("nothing uses it");
        }
        // Blocks and edges are as they were.
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
    }
}
