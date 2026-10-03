//! Which globals each call leaves as they were, stated on the call for the
//! machine side: `!llrm.spares = !{ptr @g, ...}`.
//!
//! A tracked global's address never leaves the program's bodies, so a call
//! writes it only where alias's effects name its object. The machine form has
//! no objects, only cells, so the fact is carried on the call, once, after the
//! last pass that moves a call or changes what one writes. An absent node
//! says nothing is spared.

use std::collections::BTreeSet;

use llrm_analysis::manager::{CallEffects, GlobalsAA};
use llrm_analysis::memory::{Identity, MemoryKind};
use llrm_mir::context::{Constant, ConstantKind, GlobalId};
use llrm_mir::module::{InstId, MetadataNode, MetadataOperand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_support::hash::IndexMap;

pub struct Spares;

impl FunctionPass for Spares {
    fn name(&self) -> &'static str {
        "spares"
    }

    fn run(&mut self, unit: &mut passes::Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let Some(Ok(globals)) = outer.cached_ref::<GlobalsAA>() else { return PreservedAnalyses::all() };
        let tracked = globals.tracked_globals().clone();
        let effects = analyses.get::<CallEffects>(unit.context, unit.layout, unit.function);
        let Ok(effects) = &*effects else { return PreservedAnalyses::all() };
        let mut stamps: Vec<(InstId, BTreeSet<GlobalId>)> = Vec::new();
        let pointers = analyses.get::<llrm_analysis::manager::Pointers>(unit.context, unit.layout, unit.function);
        if let Ok(pointers) = &*pointers {
            let view = llrm_analysis::memory::Unit::within(unit.context, unit.layout, unit.function, &outer);
            for (_, inst) in unit.function.walk() {
                if !matches!(unit.function.instruction(inst).opcode, Opcode::Store { .. }) || unit.function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "llrm.spares") {
                    continue;
                }
                if let Some(written) = llrm_analysis::alias::writes_only(&view, pointers, inst) {
                    let spared: BTreeSet<GlobalId> = tracked.difference(&written).copied().collect();
                    if !spared.is_empty() {
                        stamps.push((inst, spared));
                    }
                }
            }
        }
        for (&at, effect) in effects {
            if !matches!(unit.function.instruction(at).opcode, Opcode::Call(_)) || unit.function.instruction(at).metadata.iter().any(|(kind, _)| kind == "llrm.spares") {
                continue;
            }
            // A store whose bytes are no object's may be any, a tracked global's too.
            if effect.stores.iter().any(|one| one.provenance.is_none()) {
                continue;
            }
            let written: BTreeSet<GlobalId> = effect
                .stores
                .iter()
                .flat_map(|one| one.provenance.iter().flat_map(|provenance| provenance.slices.iter()))
                .filter(|slice| slice.object.kind == MemoryKind::Global)
                .filter_map(|slice| match slice.object.identity {
                    Some(Identity::Global(global)) => Some(GlobalId(global)),
                    _ => None,
                })
                .collect();
            let spared: BTreeSet<GlobalId> = tracked.difference(&written).copied().collect();
            if !spared.is_empty() {
                stamps.push((at, spared));
            }
        }
        if stamps.is_empty() {
            return PreservedAnalyses::all();
        }
        let mut nodes: IndexMap<BTreeSet<GlobalId>, llrm_mir::module::MetadataId> = IndexMap::default();
        for (at, spared) in stamps {
            let node = match nodes.get(&spared) {
                Some(&node) => node,
                None => {
                    let operands = spared
                        .iter()
                        .map(|&global| {
                            let ty = unit.context.types.ptr(outer.globals[global.0 as usize].address_space);
                            MetadataOperand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Global(global) }))
                        })
                        .collect();
                    let node = unit.declared.node(MetadataNode { distinct: false, operands });
                    nodes.insert(spared, node);
                    node
                }
            };
            unit.function.annotate(at, "llrm.spares", node);
        }
        PreservedAnalyses::all()
    }
}
