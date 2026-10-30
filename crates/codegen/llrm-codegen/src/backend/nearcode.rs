//! Puts a function a frontend allows near, which only its own module calls and
//! only directly, in the near code space, so callers enter it by a near call.
//!
//! # How it works
//! MIR states linkage and calls; which instruction enters the code is a target
//! fact, so the move happens here. `placed` takes the functions
//! `llrm_mir::callgraph::direct_only` returns (defined, internal or private,
//! not an interrupt handler, never named but as a callee), keeps those with
//! the `nearcode` attribute that are not already in the target's near space,
//! and sets their address space to it. A module with none is returned as it
//! is, borrowed.
//!
//! ```text
//!   internal @f  (far space, nearcode, only called directly)
//!       call @f  was far call, return address with a segment
//!       call @f  now near call, shared code segment, no segment
//! ```
//!
//! A far caller needs no segment in the return address because every caller is
//! in the same module, and so in the same code segment.
//!
//! # Example
//! No honest example: the pass changes the address space of MIR functions and
//! touches no LIR. Read `test_*` in this file for the MIR it accepts.
//!
//! # Runs
//! Once per module at the start of `assemble::assembled_by` (`mir near code`),
//! before global names are made and before any function is selected. Whether
//! the attribute is set is the frontend's decision.

use std::borrow::Cow;

use llrm_mir::callgraph::{NEAR_CODE, direct_only};
use llrm_mir::module::Module;
use llrm_mir::opcode::Attribute;
use llrm_mir::spaces::Spaces;

/// `module` with its internal, directly-called functions in near code.
pub fn placed<'a>(
    module: &'a Module,
    spaces: &Spaces,
) -> Cow<'a, Module> {
    let near: Vec<_> = direct_only(module)
        .into_iter()
        .filter(|&id| module.global(id).address_space != spaces.near && stated(module, id))
        .collect();
    if near.is_empty() {
        return Cow::Borrowed(module);
    }
    let mut module = module.clone();
    for id in near {
        module.globals[id.0 as usize].address_space = spaces.near;
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

#[cfg(test)]
mod tests {
    use llrm_mir::GlobalKind;
    use llrm_mir::opcode::Attribute;
    use llrm_mir::spaces::Spaces;

    use super::*;

    /// An internal function in a far space, allowed near.
    fn far_function(space: u32) -> Module {
        let mut module = llrm_mir::parse::module("define internal void @f() {\n  ret void\n}\n").expect("parses");
        let global = &mut module.globals[0];
        global.address_space = space;
        if let GlobalKind::Function(function) = &mut global.kind {
            function.attrs.push(Attribute::Flag(NEAR_CODE.to_owned()));
        }
        module
    }

    /// Near code is the target's near space, not space 0: with the near space
    /// numbered 5, a function in 1 moves to 5 and one already in 5 stays put.
    #[test]
    fn near_code_is_placed_in_the_targets_near_space() {
        let spaces = Spaces { near: 5, ..Spaces::FLAT };
        assert_eq!(placed(&far_function(1), &spaces).globals[0].address_space, 5);
        assert!(matches!(placed(&far_function(5), &spaces), Cow::Borrowed(_)));
    }
}
