//! `norecurse`, `nounwind`, `willreturn` and `noreturn` of one call-graph SCC,
//! as LLVM's `PostOrderFunctionAttrs` infers them from a body and the
//! attributes of its callees.
//!
//! # How it works
//! `infer` takes the members of an SCC, callees already stated, and answers
//! what each may be stamped with. It reads no analysis: a call is judged by
//! its site's attributes and its callee's declaration.
//!
//! - `norecurse` (`addNoRecurseAttrs`): a one-member SCC, every call of which
//!   goes to a function other than itself that states `norecurse` or is a
//!   `nocallback` declaration or an intrinsic (an indirect call with `!callees`
//!   counts each listed target, DEVIATE F2).
//! - `nounwind` (`inferAttrsFromFunctionBodies`): all or none of the SCC. No
//!   member may be a declaration or inexact; no instruction may throw (a call
//!   without `nounwind`, an `invoke`, `resume`) except a `call` to another
//!   member.
//! - `nofree` and `nosync` (`InstrBreaksNoFree`, `InstrBreaksNoSync`): every
//!   call states it or reaches an SCC member; all or none of the SCC.
//! - `willreturn` (`addWillReturn`): `mustprogress` and no writes, or no back
//!   edge and every call states `willreturn`.
//! - `noreturn` (`addNoReturnAttrs`): no path from the entry reaches a `ret`
//!   without a call that states `noreturn` in its block.
//!
//! # Example
//! A function that calls a `norecurse` function and adds is itself
//! `norecurse`, `nounwind` and `willreturn`:
//!
//! ```mir
//! declare i32 @leaf(i32) norecurse nounwind willreturn
//!
//! define i32 @f(i32 %x) {
//! b0:
//!   %a = call i32 @leaf(i32 %x)
//!   %b = add i32 %a, 1
//!   ret i32 %b
//! }
//! ```
//!
//! From unit test `test_a_body_over_norecurse_callees_is_stamped_all_three` in
//! `functionattrs_tests.rs`.
//!
//! - `memory(...)` (`addMemoryAttrs`, `checkFunctionMemoryAccess`): the meet
//!   over the SCC of what each body accesses, by the places its instructions
//!   name; a call is judged by `AAResults::getMemoryEffects`, a location by
//!   `getModRefInfoMask` and its underlying object. Calls inside the SCC are
//!   free, except that their pointer arguments count if the SCC reads argument
//!   memory. DEVIATE F3: a `noipa` body is left out of the SCC (it is this
//!   repository's `optnone`: callers read what the source declared). DEVIATE
//!   F4: no `writable` argument attribute to drop (MIR has none).
//!
//! # Runs
//! `interprocedural::stamped` calls `post_order` once per SCC, callees first:
//! `memory(...)`, the pointer parameters' attributes, then the four flags.
//! DEVIATE F1: `willreturn` also where a loop's trip count is proved or the
//! language promises each loop ends, in a function that does nothing
//! observable (`will_return_by_loops`).
//! Exit: once llrm-cc over clang is the only C frontend its IR carries
//! `mustprogress`, and LLVM's rule gives the same answer; F1 goes then.
//!
//! # See also
//! LLVM `FunctionAttrs.cpp` (`addNoRecurseAttrs`, `addNoReturnAttrs`,
//! `addWillReturn`, `inferAttrsFromFunctionBodies`), `CFG.cpp` (`canReturn`).

use std::collections::BTreeSet;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Function, InstId, Linkage, Module};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::spaces::Spaces;
use llrm_mir::{BlockId, Context, GlobalId, memory};

use crate::aa::{AAQueryInfo, AAResults, IRMemLocation, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::basicaa::BasicAA;
use crate::cfg::Shape;
use crate::effects::{self, Declarations};
use crate::memory::Unit;

/// What one member of an SCC may be stamped with.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Inferred {
    pub no_recurse: bool,
    pub no_unwind: bool,
    pub will_return: bool,
    pub no_return: bool,
    pub no_free: bool,
    pub no_sync: bool,
}

/// `hasExactDefinition`: the body here is the one that runs.
fn exact(linkage: Linkage) -> bool {
    matches!(
        linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
}

fn calls(function: &Function) -> impl Iterator<Item = InstId> + '_ {
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        ))
}

/// `mayHaveRecursiveCallee` (FunctionAttrs.cpp:2030) with
/// `AnyFunctionsAddressIsTaken` true, its default.
fn may_have_recursive_callee(
    module: &Module,
    declarations: &Declarations,
    id: GlobalId,
    function: &Function,
) -> bool {
    calls(function).any(|inst| {
        // DEVIATE F2: an indirect call whose `!callees` lists its targets is as
        // recursive as they are; LLVM takes any indirect call to be.
        let Some(targets) = llrm_mir::callgraph::reached(module, function, inst) else { return true };
        targets
            .into_iter()
            .any(
                |callee| {
                    if callee == id {
                        return true;
                    }
                    let named = module.global(callee).intrinsic();
                    // DEVIATE F1: an intrinsic carries `norecurse` in LLVM's
                    // attribute tables; llrm's carry none, so the name says it.
                    if named.is_some_and(|one| !matches!(one, Intrinsic::Code | Intrinsic::Asm)) {
                        return false;
                    }
                    let Some(declared) = declarations.get(callee.0 as usize).and_then(|one| one.function()) else {
                        return true;
                    };
                    let facts = Facts::of(&declared.attrs);
                    !(facts.no_recurse() || (declared.is_declaration() && facts.no_callback()))
                },
            )
    })
}

/// `InstrBreaksNonThrowing` (FunctionAttrs.cpp:1873): the instruction may
/// throw, and is not a `call` to a member.
fn breaks_non_throwing(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
    members: &BTreeSet<GlobalId>,
) -> bool {
    match &function.instruction(inst).opcode {
        Opcode::Resume | Opcode::Invoke(_) => true,
        Opcode::Call(_) => {
            if effects::states(context, declarations, function, inst, Fact::NoUnwind) {
                return false;
            }
            !memory::callee(context, function, inst).is_some_and(|callee| members.contains(&callee))
        }
        _ => false,
    }
}

/// `InstrBreaksNoFree` (FunctionAttrs.cpp:1889): a call that does not state
/// `nofree`, and is not a call to a member. MIR has no atomics, so no other
/// instruction `maySynchronize`.
fn breaks_no_free(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
    members: &BTreeSet<GlobalId>,
) -> bool {
    if !matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    ) {
        return false;
    }
    !effects::states(context, declarations, function, inst, Fact::NoFree)
        && !memory::callee(context, function, inst).is_some_and(|callee| members.contains(&callee))
}

/// `InstrBreaksNoSync` (FunctionAttrs.cpp:1908): a call that does not state
/// `nosync` (`maySynchronize`), and is not a call to a member.
fn breaks_no_sync(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
    members: &BTreeSet<GlobalId>,
) -> bool {
    if !matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    ) {
        return false;
    }
    !effects::states(context, declarations, function, inst, Fact::NoSync)
        && !memory::callee(context, function, inst).is_some_and(|callee| members.contains(&callee))
}

/// `FindFunctionBackedges` is empty: a depth-first walk from the entry meets no
/// block still on its stack.
fn has_back_edge(function: &Function) -> bool {
    let Some(entry) = function.entry() else { return false };
    let mut state = vec![0_u8; function.layout().iter().map(|one| one.0 as usize + 1).max().unwrap_or(0)];
    let mut stack: Vec<(BlockId, Vec<BlockId>)> = vec![(entry, function.successors(entry).iter().copied().collect())];
    state[entry.0 as usize] = 1;
    while let Some((block, pending)) = stack.last_mut() {
        match pending.pop() {
            Some(next) => match state[next.0 as usize] {
                1 => return true,
                0 => {
                    state[next.0 as usize] = 1;
                    let following = function.successors(next).iter().copied().collect();
                    stack.push((next, following));
                }
                _ => {}
            },
            None => {
                state[block.0 as usize] = 2;
                stack.pop();
            }
        }
    }
    false
}

/// `functionWillReturn` (FunctionAttrs.cpp:2141).
fn function_will_return(
    context: &Context,
    declarations: &Declarations,
    linkage: Linkage,
    function: &Function,
) -> bool {
    if !exact(linkage) {
        return false;
    }
    if Facts::of(&function.attrs).must_progress() && !memory::stated(&function.attrs).writes {
        return true;
    }
    if function.is_declaration() || has_back_edge(function) {
        return false;
    }
    calls(function).all(|inst| effects::states(context, declarations, function, inst, Fact::WillReturn))
}

/// `canReturn` (CFG.cpp:405): some block reachable from the entry ends in
/// `ret` and holds no call that states `noreturn`.
fn can_return(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
) -> bool {
    let Some(entry) = function.entry() else { return true };
    let mut seen = BTreeSet::from([entry]);
    let mut work = vec![entry];
    while let Some(block) = work.pop() {
        let ends_in_ret =
            function.terminator(block).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret);
        if ends_in_ret
            && !function
                .iter_block(block)
                .any(|inst| effects::states(context, declarations, function, inst, Fact::NoReturn))
        {
            return true;
        }
        for next in function.successors(block).iter().copied() {
            if seen.insert(next) {
                work.push(next);
            }
        }
    }
    false
}

/// What each member of one SCC may be stamped with. `declarations` carry the
/// attributes of everything below it, and of the members as they stand.
pub fn infer(
    module: &Module,
    declarations: &Declarations,
    members: &[GlobalId],
) -> Vec<(GlobalId, Inferred)> {
    let context = &module.context;
    let set: BTreeSet<GlobalId> = members.iter().copied().collect();
    let body = |id: GlobalId| {
        let global = module.global(id);
        global.function().filter(|one| !one.is_declaration()).map(|one| (global.linkage, one))
    };
    // All or none, per attribute (`AttributeInferer`): one member that cannot
    // be scanned, or one instruction that breaks it, takes it from the SCC.
    let scc_states = |fact: Fact, breaks: &dyn Fn(&Function, InstId) -> bool| {
        members
            .iter()
            .all(
                |&id| {
                    let Some((linkage, function)) = body(id) else {
                        return module.global(id).function().is_some_and(|one| Facts::of(&one.attrs).contains(fact));
                    };
                    if Facts::of(&function.attrs).contains(fact) {
                        return true;
                    }
                    exact(linkage) && function.walk().all(|(_, inst)| !breaks(function, inst))
                },
            )
    };
    let unwinds =
        scc_states(Fact::NoUnwind, &|function, inst| breaks_non_throwing(context, declarations, function, inst, &set));
    let frees_nothing =
        scc_states(Fact::NoFree, &|function, inst| breaks_no_free(context, declarations, function, inst, &set));
    let syncs_nothing =
        scc_states(Fact::NoSync, &|function, inst| breaks_no_sync(context, declarations, function, inst, &set));
    let no_recurse_scc = members.len() == 1;
    members
        .iter()
        .map(|&id| {
            let Some((linkage, function)) = body(id) else { return (id, Inferred::default()) };
            let stamped = Facts::of(&function.attrs);
            let exact = exact(linkage);
            let inferred = Inferred {
                no_recurse: !stamped.no_recurse()
                    && exact
                    && no_recurse_scc
                    && !may_have_recursive_callee(module, declarations, id, function),
                no_unwind: !stamped.no_unwind() && unwinds,
                will_return: !stamped.will_return() && function_will_return(context, declarations, linkage, function),
                no_return: !stamped.no_return() && exact && !can_return(context, declarations, function),
                no_free: !stamped.contains(Fact::NoFree) && frees_nothing,
                no_sync: !stamped.contains(Fact::NoSync) && syncs_nothing,
            };
            (id, inferred)
        })
        .collect()
}

/// `addLocAccess` (FunctionAttrs.cpp:100,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L100>).
fn add_loc_access(
    me: &mut MemoryEffects,
    aa: &mut BasicAA,
    aaqi: &mut AAQueryInfo,
    loc: &MemoryLocation,
    info: ModRefInfo,
) {
    let info = info.intersect(aa.get_mod_ref_info_mask(loc, aaqi, true));
    if info.is_no_mod_ref() {
        return;
    }
    let object = aa.underlying_aggressive(loc.ptr);
    if aa.is_alloca(object) {
        return;
    }
    if aa.is_argument(object) {
        *me = me.union(MemoryEffects::arg_mem_only(info));
        return;
    }
    if !aa.is_identified_object(object) {
        *me = me.union(MemoryEffects::arg_mem_only(info));
    }
    *me = me
        .union(MemoryEffects::none().with(IRMemLocation::ErrnoMem, info))
        .union(MemoryEffects::none().with(IRMemLocation::Other, info));
}

/// `addArgLocs` (FunctionAttrs.cpp:123): each pointer argument of the call.
fn add_arg_locs(
    me: &mut MemoryEffects,
    aa: &mut BasicAA,
    aaqi: &mut AAQueryInfo,
    context: &Context,
    function: &Function,
    call: InstId,
    info: ModRefInfo,
) {
    let op = function.instruction(call);
    // `invoke args.., normal, unwind, callee`; `call args.., callee`.
    let tail = if matches!(op.opcode, Opcode::Invoke(_)) { 3 } else { 1 };
    for &arg in &op.operands[..op.operands.len() - tail] {
        let pointer = function
            .operand_type(context, arg)
            .is_some_and(|ty| matches!(context.types.get(ty), llrm_mir::types::Type::Pointer(_)));
        if pointer {
            add_loc_access(me, aa, aaqi, &MemoryLocation::before_or_after(arg), info);
        }
    }
}

/// `Instruction::getMemoryEffects` (Instruction.cpp:1085) for what is not a
/// call. MIR has no atomics, fences or `va_arg`: a load and a store only.
fn instruction_effects(opcode: &Opcode) -> MemoryEffects {
    let access = |info, volatile| {
        if volatile {
            MemoryEffects::arg_mem_only(ModRefInfo::ModRef)
                .union(MemoryEffects::none().with(IRMemLocation::InaccessibleMem, ModRefInfo::ModRef))
        } else {
            MemoryEffects::arg_mem_only(info)
        }
    };
    match opcode {
        Opcode::Load { volatile, .. } => access(ModRefInfo::Ref, *volatile),
        Opcode::Store { volatile, .. } => access(ModRefInfo::Mod, *volatile),
        _ => MemoryEffects::none(),
    }
}

/// `checkFunctionMemoryAccess` (FunctionAttrs.cpp:154): what the body of `id`
/// always does to memory, and what it additionally does if the SCC accesses
/// argument memory.
fn check_function_memory_access(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
    members: &BTreeSet<GlobalId>,
) -> (MemoryEffects, MemoryEffects) {
    let global = module.global(id);
    let function = global.function().expect("a procedure");
    let original = MemoryEffects::of_attrs(&function.attrs);
    if original.does_not_access_memory() || !exact(global.linkage) || function.is_declaration() {
        return (original, MemoryEffects::none());
    }
    let context = &module.context;
    let unit = Unit::of(module, layout, function);
    let mut aa = BasicAA::new(&unit);
    let aaqi = &mut AAQueryInfo::default();
    let mut me = MemoryEffects::none();
    let mut recursive = MemoryEffects::none();
    let non_argument = |me: &mut MemoryEffects, inst_me: MemoryEffects| {
        *me = me.union(inst_me.without_loc(IRMemLocation::ArgMem));
        *me = me.union(MemoryEffects::arg_mem_only(inst_me.mod_ref(IRMemLocation::Other)));
    };
    if function.parameter_attrs.iter().flatten().any(
        |attr| matches!(
            attr,
            llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) if name == "inalloca" || name == "preallocated"
        ),
    ) {
        me = me.union(MemoryEffects::arg_mem_only(ModRefInfo::ModRef));
    }
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        if matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            // DEVIATE B1: MIR has no operand bundles.
            if llrm_mir::memory::callee(context, function, inst).is_some_and(|one| members.contains(&one)) {
                add_arg_locs(&mut recursive, &mut aa, aaqi, context, function, inst, ModRefInfo::ModRef);
                continue;
            }
            let call_me = aa.get_memory_effects(inst, aaqi);
            if call_me.does_not_access_memory() {
                continue;
            }
            non_argument(&mut me, call_me);
            let argument = call_me.mod_ref(IRMemLocation::ArgMem);
            if argument != ModRefInfo::NoModRef {
                add_arg_locs(&mut me, &mut aa, aaqi, context, function, inst, argument);
            }
            continue;
        }
        let inst_me = instruction_effects(&op.opcode);
        if inst_me.does_not_access_memory() {
            continue;
        }
        let Some(loc) = MemoryLocation::get(&unit, inst) else {
            me = me.union(MemoryEffects::everywhere(inst_me.overall()));
            continue;
        };
        non_argument(&mut me, inst_me);
        add_loc_access(&mut me, &mut aa, aaqi, &loc, inst_me.mod_ref(IRMemLocation::ArgMem));
    }
    (original.intersect(me), recursive)
}

/// `addMemoryAttrs` (FunctionAttrs.cpp:273,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L273>):
/// the new `memory(...)` of each member of the SCC whose attribute narrows.
pub fn infer_memory(
    module: &Module,
    members: &[GlobalId],
) -> Vec<(GlobalId, MemoryEffects)> {
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(Default::default, |text| llrm_mir::datalayout::DataLayout::parse(text).unwrap_or_default());
    // DEVIATE F3: a `noipa` body is not a member.
    let set: BTreeSet<GlobalId> = members
        .iter()
        .copied()
        .filter(|&id| module.global(id).function().is_some_and(|one| !llrm_mir::facts::opaque(one)))
        .collect();
    let mut me = MemoryEffects::none();
    let mut recursive = MemoryEffects::none();
    for &id in &set {
        let (always, argument) = check_function_memory_access(module, &layout, id, &set);
        me = me.union(always);
        recursive = recursive.union(argument);
        if me == MemoryEffects::unknown() {
            break;
        }
    }
    let argument = me.mod_ref(IRMemLocation::ArgMem);
    if argument != ModRefInfo::NoModRef {
        me = me.union(recursive.intersect(MemoryEffects::everywhere(argument)));
    }
    set.into_iter()
        .filter_map(|id| {
            let old = MemoryEffects::of_attrs(&module.global(id).function()?.attrs);
            let new = me.intersect(old);
            (new != old).then_some((id, new))
        })
        .collect()
}

/// `infer_memory` stated on the members (`F->setMemoryEffects`): the
/// `memory(...)` attribute replaces `readnone`, `readonly` and `writeonly`.
/// The members whose attributes changed.
pub fn stamp_memory(
    module: &mut Module,
    members: &[GlobalId],
) -> Vec<GlobalId> {
    let found = infer_memory(module, members);
    let mut changed = Vec::new();
    for (id, effects) in found {
        let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else {
            continue;
        };
        function
            .attrs
            .retain(
                |one| match one {
                    Attribute::Memory(_) => false,
                    other => !matches!(
                        Fact::of_attribute(&other),
                        Some(Fact::ReadNone | Fact::ReadOnly | Fact::WriteOnly)
                    ),
                },
            );
        function.attrs.push(effects.attribute());
        changed.push(id);
    }
    changed
}

/// DEVIATE F1: `willreturn` also where a loop's trip count induction proves,
/// or the language promises each loop ends (`mustprogress` on the function,
/// `llvm.loop.mustprogress` on each loop) of a function that does nothing
/// observable; LLVM's rule is a body without back edges. The calls must state
/// `willreturn` themselves. `norecurse` is as inferred or already stated.
fn will_return_by_loops(
    module: &Module,
    layout: &DataLayout,
    spaces: Spaces,
    function: &Function,
    norecurse: bool,
) -> bool {
    let context = &module.context;
    let calls_state = || {
        function
            .walk()
            .map(|(_, inst)| inst)
            .all(
                |inst| !matches!(
                    function.instruction(inst).opcode,
                    Opcode::Call(_) | Opcode::Invoke(_)
                )
                    || effects::states(context, &module.globals, function, inst, Fact::WillReturn),
            )
    };
    let unit = || Unit::of(module, layout, function).with_spaces(spaces);
    let counted = || {
        let shape = Shape::of(function);
        // Found once for the function, not for each loop: the proofs of every
        // loop ask the same.
        let registers = crate::consts::known(&unit());
        shape
            .loops
            .iter()
            .all(
                |one| crate::induction::counted(
                    &unit().with_registers(&registers).with_shape(&shape),
                    one,
                    Some(&registers),
                    false,
                )
                .iter()
                .any(|proof| {
                    !proof.stops && (proof.count.is_some() || proof.step.magnitude() == crate::num::Num::from(1_u8))
                }),
            )
    };
    let calls_state = calls_state();
    if calls_state && counted() {
        return true;
    }
    if !norecurse || !calls_state {
        return false;
    }
    let volatile = function
        .walk()
        .any(
            |(_, inst)| matches!(
                function.instruction(inst).opcode,
                Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. }
            ),
        );
    let callees = memory::callees(module);
    let unobserved = !volatile
        && function.walk().map(|(_, inst)| inst).all(|inst| {
            !matches!(
                function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            )
                || !memory::of(context, &callees, function, inst).writes
        });
    unobserved
        && llrm_mir::loopinfo::ends_by_promise(&module.metadata, function, Facts::of(&function.attrs).must_progress())
}

/// `infer`'s flags stated on the members in `deriveAttrsInPostOrder`'s order
/// (`noreturn`, `willreturn`, `nounwind`, `norecurse`), `willreturn` as F1 has
/// it. The members whose attributes changed.
fn stamp_flags(
    module: &mut Module,
    layout: &DataLayout,
    spaces: Spaces,
    members: &[GlobalId],
) -> Vec<GlobalId> {
    let mut found = infer(module, &module.globals, members);
    for (id, one) in &mut found {
        let Some(function) = module.global(*id).function().filter(|one| !one.is_declaration()) else { continue };
        let norecurse = one.no_recurse || Facts::of(&function.attrs).no_recurse();
        if !one.will_return && !Facts::of(&function.attrs).will_return() {
            one.will_return = will_return_by_loops(module, layout, spaces, function, norecurse);
        }
    }
    let mut changed = Vec::new();
    for (id, one) in found {
        let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else {
            continue;
        };
        let before = function.attrs.len();
        for (stated, fact) in [
            (one.no_return, Fact::NoReturn),
            (one.will_return, Fact::WillReturn),
            (one.no_unwind, Fact::NoUnwind),
            (one.no_free, Fact::NoFree),
            (one.no_sync, Fact::NoSync),
            (one.no_recurse, Fact::NoRecurse),
        ] {
            if stated && !Facts::of(&function.attrs).contains(fact) {
                function.attrs.extend(fact.attribute());
            }
        }
        if function.attrs.len() != before {
            changed.push(id);
        }
    }
    changed
}

/// `post_order` over every component of `module`, callees first: the
/// attributes the pipeline has stated when a pass that reads them runs.
pub fn infer_module(
    module: &mut Module,
    layout: &DataLayout,
    spaces: Spaces,
) {
    let components = llrm_mir::callgraph::CallGraph::new(module).bottom_up_components();
    for (members, _) in components {
        post_order(module, layout, spaces, &members);
    }
}

/// `deriveAttrsInPostOrder` (FunctionAttrs.cpp:2211,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L2211>)
/// for what is translated, in its order: `addMemoryAttrs`, `addArgumentAttrs`,
/// then the flags. The members whose attributes changed.
pub fn post_order(
    module: &mut Module,
    layout: &DataLayout,
    spaces: Spaces,
    members: &[GlobalId],
) -> Vec<GlobalId> {
    // DEVIATE F3: a `noipa` body is not a member.
    let set: BTreeSet<GlobalId> = members
        .iter()
        .copied()
        .filter(|&id| module.global(id).function().is_some_and(|one| !llrm_mir::facts::opaque(one)))
        .collect();
    let list: Vec<GlobalId> = set.iter().copied().collect();
    let mut changed: BTreeSet<GlobalId> = stamp_memory(module, &list).into_iter().collect();
    changed.extend(crate::argumentattrs::add_argument_attrs(module, &set));
    changed.extend(crate::initializes::add_initializes(module, layout, &set));
    changed.extend(stamp_flags(module, layout, spaces, &list));
    changed.into_iter().collect()
}

#[cfg(test)]
#[path = "functionattrs_tests.rs"]
mod tests;
