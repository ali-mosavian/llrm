//! A function a frontend allows near (`nearcode`), only its own module calls,
//! and only directly, is entered by a near call: its callers share its code
//! segment, so a return address needs no segment. MIR states linkage and calls;
//! which instruction enters the code is chosen here, as a target fact.

use std::borrow::Cow;

use llrm_mir::callgraph::{NEAR_CODE, direct_only};
use llrm_mir::module::Module;
use llrm_mir::opcode::Attribute;

/// `module` with its internal, directly-called functions in near code.
pub fn placed(module: &Module) -> Cow<'_, Module> {
    let near: Vec<_> = direct_only(module)
        .into_iter()
        .filter(|&id| module.global(id).address_space != 0 && stated(module, id))
        .collect();
    if near.is_empty() {
        return Cow::Borrowed(module);
    }
    let mut module = module.clone();
    for id in near {
        module.globals[id.0 as usize].address_space = 0;
    }
    Cow::Owned(module)
}

fn stated(
    module: &Module,
    id: llrm_mir::context::GlobalId,
) -> bool {
    module
        .global(id)
        .function()
        .is_some_and(
            |function| function.attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == NEAR_CODE)),
        )
}
