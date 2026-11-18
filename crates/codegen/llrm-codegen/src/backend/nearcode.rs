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
use std::collections::BTreeSet;

use llrm_mir::callgraph::{NEAR_CODE, direct_only};
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_mir::opcode::Attribute;
use llrm_mir::spaces::Spaces;

/// `module` with its internal, directly-called functions in near code. With
/// `bytes` (code size outranks clocks) a function stays far where the jumps it
/// would lose cost more than its callers save (NEAR-TAIL).
pub fn placed<'a>(
    module: &'a Module,
    spaces: &Spaces,
    bytes: bool,
) -> Cow<'a, Module> {
    let mut near: BTreeSet<GlobalId> = direct_only(module)
        .into_iter()
        .filter(|&id| module.global(id).address_space != spaces.near && stated(module, id))
        .collect();
    if bytes {
        let layout = module.datalayout.as_deref().and_then(|text| DataLayout::parse(text).ok()).unwrap_or_default();
        // A function left far keeps its callers' far calls, which can take a
        // sibling jump out of another one: until nothing more moves.
        while let Some(&id) =
            near.iter().find(|&&id| lost(module, &layout, spaces, &near, id) > saved(module, &layout, spaces, id))
        {
            near.remove(&id);
        }
    }
    if near.is_empty() {
        return Cow::Borrowed(module);
    }
    let mut module = module.clone();
    for id in near {
        module.globals[id.0 as usize].address_space = spaces.near;
    }
    Cow::Owned(module)
}

/// What the near calls of `id` save: the far pointer's extra bytes in each
/// `call`.
fn saved(
    module: &Module,
    layout: &DataLayout,
    spaces: &Spaces,
    id: GlobalId,
) -> i64 {
    let extra = (i64::from(layout.pointer(spaces.far).bits) - i64::from(layout.pointer(spaces.near).bits)) / 8;
    let sites = module
        .functions()
        .flat_map(|(_, _, function)| function.walk().map(move |(_, inst)| (function, inst)))
        .filter(|(function, inst)| call_of(module, function, *inst) == Some(id))
        .count() as i64;
    sites * extra
}

/// What `id` loses as a near function: each call in tail position that passes
/// its own parameters to a callee that stays far. That call was a jump reusing
/// them; a near function cannot jump to a far callee, which returns a far
/// address, so the call is made, its arguments pushed again and cleaned up
/// (`twice` in far_pointer_call, m16 -Os: 5 B in the function against 2 saved
/// at its one call). Bytes of the arguments and a word of cleanup stand for it.
fn lost(
    module: &Module,
    layout: &DataLayout,
    spaces: &Spaces,
    near: &BTreeSet<GlobalId>,
    id: GlobalId,
) -> i64 {
    use llrm_mir::opcode::Opcode;
    let Some(function) = module.global(id).function() else { return 0 };
    let mut total = 0;
    for &block in function.layout() {
        let insts: Vec<_> = function.iter_block(block).collect();
        let [.., call, ret] = insts[..] else { continue };
        let (called, returned) = (function.instruction(call), function.instruction(ret));
        if !matches!(called.opcode, Opcode::Call(_)) || !matches!(returned.opcode, Opcode::Ret) {
            continue;
        }
        if returned.operands.first() != called.result.map(llrm_mir::module::Operand::Value).as_ref()
            || !llrm_mir::callgraph::passes_own_parameters(function, call)
        {
            continue;
        }
        let Some(callee) = call_of(module, function, call) else { continue };
        // A callee that is near, now or after this pass, is a near jump.
        if callee == id || near.contains(&callee) || module.global(callee).address_space == spaces.near {
            continue;
        }
        let given: i64 = called
            .operands
            .split_last()
            .map_or(&[][..], |(_, arguments)| arguments)
            .iter()
            .filter_map(|&one| function.operand_type(&module.context, one))
            .map(|ty| layout.alloc_size(&module.context.types, ty) as i64)
            .sum();
        total += given + 2;
    }
    total
}

/// The function `inst` of `function` calls by name, if it does.
fn call_of(
    module: &Module,
    function: &llrm_mir::module::Function,
    inst: llrm_mir::module::InstId,
) -> Option<GlobalId> {
    use llrm_mir::context::ConstantKind;
    let operation = function.instruction(inst);
    if !matches!(operation.opcode, llrm_mir::opcode::Opcode::Call(_)) {
        return None;
    }
    let llrm_mir::module::Operand::Constant(id) = operation.operands.last()? else { return None };
    match module.context.get(*id).kind {
        ConstantKind::Global(callee) => Some(callee),
        _ => None,
    }
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
            function.attrs.push(Attribute::Flag(NEAR_CODE.into()));
        }
        module
    }

    /// `twice` ends in a call to a far function with the argument it was
    /// given, a jump while `twice` is far; near it is a call with the argument
    /// pushed again. At size it stays far (5 bytes lost, 2 saved at its one
    /// call); at speed it is near. A function that ends in no call goes near
    /// at size too, as does one whose tail callee is near.
    #[test]
    fn a_function_that_ends_in_a_jump_to_a_far_callee_stays_far_at_size() {
        let text = |body: &str| {
            format!(
                "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16\"\n\
                 @g = global i32 0\ndeclare void @report(i32)\n\
                 define internal void @twice(i32 %v) {{\nb0:\n{body}  ret void\n}}\n\
                 define void @f() {{\nb0:\n  call void @twice(i32 3)\n  ret void\n}}\n"
            )
        };
        let far = |text: String| {
            let mut module = llrm_mir::parse::module(&text).expect("parses");
            for global in &mut module.globals {
                if let GlobalKind::Function(function) = &mut global.kind {
                    global.address_space = 1;
                    function.attrs.push(Attribute::Flag(NEAR_CODE.into()));
                }
            }
            module
        };
        let spaces = Spaces { near: 0, far: 1, ..Spaces::FLAT };
        let moved = |module: &Module, bytes: bool| {
            let placed = placed(module, &spaces, bytes);
            let at = placed.globals.iter().position(|one| one.name.as_deref() == Some("twice")).unwrap();
            placed.globals[at].address_space == spaces.near
        };
        let ends_in_call = far(text("  call void @report(i32 %v)\n  call void @report(i32 %v)\n"));
        assert!(!moved(&ends_in_call, true), "kept far at size");
        assert!(moved(&ends_in_call, false), "near at speed");
        let plain = far(text("  call void @report(i32 %v)\n  store i32 1, ptr @g\n"));
        assert!(moved(&plain, true), "no jump to lose");
        // A call with a new argument was a call before too.
        let fresh = far(text("  call void @report(i32 %v)\n  call void @report(i32 7)\n"));
        assert!(moved(&fresh, true), "the last call was never a jump");
    }

    /// Near code is the target's near space, not space 0: with the near space
    /// numbered 5, a function in 1 moves to 5 and one already in 5 stays put.
    #[test]
    fn near_code_is_placed_in_the_targets_near_space() {
        let spaces = Spaces { near: 5, ..Spaces::FLAT };
        assert_eq!(placed(&far_function(1), &spaces, false).globals[0].address_space, 5);
        assert!(matches!(placed(&far_function(5), &spaces, false), Cow::Borrowed(_)));
    }
}
