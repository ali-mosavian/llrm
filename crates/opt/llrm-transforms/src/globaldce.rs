//! LLVM's GlobalDCE: a module's own body or variable that nothing live
//! names -- no code outside the program, no live body, no live initializer
//! -- is deleted. A declaration stays: naming one asks the linker for it.

use std::collections::BTreeSet;

use llrm_analysis::globalsaa::embedded;
use llrm_mir::callgraph::Defined;
use llrm_mir::context::GlobalId;
use llrm_mir::module::{GlobalKind, GlobalValue, Linkage, Operand};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

pub struct GlobalDce;

impl ProgramPass for GlobalDce {
    fn name(&self) -> &'static str {
        "globaldce"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let live = live(program);
        let mut changed = false;
        for (at, module) in program.modules.iter_mut().enumerate() {
            let dead = |id: GlobalId| defines(module.global(id)) && !live.contains(&(at, id));
            if (0..module.globals.len() as u32).map(GlobalId).any(dead) {
                let keep: Vec<bool> = (0..module.globals.len() as u32).map(|id| !dead(GlobalId(id))).collect();
                module.retain_globals(&|id| keep[id.0 as usize]);
                changed = true;
            }
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

/// Whether `global` is a definition this pass may delete: a body or an
/// initialized variable no other module can name.
fn defines(global: &GlobalValue) -> bool {
    let defined = match &global.kind {
        GlobalKind::Function(function) => !function.is_declaration(),
        GlobalKind::Variable(variable) => variable.initializer.is_some(),
    };
    defined && matches!(global.linkage, Linkage::Internal | Linkage::Private)
}

/// The definitions outside code may reach, those the program keeps, and
/// those a live one names, transitively.
pub fn live(program: &Program) -> BTreeSet<Defined> {
    let mut pending = Vec::new();
    for (at, module) in program.modules.iter().enumerate() {
        for (index, global) in module.globals.iter().enumerate() {
            let kept = global.name.as_ref().is_some_and(|name| program.exports.kept.contains(name));
            if !defines(global) || program.exports.exported(global) || kept {
                pending.push((at, GlobalId(index as u32)));
            }
        }
    }
    let mut live = BTreeSet::new();
    while let Some((at, id)) = pending.pop() {
        if !live.insert((at, id)) {
            continue;
        }
        let module = &program.modules[at];
        let mut named = BTreeSet::new();
        match &module.global(id).kind {
            GlobalKind::Variable(variable) => variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut named)),
            GlobalKind::Function(function) => {
                function.personality.iter().for_each(|&one| embedded(&module.context, one, &mut named));
                for (_, inst) in function.walk() {
                    for operand in &function.instruction(inst).operands {
                        if let Operand::Constant(one) = operand {
                            embedded(&module.context, *one, &mut named);
                        }
                    }
                }
            }
        }
        for one in named {
            pending.push((at, one));
            pending.extend(program.definition(at, one));
        }
    }
    live
}
