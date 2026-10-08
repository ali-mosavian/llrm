//! The runtime's cells a program names by EXTDEF, and what writes them:
//! llrm-core's `raising_defseg` and `spared`.
//!
//! `DEF SEG = x` is `push x / call B$DSEG`, whose whole body stores the
//! pushed word into `b$seg` and leaves it in AX: here, that store. Every
//! other routine's promise is stated in the runtime module as the HIR
//! emitter states it: a cell `runtime::named_only` names is `!llrm.named`,
//! and a routine that runs no program code writes only the named cells
//! `runtime::writers` lists. GlobalsAA reads both.

use llrm_support::hash::HashMap;

use iced_x86::Register;
use llrm_qbruntime::{self as runtime, Control};
use llrm_x86_bcmachine::model::ir::nodes::Node;
use llrm_x86_bcmachine::objectfile::omf;
use llrm_hir::model::RuntimePromises;
use llrm_mir::{ConstantId, ConstantKind, Module, Operand};

use crate::emit::{Emit, Emitter};
use crate::machine::Facts;
use crate::objects::Objects;
use crate::sites::Recognizer;

/// The segment PEEK, POKE and BLOAD address.
pub const SEGMENT_CELL: &str = "b$seg";
const DEF_SEG: &str = "B$DSEG";

/// The global of the runtime cell the module names `name`.
fn external(facts: &Facts, objects: &Objects, name: &str) -> Option<ConstantId> {
    let index = omf::externals(&facts.found.records).iter().position(|one| one == name)?;
    objects.external(index as i64)
}

/// `B$DSEG` as the store it is.
pub struct DefSeg;

impl Recognizer for DefSeg {
    fn node(&self, emitter: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let Node::Call(call) = node else { return None };
        if call.name != DEF_SEG || emitter.unit.procedures.contains_key(DEF_SEG) {
            return None;
        }
        let contract = emitter.unit.facts.contract(call.insn.at)?;
        let expected = runtime::contract(Some(DEF_SEG));
        let whole = contract.established
            && contract.cleanup == expected.cleanup
            && contract.clobbers == expected.clobbers
            && contract.control == Control::Returns
            && !contract.enters_user_code
            && !contract.raises_error
            && !contract.error_handling;
        let cell = external(emitter.unit.facts, emitter.unit.objects, SEGMENT_CELL).filter(|_| whole)?;
        Some(def_seg(emitter, cell))
    }
}

fn def_seg(emitter: &mut Emitter, cell: ConstantId) -> Emit<()> {
    let value = emitter.stack_word(emitter.depth(), 2)?;
    emitter.popped(2)?;
    emitter.b.store(value, Operand::Constant(cell), false);
    emitter.set_register(Register::AX, value)
}

/// The runtime module: which runtime cells are named and which routines
/// write them.
pub fn promise(module: &Module, facts: &Facts, objects: &Objects) -> Result<Module, String> {
    let family = facts.family();
    let family = family.value();
    let externals = omf::externals(&facts.found.records);
    let mut named = HashMap::default();
    for (index, name) in externals.iter().enumerate().skip(1) {
        if !runtime::named_only(name, family) {
            continue;
        }
        if let Some(reference) = objects.external(index as i64)
            && let ConstantKind::Global(global) = module.context.get(reference).kind
        {
            named.insert(name.as_str(), global);
        }
    }
    // Where the program handles errors, a routine that raises one runs it.
    let handles = runtime::handles_errors(facts.contracts.values());
    let declared = module.functions().filter(|(_, _, one)| one.is_declaration()).filter_map(|(_, global, _)| global.name.as_deref()?.strip_prefix(crate::RUNTIME));
    let (raising, nounwind): (Vec<&str>, Vec<&str>) = declared.partition(|&routine| runtime::contract(Some(routine)).raises_error);
    let calling_back = runtime::ENTERS_USER_CODE.iter().copied().chain(raising.into_iter().filter(|_| handles));
    llrm_hir::mir::promised(&[(module, named)], &RuntimePromises::of(calling_back, runtime::writers(family), nounwind))
}
