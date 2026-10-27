//! Mark internal globals that are only ever loaded from as `constant`, drop
//! the stores of those only ever stored to, and make a global one function
//! always writes before it reads that function's stack slot.
//!
//! # How it works
//! A variable qualifies when it is internal or private, initialized, not yet
//! `constant`, and not named by any other global's initializer. Every use of it
//! in every function must then be a load's address, reached through constant
//! `gep`s and `bitcast`/`addrspacecast`s of its address, and through the same
//! instructions at run time as long as all their users load. Any other use (a
//! store, a call argument, a compare, an escape into a value) disqualifies it.
//!
//! A qualifying variable gets `constant = true`. A load of it is then its
//! initializer's bytes (`memory::constant_bits`), which the body passes fold.
//! Every defined function is reported as changed so each body is run through
//! its pipeline again.
//!
//! `localize` (LLVM's `processInternalGlobal`, last step) turns an internal
//! variable of one value that a single function uses, every load of which a
//! store of that function precedes, into the function's own stack slot; the
//! per-body passes promote it. The function must not recurse (no call but to
//! intrinsics, or `norecurse`).
//!
//! `WriteOnly`, after the pipeline has forwarded what loads it could, is the
//! other half (LLVM's GlobalOpt: "if the global is never loaded (but may be
//! stored to), it is dead"; gcc's ipa `write-only` flag): a variable that is
//! internal or private, not named by another global's initializer, and whose
//! every use in every function is a non-volatile store's address (through the
//! same `gep`s and casts) loses those stores. Nothing names it then, and
//! `globaldce` deletes it.
//!
//! # Example
//! Before:
//!
//! ```mir
//! @c = internal global [2 x i16] [i16 5, i16 12]
//! @w = internal global i16 7
//!
//! define i16 @f(i16 %x) {
//! b0:
//!   %at = getelementptr i8, ptr @c, i16 2
//!   %a = load i16, ptr %at
//!   %b = load i16, ptr @w
//!   %s = add i16 %a, %b
//!   ret i16 %s
//! }
//!
//! define void @g(i16 %x) {
//! b0:
//!   store i16 %x, ptr @w
//!   ret void
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! @c = internal constant [2 x i16] [i16 5, i16 12]
//! @w = internal global i16 7
//! ; ... @f and @g unchanged
//! ```
//!
//! `@c` is only loaded and becomes `constant`; `@w` is stored by `@g` and stays
//! a variable. The body passes then fold the load of `@c` to 12.
//!
//! From the fixture of unit test `a_global_only_loaded_is_its_initializer` in
//! `globalopt_tests.rs`, run through `GlobalOpt` alone.
//!
//! # Runs
//! Once, as a module pass before the per-body pipeline and before the
//! interprocedural step. Not run at -O0.
//!
//! # See also
//! LLVM's GlobalOpt, which also marks globals it finds never stored.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::globalsaa::embedded;
use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Module, Operand};
use llrm_mir::types::Type;
use llrm_mir::opcode::{CastOp, Flags, Opcode};
use llrm_mir::passes::{ModuleAnalyses, ModulePass};

pub struct GlobalOpt;

impl ModulePass for GlobalOpt {
    fn name(&self) -> &'static str {
        "globalopt"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let found = never_stored(module);
        for &id in &found {
            if let GlobalKind::Variable(variable) = &mut module.globals[id.0 as usize].kind {
                variable.constant = true;
            }
        }
        let localized = localize(module);
        if found.is_empty() && !localized {
            Vec::new()
        } else {
            module.functions().filter(|(_, _, one)| !one.is_declaration()).map(|(id, _, _)| id).collect()
        }
    }
}

/// `processInternalGlobal`'s last step: an internal variable of one value that
/// a single function uses, every load of which a store of that function
/// precedes (so its initial value is never read), is that function's own stack
/// slot (`isPointerValueDeadOnEntryToFunction`). The function cannot recurse:
/// it makes no call, or says `norecurse`. The variable is left unnamed for
/// `globaldce`; the slot is for the per-body passes to promote.
///
/// DEVIATE: LLVM also stores the initializer at the function's entry, which
/// nothing reads here; the function-attribute pass has not run yet where this
/// does, so "cannot recurse" is read from the body.
fn localize(module: &mut Module) -> bool {
    let held = held_by_live(module);
    let mut named = BTreeMap::new();
    let mut users: BTreeMap<GlobalId, Vec<(GlobalId, Reach)>> = BTreeMap::new();
    let mut unusable: BTreeSet<GlobalId> = BTreeSet::new();
    for (id, _, function) in module.functions() {
        for (global, reach) in reach_all(&module.context, function, &mut named) {
            match reach {
                Some(reach) => users.entry(global).or_default().push((id, reach)),
                None => {
                    unusable.insert(global);
                }
            }
        }
    }
    let mut chosen: Vec<(GlobalId, GlobalId, Reach)> = Vec::new();
    for (global, mut found) in users {
        let [(function, _)] = &found[..] else { continue };
        let function = *function;
        if unusable.contains(&global) || held.contains(&global) {
            continue;
        }
        let value = &module.global(global);
        let GlobalKind::Variable(variable) = &value.kind else { continue };
        let single = matches!(module.context.types.get(variable.ty), Type::Int(_) | Type::Float(_) | Type::Pointer(_));
        let internal = matches!(value.linkage, Linkage::Internal | Linkage::Private);
        if !(single && internal && !variable.constant && !variable.externally_initialized && value.address_space == 0) {
            continue;
        }
        let GlobalKind::Function(body) = &module.global(function).kind else { continue };
        let (_, reach) = found.pop().expect("one user");
        if reach.moved.is_empty()
            && cannot_recurse(module, body)
            && dead_on_entry(&module.context, body, global, variable.ty, &reach)
        {
            chosen.push((global, function, reach));
        }
    }
    let changed = !chosen.is_empty();
    let Module { context, globals, .. } = module;
    for (global, function, reach) in chosen {
        let (ty, align) = match &globals[global.0 as usize].kind {
            GlobalKind::Variable(variable) => (variable.ty, variable.align),
            GlobalKind::Function(_) => continue,
        };
        let GlobalKind::Function(body) = &mut globals[function.0 as usize].kind else { continue };
        let Some(entry) = body.entry() else { continue };
        let Some(first) = body.iter_block(entry).next() else { continue };
        let pointer = context.types.ptr(0);
        let slot = body.create_instruction(
            Opcode::Alloca { allocated: ty, align, address_space: 0 },
            pointer,
            Vec::new(),
            Flags::default(),
            None,
        );
        if body.insert(slot, Position::Before(first)).is_err() {
            continue;
        }
        let address = Operand::Value(body.instruction(slot).result.expect("an alloca has a value"));
        for (inst, index) in reach.uses {
            body.set_operand(inst, index, address);
        }
    }
    changed
}

/// Whether `body` cannot be re-entered: `norecurse`, or no call but to
/// intrinsics.
fn cannot_recurse(
    module: &Module,
    body: &Function,
) -> bool {
    if body.attrs.iter().any(|one| matches!(one, llrm_mir::opcode::Attribute::Flag(name) if name == "norecurse")) {
        return true;
    }
    body.walk().all(|(_, inst)| {
        !matches!(body.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_))
            || llrm_mir::memory::callee(&module.context, body, inst)
                .and_then(|one| module.global(one).intrinsic())
                .is_some()
    })
}

/// Whether every use of `global` in `body` is a plain load or store of its
/// own type through its address, and a store precedes each load.
fn dead_on_entry(
    context: &Context,
    body: &Function,
    global: GlobalId,
    ty: llrm_mir::types::TypeId,
    reach: &Reach,
) -> bool {
    let (mut loads, mut stores) = (Vec::new(), Vec::new());
    for &(inst, index) in &reach.uses {
        let op = body.instruction(inst);
        let direct = matches!(op.operands[index], Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Global(one) if one == global));
        match op.opcode {
            Opcode::Load { volatile: false, .. } if index == 0 && direct && op.ty == ty => loads.push(inst),
            Opcode::Store { volatile: false, .. }
                if index == 1 && direct && body.operand_type(context, op.operands[0]) == Some(ty) =>
            {
                stores.push(inst)
            }
            _ => return false,
        }
    }
    let tree = llrm_mir::domtree::DomTree::of(body);
    loads.iter().all(|&load| stores.iter().any(|&store| tree.instruction_dominates(body, store, load)))
}

/// The internal, initialized, not yet constant globals every use of which,
/// through address arithmetic and casts, is a load's address.
pub fn never_stored(module: &Module) -> Vec<GlobalId> {
    let held = held_by_live(module);
    let candidates = module.globals.iter().enumerate().filter_map(|(at, global)| {
        let GlobalKind::Variable(variable) = &global.kind else { return None };
        let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
        (internal && !variable.constant && variable.initializer.is_some() && !held.contains(&GlobalId(at as u32)))
            .then_some(GlobalId(at as u32))
    });
    only_used(module, candidates.collect(), &only_loaded)
}

/// The globals the initializers of the variables something reaches name. A
/// variable nothing reaches (QB's `X$descriptor` of a variable no code takes
/// the descriptor of) names nothing: LLVM's GlobalOpt deletes it and goes on
/// with the rest in its next round, where this pass runs once.
pub(crate) fn held_by_live(module: &Module) -> BTreeSet<GlobalId> {
    let mut live: BTreeSet<GlobalId> = BTreeSet::new();
    let mut pending: Vec<GlobalId> = Vec::new();
    for (at, global) in module.globals.iter().enumerate() {
        let id = GlobalId(at as u32);
        match &global.kind {
            GlobalKind::Function(function) => {
                pending.push(id);
                let mut named = BTreeSet::new();
                function.personality.iter().for_each(|&one| embedded(&module.context, one, &mut named));
                for (_, inst) in function.walk() {
                    for operand in &function.instruction(inst).operands {
                        if let Operand::Constant(one) = operand {
                            embedded(&module.context, *one, &mut named);
                        }
                    }
                }
                pending.extend(named);
            }
            GlobalKind::Variable(_) if !matches!(global.linkage, Linkage::Internal | Linkage::Private) => {
                pending.push(id);
            }
            GlobalKind::Variable(_) => {}
        }
    }
    let mut held = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !live.insert(id) {
            continue;
        }
        if let GlobalKind::Variable(variable) = &module.global(id).kind {
            let mut named = BTreeSet::new();
            variable.initializer.iter().for_each(|&one| embedded(&module.context, one, &mut named));
            held.extend(named.iter().copied());
            pending.extend(named);
        }
    }
    held
}

/// The `candidates` every use of which, in every function, `access` says is an
/// address it may be: the function's uses found once for all of them, not
/// scanned again for each.
fn only_used(
    module: &Module,
    mut left: Vec<GlobalId>,
    access: &dyn Fn(&Module, &Function, &llrm_mir::module::Instruction, usize) -> bool,
) -> Vec<GlobalId> {
    let mut named = BTreeMap::new();
    for (_, _, function) in module.functions() {
        if left.is_empty() {
            break;
        }
        let reached = reach_all(&module.context, function, &mut named);
        left.retain(|id| match reached.get(id) {
            None => true,
            Some(None) => false,
            Some(Some(found)) => {
                found.uses.iter().all(|&(inst, index)| access(module, function, function.instruction(inst), index))
            }
        });
    }
    left
}

/// Whether every use of `global` in `function` only reads through it: a load,
/// or the source of a non-volatile `memcpy` or `memmove` (`AnalyzeGlobal`,
/// GlobalStatus.cpp: a transfer from the global is `IsLoaded`, a transfer into
/// it is a store). The C front end's aggregate initializers are a `memcpy` from
/// an internal global; until SROA turns it into loads, only this makes it
/// `constant`.
fn only_loaded(
    module: &Module,
    function: &Function,
    op: &llrm_mir::module::Instruction,
    index: usize,
) -> bool {
    let context = &module.context;
    match op.opcode {
        Opcode::Load { .. } => index == 0,
        Opcode::Call(_) => {
            use llrm_mir::intrinsics::Intrinsic;
            let volatile = match op.operands.get(3) {
                Some(Operand::Constant(id)) => {
                    !matches!(
                        context.get(*id).kind,
                        ConstantKind::Int(0) | ConstantKind::Zero
                    )
                }
                _ => true,
            };
            index == 1
                && !volatile
                && callee_intrinsic(module, function, op)
                    .is_some_and(|one| matches!(one, Intrinsic::MemCpy | Intrinsic::MemMove))
        }
        _ => false,
    }
}

/// The intrinsic the call `op` of `function` calls.
fn callee_intrinsic(
    module: &Module,
    function: &Function,
    op: &llrm_mir::module::Instruction,
) -> Option<llrm_mir::intrinsics::Intrinsic> {
    let Operand::Constant(callee) = *op.operands.last()? else { return None };
    let ConstantKind::Global(callee) = module.context.get(callee).kind else { return None };
    let _ = function;
    module.globals.get(callee.0 as usize)?.intrinsic()
}

/// Every way `function` uses `global`'s address, found once: the operands that
/// take the address as it stands (`uses`), and the address arithmetic and casts
/// between the global and them (`moved`).
pub(crate) struct Reach {
    pub(crate) uses: Vec<(InstId, usize)>,
    pub(crate) moved: Vec<InstId>,
}

thread_local! {
    static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many bodies this thread has scanned for global uses.
#[cfg(test)]
pub(crate) fn scans() -> usize {
    SCANS.with(std::cell::Cell::get)
}

/// Every global's uses in `function`, found in one scan, with `named` holding
/// the globals each constant names across calls: none for a global one of whose
/// uses is not its address, and no entry for a global the function does not
/// use.
pub(crate) fn reach_all(
    context: &Context,
    function: &Function,
    named: &mut BTreeMap<ConstantId, BTreeSet<GlobalId>>,
) -> BTreeMap<GlobalId, Option<Reach>> {
    SCANS.with(|count| count.set(count.get() + 1));
    let mut work: BTreeMap<GlobalId, Vec<(InstId, usize)>> = BTreeMap::new();
    let mut bad: BTreeSet<GlobalId> = BTreeSet::new();
    for (_, inst) in function.walk() {
        for (index, &operand) in function.instruction(inst).operands.iter().enumerate() {
            let Operand::Constant(id) = operand else { continue };
            let globals = named
                .entry(id)
                .or_insert_with(
                    || {
                        let mut found = BTreeSet::new();
                        embedded(context, id, &mut found);
                        found
                    },
                );
            for &global in globals.iter() {
                if addresses(context, id, global) {
                    work.entry(global).or_default().push((inst, index));
                } else {
                    bad.insert(global);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for (global, mut work) in work {
        if bad.contains(&global) {
            out.insert(global, None);
            continue;
        }
        let mut found = Reach { uses: Vec::new(), moved: Vec::new() };
        while let Some((inst, index)) = work.pop() {
            let op = function.instruction(inst);
            match op.opcode {
                Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) if index == 0 => {
                    found.moved.push(inst);
                    if let Some(value) = op.result {
                        work.extend(function.users(value).iter().map(|one| (one.user, one.index as usize)));
                    }
                }
                _ => found.uses.push((inst, index)),
            }
        }
        out.insert(global, Some(found));
    }
    for global in bad {
        out.insert(global, None);
    }
    out
}

/// Whether `constant` is `global`'s address, moved or cast.
pub(crate) fn addresses(
    context: &Context,
    constant: ConstantId,
    global: GlobalId,
) -> bool {
    match &context.get(constant).kind {
        ConstantKind::Global(one) => *one == global,
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
            operands.first().is_some_and(|&base| addresses(context, base, global))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
            addresses(context, *value, global)
        }
        _ => false,
    }
}

/// The stores of the globals nothing loads: removed from every body, which are
/// the returned.
pub struct WriteOnly;

impl ModulePass for WriteOnly {
    fn name(&self) -> &'static str {
        "writeonly"
    }

    fn run(
        &mut self,
        module: &mut Module,
        _: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let found = never_loaded(module);
        if found.is_empty() {
            return Vec::new();
        }
        let Module { context, globals, .. } = module;
        let mut changed = Vec::new();
        let mut named = BTreeMap::new();
        for (at, global) in globals.iter_mut().enumerate() {
            let GlobalKind::Function(function) = &mut global.kind else { continue };
            // Every use of a found global is a store's address or address
            // arithmetic that only stores use, so the stores and the arithmetic
            // between them and the globals all go; arithmetic left unused
            // before goes with them.
            let mut dead = std::collections::BTreeSet::new();
            let reached = reach_all(context, function, &mut named);
            for global in &found {
                if let Some(Some(reach)) = reached.get(global) {
                    dead.extend(reach.uses.iter().map(|&(inst, _)| inst).chain(reach.moved.iter().copied()));
                }
            }
            let dead: Vec<InstId> = dead.into_iter().collect();
            if !dead.is_empty() && function.erase_all(&dead).is_ok() {
                changed.push(GlobalId(at as u32));
            }
        }
        changed
    }
}

/// The internal variables no body loads from, and whose every use is a
/// non-volatile store's address.
pub fn never_loaded(module: &Module) -> Vec<GlobalId> {
    let held = held_by_live(module);
    let stores = |op: &llrm_mir::module::Instruction, index: usize| {
        matches!(op.opcode, Opcode::Store { volatile: false, .. }) && index == 1
    };
    let candidates = module
        .globals
        .iter()
        .enumerate()
        .filter_map(
            |(at, global)| {
                let GlobalKind::Variable(variable) = &global.kind else { return None };
                let internal = matches!(global.linkage, Linkage::Internal | Linkage::Private);
                (internal && !variable.constant && !held.contains(&GlobalId(at as u32))).then_some(GlobalId(at as u32))
            },
        );
    only_used(module, candidates.collect(), &|_, _, op, index| stores(op, index))
}

#[cfg(test)]
#[path = "globalopt_tests.rs"]
mod tests;
