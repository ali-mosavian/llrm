//! Interprocedural sparse conditional constant propagation. A translation of
//! LLVM's
//! [`runIPSCCP`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L108)
//! (d1106deb71cc) without function specialisation: one solve over every body
//! of a module, then the rewrites. What a body's callers pass becomes its
//! arguments' constants and `range` attributes, what it returns reaches each
//! call, and a global only ever stored one value is read as that value.
//!
//! # How it works
//! [`run_ipsccp`] gives each module's defined functions to
//! [`Solver::module`]. A function whose every caller the module can see (it
//! is private and its address is not taken) has its arguments tracked: its
//! entry runs only when a call reaches it, and each argument is the join of
//! what the calls pass. Any other function is entered from outside, with its
//! arguments as their attributes say. Each return whose definition is exact
//! is tracked, as is each private scalar global that only non-volatile loads
//! and stores of its type name.
//!
//! After the solve, each body has its arguments that are constants replaced
//! (if its entry runs), its values replaced by their constants, its blocks
//! that never run emptied and deleted (an entry that never runs becomes
//! `unreachable`), and its edges that are never taken cut. Then the tracked
//! returns and arguments get their `range` or `nonnull` attributes
//! (`inferReturnAttributes`, `inferArgAttributes`). A tracked function whose
//! return is a constant (or never computed) and that only the module calls
//! returns `poison`, since every call already reads the constant; it and its
//! calls lose `returned` and the attributes that would make `poison`
//! undefined. The stores to a global whose value is known go, as do the loads
//! left.
//!
//! ```text
//!  @g(i32 %a): ret %a * 5           @g(i32 range(i32 1, 4) %a): ret %a * 5
//!  @f: call @g(1); call @g(3)  ==>  @f: call @g(1); call @g(3)
//! ```
//!
//! # DEVIATEs
//! 1. No `FunctionSpecializer`
//!    ([L161](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L161),
//!    on at -O3 in the pin): [`crate::sccpsolver`] DEVIATE 2. Exit: its
//!    translation.
//! 2. The solve is per module of a program, and
//!    `canTrackArgumentsInterprocedurally`
//!    ([L133](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L133))
//!    is `hasExactDefinition` and [`crate::interprocedural`]'s `private`: not
//!    exported by the program, reached by no other module, not address-taken
//!    and not `noipa`, where the pin asks `hasLocalLinkage` and
//!    `!hasAddressTaken`. For one module with no link list the two agree,
//!    except that a function a metadata node names is address-taken here.
//! 3. A tracked global whose stores went stays in the module, with no users;
//!    the pin erases it and rewrites its debug expression
//!    ([L344-L361](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L344)).
//!    Erasing renumbers the module's globals under the program pass's
//!    analyses; GlobalOpt's dead-global step deletes it. MIR has no debug
//!    expressions.
//! 4. `canTrackGlobalVariableInterprocedurally`'s users are the operands of the
//!    module's instructions and its globals' initializers (MIR constants have
//!    no use lists, so `removeDeadConstantUsers`
//!    ([L153](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L153))
//!    has nothing to do); a constant expression naming the global is a user
//!    that is neither load nor store.
//! 5. A pointer argument replaced moves the body's argument memory effects to
//!    other memory with `memory::unsplit`
//!    ([L195](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L195));
//!    MIR has no errno location.
//! 6. A dead block is deleted even when its address is taken (`hasAddressTaken`
//!    at
//!    [L256](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L256)):
//!    MIR has no `blockaddress`.
//! 7. `getMRVFunctionsTracked` is visited in module order (a `SmallPtrSet` in
//!    the pin); which returns are zapped does not depend on it.
//! 8. What [`run_ipsccp`] returns is every body it edited; the pin's
//!    `MadeChanges` leaves out a non-pointer argument replaced
//!    ([L199](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L199)).
//!
//! The solver's and the rewrites' DEVIATEs are those of
//! [`crate::sccpsolver`].
//!
//! # Example
//! `@g` is called with 1 and 3 and its argument becomes `range(i32 1, 4) %a`:
//! unit test `test_a_callee_called_with_1_and_3_gets_the_range_1_to_4` in
//! `crates/opt/llrm-transforms/src/ipsccp_tests.rs`.
//!
//! # Runs
//! Once per compile at -O1 and up, first in [`crate::walk::ModuleIpo`]
//! ([`run_ipsccp_with`]): after the early function pipeline, before the
//! CGSCC walk, where the pin's `buildModuleSimplificationPipeline` runs
//! `IPSCCPPass`.
//!
//! # See also
//! - Uses: [`crate::sccpsolver`], [`crate::sccp`]'s rewrites,
//!   [`crate::predicateinfo`]

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::callgraph::Defined;
use llrm_mir::context::{Constant, ConstantExpr, ConstantKind, Context, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::facts::Fact;
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::{Attribute, CastOp, Opcode, Tail};
use llrm_mir::passes::{ModuleAnalyses, Outer};
use llrm_mir::program::Program;
use llrm_mir::types::Type;
use llrm_mir::valuetracking::AssumptionCache;

use crate::local;
use crate::predicateinfo::PredicateInfo;
use crate::sccp::{Rewrite, can_track_returns, has_exact_definition, infer_attribute, remove_ssa_copies};
use crate::sccpsolver::{ModuleEnv, Results, Solver, is_replaceable_constant};

/// `runIPSCCP` over each module of `program`; the bodies it changed.
pub fn run_ipsccp(program: &mut Program) -> BTreeSet<Defined> {
    solved_each(program, |program, at| Rc::new(Outer::of(&program.modules[at], Some(Rc::clone(&program.target)))))
}

/// [`run_ipsccp`], each module's outer view the one `modules` holds. The
/// caller reports the bodies it changed.
pub fn run_ipsccp_with(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
) -> BTreeSet<Defined> {
    solved_each(program, |program, at| modules[at].outer(&program.modules[at]))
}

fn solved_each(
    program: &mut Program,
    mut outer_of: impl FnMut(&Program, usize) -> Rc<Outer>,
) -> BTreeSet<Defined> {
    let mut changed = BTreeSet::new();
    for at in 0..program.modules.len() {
        let private = crate::interprocedural::private(program, at);
        let outer = outer_of(program, at);
        let Program { modules, layout, .. } = &mut *program;
        for id in run_on_module(&mut modules[at], layout, &outer, &private) {
            changed.insert((at, id));
        }
    }
    changed
}

/// `canTrackGlobalVariableInterprocedurally`: a private variable that is not
/// constant, has an initializer that stays, and that only non-volatile loads
/// and stores of its own type name (none storing its address).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLatticeUtils.cpp#L28>
fn trackable_globals(module: &Module) -> Vec<GlobalId> {
    let context = &module.context;
    let mut candidates: BTreeSet<GlobalId> = module
        .globals
        .iter()
        .enumerate()
        .filter(|(_, global)| match &global.kind {
            GlobalKind::Variable(variable) => {
                !variable.constant
                    && matches!(global.linkage, Linkage::Internal | Linkage::Private)
                    && variable.initializer.is_some()
                    && !variable.externally_initialized
            }
            GlobalKind::Function(_) => false,
        })
        .map(|(at, _)| GlobalId(at as u32))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let named = |id| match context.get(id).kind {
        ConstantKind::Global(global) => Some(global),
        _ => None,
    };
    // A use inside an aggregate or a constant expression is neither a load
    // nor a store.
    let mut nested = Vec::new();
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            nested.extend(variable.initializer);
        }
    }
    for (_, _, function) in module.functions() {
        nested.extend(function.personality);
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let ty_of = |op: Operand| function.operand_type(context, op);
            for (index, &operand) in instruction.operands.iter().enumerate() {
                let Operand::Constant(id) = operand else { continue };
                let Some(global) = named(id) else {
                    nested.push(id);
                    continue;
                };
                if !candidates.contains(&global) {
                    continue;
                }
                let GlobalKind::Variable(variable) = &module.global(global).kind else { continue };
                let fits = match instruction.opcode {
                    Opcode::Store { volatile, .. } => {
                        index == 1 && !volatile && ty_of(instruction.operands[0]) == Some(variable.ty)
                    }
                    Opcode::Load { volatile, .. } => index == 0 && !volatile && instruction.ty == variable.ty,
                    _ => false,
                };
                if !fits {
                    candidates.remove(&global);
                }
            }
        }
    }
    while let Some(id) = nested.pop() {
        match &context.get(id).kind {
            ConstantKind::Global(global) => {
                candidates.remove(global);
            }
            ConstantKind::Aggregate(members) => nested.extend(members),
            ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => nested.extend(operands),
            ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => nested.push(*value),
            _ => {}
        }
    }
    candidates.into_iter().collect()
}

fn body_mut(
    module: &mut Module,
    id: GlobalId,
) -> (&Context, &mut Function) {
    let Module { context, globals, .. } = module;
    match &mut globals[id.0 as usize].kind {
        GlobalKind::Function(function) => (context, function),
        GlobalKind::Variable(_) => unreachable!("a defined function"),
    }
}

/// `runIPSCCP` of one module: the functions it changed.
fn run_on_module(
    module: &mut Module,
    layout: &DataLayout,
    outer: &Outer,
    private: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    let ids: Vec<GlobalId> =
        module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect();
    if ids.is_empty() {
        return BTreeSet::new();
    }
    // Loop over all functions, marking arguments to those with their
    // addresses taken or that are external as overdefined.
    let mut predicates = Vec::with_capacity(ids.len());
    let mut tracks_returns = Vec::with_capacity(ids.len());
    let mut tracks_arguments = Vec::with_capacity(ids.len());
    for &id in &ids {
        let linkage = module.global(id).linkage;
        let Module { context, globals, metadata, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let dt = DomTree::of(function);
        let ac = AssumptionCache::of(context, &outer.globals, metadata, function);
        predicates.push(PredicateInfo::new(context, &outer.globals, metadata, function, &dt, &ac));
        tracks_returns.push(can_track_returns(linkage, function));
        tracks_arguments.push(private.contains(&id) && has_exact_definition(linkage, function));
    }
    let globals_tracked = trackable_globals(module);
    let solved = {
        let Module { context, globals, metadata, .. } = &mut *module;
        let functions = ids
            .iter()
            .zip(&predicates)
            .map(|(&id, info)| (id, globals[id.0 as usize].function().expect("a function"), info))
            .collect();
        let env = ModuleEnv { context, layout, globals, metadata };
        let mut solver = Solver::module(env, functions).expect("a defined function");
        for (at, &id) in ids.iter().enumerate() {
            // Determine if we can track the function's return values.
            if tracks_returns[at] {
                solver.add_tracked_function(id);
            }
            // Determine if we can track the function's arguments.
            if tracks_arguments[at] {
                solver.add_argument_tracked_function(id);
                continue;
            }
            // Assume the function is called.
            let function = globals[id.0 as usize].function().expect("a function");
            solver.mark_block_executable_in(id, function.entry().expect("a body"));
            for index in 0..function.parameters().len() {
                solver.track_value_of_argument_in(id, index as u32);
            }
        }
        for &global in &globals_tracked {
            solver.track_value_of_global_variable(global);
        }
        // Solve for constants.
        solver.solve_while_resolved_undefs_in();
        solver.into_module_results()
    };

    // Iterate over all of the instructions in the module, replacing them with
    // constants if we have found them to be of constant values.
    let mut changed = BTreeSet::new();
    let mut results: Vec<Results> = Vec::with_capacity(ids.len());
    let mut preserved: BTreeSet<GlobalId> = BTreeSet::new();
    let mut pointer_replaced = Vec::new();
    for ((&id, (solved_id, solved)), info) in ids.iter().zip(solved.functions).zip(&predicates) {
        debug_assert_eq!(solved_id, Some(id));
        let Module { context, globals, metadata, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let mut rewrite = Rewrite::new(context, layout, outer, metadata, solved);
        let mut made = false;
        let entry = function.entry().expect("a body");
        if rewrite.results.is_block_executable(entry) {
            let mut replaced_pointer_arg = false;
            for argument in function.parameters().to_vec() {
                if !function.users(argument).is_empty()
                    && rewrite.try_to_replace_value_with_constant(function, argument, None)
                {
                    replaced_pointer_arg |= matches!(
                        context.types.get(function.value(argument).ty),
                        Type::Pointer(_)
                    );
                    made = true;
                }
            }
            // If we replaced an argument, we may now also access a global
            // (currently classified as "other" memory).
            if replaced_pointer_arg {
                pointer_replaced.push(id);
            }
        }
        let mut blocks_to_erase = Vec::new();
        for block in function.layout().to_vec() {
            if !rewrite.results.is_block_executable(block) {
                made = true;
                if block != entry {
                    blocks_to_erase.push(block);
                }
                continue;
            }
            made |= rewrite.simplify_insts_in_block(function, block);
        }
        // Change dead blocks to unreachable. We do it after replacing
        // constants in all executable blocks, because changeToUnreachable may
        // remove PHI nodes in executable blocks we found values for. The
        // function's entry block is not part of BlocksToErase, so we have to
        // handle it separately.
        let first_non_phi = |function: &Function, block| {
            function
                .iter_block(block)
                .find(|&one| function.instruction(one).opcode != Opcode::Phi)
                .expect("a block has a terminator")
        };
        for &dead in &blocks_to_erase {
            let first = first_non_phi(function, dead);
            local::change_to_unreachable(context, function, first);
        }
        if !rewrite.results.is_block_executable(entry) {
            let first = first_non_phi(function, entry);
            local::change_to_unreachable(context, function, first);
        }
        let mut new_unreachable = None;
        for block in function.layout().to_vec() {
            made |= rewrite.remove_non_feasible_edges(function, block, &mut new_unreachable);
        }
        for &dead in &blocks_to_erase {
            rewrite.delete_block(function, dead);
        }
        remove_ssa_copies(function, info);
        preserved.extend(rewrite.preserved.iter().copied());
        results.push(rewrite.results);
        if made {
            changed.insert(id);
        }
    }
    for &id in &pointer_replaced {
        moved_out_of_arguments(module, id);
        changed.insert(id);
    }

    // `inferReturnAttributes` and `inferArgAttributes`.
    for (&id, solved) in ids.iter().zip(&results) {
        let (context, function) = body_mut(module, id);
        let Type::Function { returns, .. } = context.types.get(function.ty).clone() else { continue };
        if let Some(value) = solved.tracked_return()
            && infer_attribute(context, &mut function.return_attrs, returns, value)
        {
            changed.insert(id);
        }
    }
    for (&id, solved) in ids.iter().zip(&results) {
        let (context, function) = body_mut(module, id);
        if !solved.is_argument_tracked() || !solved.is_block_executable(function.entry().expect("a body")) {
            continue;
        }
        for (index, argument) in function.parameters().to_vec().into_iter().enumerate() {
            let ty = function.value(argument).ty;
            if context.types.fields(ty).is_some() {
                continue;
            }
            if function.parameter_attrs.len() <= index {
                function.parameter_attrs.resize(index + 1, Vec::new());
            }
            let lv = solved.lattice_value(argument);
            if infer_attribute(context, &mut function.parameter_attrs[index], ty, &lv) {
                changed.insert(id);
            }
        }
    }

    // If we inferred constant or undef return values for a function, we
    // replaced all call uses with the inferred value. This means we don't
    // need to bother actually returning anything from the function.
    let mut returns_to_zap: Vec<(GlobalId, InstId)> = Vec::new();
    for (&id, solved) in ids.iter().zip(&results) {
        if let Some(value) = solved.tracked_return()
            && (is_replaceable_constant(value) || value.is_unknown_or_undef())
        {
            find_returns_to_zap(module, id, solved, &preserved, &mut returns_to_zap);
        }
    }
    for (&id, solved) in ids.iter().zip(&results) {
        if solved.tracked_multiple().is_some() && solved.is_struct_lattice_constant() {
            find_returns_to_zap(module, id, solved, &preserved, &mut returns_to_zap);
        }
    }
    // Zap all returns which we've identified as zap to change.
    let mut zapped: Vec<GlobalId> = Vec::new();
    for (id, ret) in returns_to_zap {
        let (context, function) = body_mut(module, id);
        let Type::Function { returns, .. } = context.types.get(function.ty).clone() else { unreachable!("a function") };
        let poison = context.constant(Constant { ty: returns, kind: ConstantKind::Poison });
        function.set_operand(ret, 0, Operand::Constant(poison));
        if !zapped.contains(&id) {
            zapped.push(id);
        }
        changed.insert(id);
    }
    // Remove the returned attribute for zapped functions and the
    // corresponding call sites. Also remove any attributes that convert an
    // undef return value into immediate undefined behavior.
    for &id in &zapped {
        let (_, function) = body_mut(module, id);
        for attrs in &mut function.parameter_attrs {
            attrs.retain(|one| !is_returned(one));
        }
        function.return_attrs.retain(|one| !is_ub_implying(one));
        for caller in calls_to(module, id) {
            let (at, call) = caller;
            let (_, function) = body_mut(module, at);
            let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else { continue };
            let mut info = (**info).clone();
            for attrs in &mut info.argument_attrs {
                attrs.retain(|one| !is_returned(one));
            }
            info.return_attrs.retain(|one| !is_ub_implying(one));
            function.set_call_attributes(call, info.attrs, info.return_attrs, info.argument_attrs);
            changed.insert(at);
        }
    }

    // If we inferred constant or undef values for globals variables, we can
    // delete the global and any stores that remain to it.
    let mut tracked: Vec<_> = solved.tracked_globals.into_iter().collect();
    tracked.sort_by_key(|(global, _)| *global);
    for (global, lv) in tracked {
        // We may have inferred a constant that is not replaceable (e.g., due
        // to different pointer provenance), so skip deleting the global in
        // this case.
        if !is_replaceable_constant(&lv) && !lv.is_unknown_or_undef() {
            continue;
        }
        for &id in &ids {
            let (context, function) = body_mut(module, id);
            let users: Vec<InstId> = function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| {
                    let instruction = function.instruction(inst);
                    let pointer = match instruction.opcode {
                        Opcode::Load { .. } => instruction.operands[0],
                        Opcode::Store { .. } => instruction.operands[1],
                        _ => return false,
                    };
                    matches!(
                        pointer,
                        Operand::Constant(one) if context.get(one).kind == ConstantKind::Global(global)
                    )
                })
                .collect();
            for inst in users {
                let instruction = function.instruction(inst);
                if let Some(result) = instruction.result {
                    let poison = context.constant(Constant { ty: instruction.ty, kind: ConstantKind::Poison });
                    function.replace_all_uses_with(result, Operand::Constant(poison));
                }
                function.erase(inst).expect("its uses were replaced");
                changed.insert(id);
            }
        }
    }
    changed
}

/// The `ReplacedPointerArg` update: the function `id` and its direct calls
/// may now reach through a constant what they reached through the argument.
fn moved_out_of_arguments(
    module: &mut Module,
    id: GlobalId,
) {
    let (_, function) = body_mut(module, id);
    llrm_mir::memory::unsplit(&mut function.attrs, llrm_mir::memory::Moved::OutOfArguments);
    for (at, call) in calls_to(module, id) {
        let (_, function) = body_mut(module, at);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else { continue };
        let mut info = (**info).clone();
        llrm_mir::memory::unsplit(&mut info.attrs, llrm_mir::memory::Moved::OutOfArguments);
        function.set_call_attributes(call, info.attrs, info.return_attrs, info.argument_attrs);
    }
}

/// The calls in `module` that name `callee` directly.
fn calls_to(
    module: &Module,
    callee: GlobalId,
) -> Vec<(GlobalId, InstId)> {
    let mut out = Vec::new();
    for (id, _, function) in module.functions() {
        for (_, inst) in function.walk() {
            if matches!(
                function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            )
                && llrm_mir::memory::callee(&module.context, function, inst) == Some(callee)
            {
                out.push((id, inst));
            }
        }
    }
    out
}

fn is_returned(attr: &Attribute) -> bool {
    matches!(attr, Attribute::Flag(name) if name == "returned")
}

/// `AttributeFuncs::getUBImplyingAttributes`.
fn is_ub_implying(attr: &Attribute) -> bool {
    matches!(
        Fact::of_attribute(attr),
        Some(Fact::NoUndef | Fact::Dereferenceable(_))
    )
        || matches!(
            attr,
            Attribute::Int(name, _) if name == "dereferenceable_or_null"
        )
}

/// `findReturnsToZap`: the returns of `id` that may return `poison`, since
/// every call reads the tracked value instead.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/SCCP.cpp#L50>
fn find_returns_to_zap(
    module: &Module,
    id: GlobalId,
    solved: &Results,
    preserved: &BTreeSet<GlobalId>,
    returns_to_zap: &mut Vec<(GlobalId, InstId)>,
) {
    // We can only do this if we know that nothing else can call the function.
    if !solved.is_argument_tracked() {
        return;
    }
    if solved.must_preserve_return() || preserved.contains(&id) {
        return;
    }
    let context = &module.context;
    let function = module.global(id).function().expect("a function");
    for &block in function.layout() {
        if terminating_must_tail_call(function, block).is_some() {
            return;
        }
        let Some(term) = function.terminator(block) else { continue };
        let instruction = function.instruction(term);
        if instruction.opcode != Opcode::Ret {
            continue;
        }
        let undef = match instruction.operands.first() {
            Some(Operand::Constant(one)) => {
                matches!(
                    context.get(*one).kind,
                    ConstantKind::Undef | ConstantKind::Poison
                )
            }
            Some(_) => false,
            None => true,
        };
        if !undef {
            returns_to_zap.push((id, term));
        }
    }
}

/// `BasicBlock::getTerminatingMustTailCall`: the `musttail` call the block's
/// `ret` returns, through at most one bitcast.
fn terminating_must_tail_call(
    function: &Function,
    block: llrm_mir::module::BlockId,
) -> Option<InstId> {
    let insts: Vec<InstId> = function.iter_block(block).collect();
    let (&ret, rest) = insts.split_last()?;
    if function.instruction(ret).opcode != Opcode::Ret {
        return None;
    }
    let mut rest = rest.iter().rev();
    let mut previous = *rest.next()?;
    if function.instruction(previous).opcode == Opcode::Cast(CastOp::BitCast) {
        previous = *rest.next()?;
    }
    match &function.instruction(previous).opcode {
        Opcode::Call(info) if info.tail == Tail::MustTail => Some(previous),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ipsccp_tests.rs"]
pub(crate) mod tests;
