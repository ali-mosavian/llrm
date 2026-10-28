//! `nocapture`, `readonly`, `writeonly` and `readnone` of the pointer
//! parameters of one call-graph SCC, as `addArgumentAttrs` infers them
//! (FunctionAttrs.cpp:1238).
//!
//! # How it works
//! A parameter's uses are walked by capture tracking
//! ([`crate::capturetracking`]) with a tracker that does not count a call to
//! a member of the SCC as a capture: it records the callee's parameter
//! instead, as an edge of the argument graph. A parameter with no such edge
//! is settled at once: `nocapture` if nothing keeps it. The rest are the
//! graph's strongly connected components, callees first: an SCC of arguments
//! keeps what the union of its members and of what they pass on to outside
//! the SCC keeps. `readonly`, `writeonly` or `readnone` come from the uses
//! that can still reach the pointer's memory (`determinePointerAccessAttrs`):
//! loads, stores, and calls by what their `memory(argmem: ...)` and the
//! parameter's attributes allow, with a pass to a member of the SCC assumed
//! harmless.
//!
//! A parameter no use can free is `nofree` too (`IsFree`): a write through it
//! by a call whose site, callee and parameter do not say `nofree` may free it.
//!
//! A parameter the entry block's guaranteed calls pass on to a `nonnull`
//! `noundef` parameter of their callee is `nonnull`
//! (`addArgumentAttrsFromCallsites`): the call runs on every entry, so the
//! value is non-null throughout. DEVIATE A4: `nofpclass` has no MIR spelling,
//! so only `nonnull` is propagated.
//!
//! DEVIATE A1: `nofree_obj` has no MIR spelling, so only `nofree` is looked
//! for. DEVIATE A2: only `captures(none)` can be stated (see
//! [`crate::capturetracking`]), so a parameter that would be
//! `captures(ret: ...)` stays unstated. Not translated yet:
//! `returned`; `inferInitializes` is
//! [`crate::initializes`].
//!
//! # Example
//! `get` reads through `%p` and never keeps it; `put` writes through it:
//!
//! ```mir
//! define i32 @get(ptr %p) {
//! b0:
//!   %v = load i32, ptr %p
//!   ret i32 %v
//! }
//! ```
//!
//! `%p` is `nocapture readonly`. From unit test
//! `a_load_only_parameter_is_nocapture_readonly` in `argumentattrs_tests.rs`.
//!
//! # Runs
//! `functionattrs::post_order` once per SCC, callees first.
//!
//! # See also
//! LLVM `FunctionAttrs.cpp` (`addArgumentAttrs`, `addAccessAttrs`,
//! `determinePointerAccessAttrs`, `ArgumentUsesTracker`),
//! `CaptureTracking.cpp`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::module::{Function, InstId, Linkage, Module, Operand, Use};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::types::Type;
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::is_guaranteed_to_transfer_execution_to_successor;
use llrm_mir::valuetracking::nonzero::null_pointer_is_defined;
use llrm_support::hash::HashSet;

use crate::aa::{IRMemLocation, MemoryEffects};
use crate::capturetracking::{
    Action, CaptureComponents, CaptureTracker, DEFAULT_MAX_USES_TO_EXPLORE, UseCaptureInfo, determine_use_capture_kind,
    pointer_may_be_captured_by,
};

/// One parameter of one function.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Arg {
    function: GlobalId,
    index: usize,
}

/// `CaptureInfo`: what a use keeps outside a `ret` and through one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaptureInfo {
    other: CaptureComponents,
    ret: CaptureComponents,
}

impl CaptureInfo {
    const NONE: Self = Self { other: CaptureComponents::NONE, ret: CaptureComponents::NONE };
    const ALL: Self = Self { other: CaptureComponents::ALL, ret: CaptureComponents::ALL };

    fn of(cc: CaptureComponents) -> Self {
        Self { other: cc, ret: cc }
    }

    fn ret_only(cc: CaptureComponents) -> Self {
        Self { other: CaptureComponents::NONE, ret: cc }
    }

    fn union(
        self,
        other: Self,
    ) -> Self {
        Self { other: self.other.union(other.other), ret: self.ret.union(other.ret) }
    }

    fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self { other: self.other.intersect(other.other), ret: self.ret.intersect(other.ret) }
    }
}

/// `hasExactDefinition`.
pub(crate) fn exact_definition(
    module: &Module,
    id: GlobalId,
) -> bool {
    let global = module.global(id);
    matches!(
        global.linkage,
        Linkage::External | Linkage::Internal | Linkage::Private
    )
        && global.function().is_some_and(|one| !one.is_declaration())
}

fn parameter<'a>(
    module: &'a Module,
    arg: Arg,
) -> &'a [Attribute] {
    module.global(arg.function).function().map_or(&[], |one| one.parameter_attrs[arg.index].as_slice())
}

fn is_pointer(
    module: &Module,
    function: &Function,
    index: usize,
) -> bool {
    matches!(
        module.context.types.get(function.value(function.parameters()[index]).ty),
        Type::Pointer(_)
    )
}

/// The captures `attrs` state: nothing for `nocapture`, else everything.
fn stated_capture(attrs: &[Attribute]) -> CaptureInfo {
    if Facts::of(attrs).no_capture() { CaptureInfo::NONE } else { CaptureInfo::ALL }
}

/// `ArgumentUsesTracker` (FunctionAttrs.cpp:520).
struct ArgumentUsesTracker<'a> {
    module: &'a Module,
    function: &'a Function,
    members: &'a BTreeSet<GlobalId>,
    ci: CaptureInfo,
    uses: Vec<Arg>,
}

impl ArgumentUsesTracker<'_> {
    /// `updateCaptureInfo`: whether the capture is final, else the use is an
    /// edge into the SCC.
    fn update_capture_info(
        &mut self,
        used: Use,
        cc: CaptureComponents,
    ) -> bool {
        let op = self.function.instruction(used.user);
        if !matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
            self.ci = self
                .ci
                .union(
                    if matches!(op.opcode, Opcode::Ret) {
                        CaptureInfo::ret_only(cc)
                    } else {
                        CaptureInfo::of(cc)
                    },
                );
            return true;
        }
        let callee = llrm_mir::memory::callee(&self.module.context, self.function, used.user)
            .filter(|&one| exact_definition(self.module, one) && self.members.contains(&one));
        let Some(callee) = callee else {
            self.ci = self.ci.union(CaptureInfo::of(cc));
            return true;
        };
        let index = used.index as usize;
        let parameters = self.module.global(callee).function().map_or(0, |one| one.parameters().len());
        if index >= parameters {
            // A variadic argument.
            self.ci = self.ci.union(CaptureInfo::of(cc));
            return true;
        }
        self.uses.push(Arg { function: callee, index });
        false
    }
}

impl CaptureTracker for ArgumentUsesTracker<'_> {
    fn too_many_uses(&mut self) {
        self.ci = CaptureInfo::ALL;
    }

    fn captured(
        &mut self,
        used: Use,
        info: UseCaptureInfo,
    ) -> Action {
        if self.update_capture_info(used, info.use_cc) {
            if self.ci.other == CaptureComponents::ALL { Action::Stop } else { Action::Continue }
        } else {
            // Not following the return value of a call into the SCC.
            Action::ContinueIgnoringReturn
        }
    }
}

/// `ArgAccessProperties`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Access {
    read: bool,
    write: bool,
    free: bool,
}

impl Access {
    const ALL: Self = Self { read: true, write: true, free: true };

    fn has_all(self) -> bool {
        self == Self::ALL
    }

    fn union(
        self,
        other: Self,
    ) -> Self {
        Self { read: self.read || other.read, write: self.write || other.write, free: self.free || other.free }
    }
}

/// `CallBase::paramHasAttr`: the site's argument attribute or the direct
/// callee's parameter attribute.
fn call_param_has(
    module: &Module,
    function: &Function,
    call: InstId,
    index: usize,
    fact: fn(&Facts) -> bool,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else { return false };
    let site = info.argument_attrs.get(index).is_some_and(|attrs| fact(&Facts::of(attrs)));
    site || llrm_mir::memory::callee(&module.context, function, call)
        .and_then(|one| module.global(one).function())
        .and_then(|one| one.parameter_attrs.get(index))
        .is_some_and(|attrs| fact(&Facts::of(attrs)))
}

/// `CallBase::hasFnAttr(NoFree)`: the site's attributes or the direct callee's.
fn call_has_nofree(
    module: &Module,
    function: &Function,
    call: InstId,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else { return false };
    Facts::of(&info.attrs).no_free()
        || llrm_mir::memory::callee(&module.context, function, call)
            .and_then(|one| module.global(one).function())
            .is_some_and(|one| Facts::of(&one.attrs).no_free())
}

/// `CallBase::getMemoryEffects`: the site's attributes met with the direct
/// callee's.
fn call_effects(
    module: &Module,
    function: &Function,
    call: InstId,
) -> MemoryEffects {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
        return MemoryEffects::unknown();
    };
    let site = MemoryEffects::of_attrs(&info.attrs);
    llrm_mir::memory::callee(&module.context, function, call)
        .and_then(|one| module.global(one).function())
        .map_or(site, |one| site.intersect(MemoryEffects::of_attrs(&one.attrs)))
}

/// `CallBase::onlyReadsMemory(OpNo)`.
fn only_reads_operand(
    module: &Module,
    function: &Function,
    call: InstId,
    index: usize,
) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else { return false };
    let by_value =
        |attrs: &[Attribute]| attrs.iter().any(|one| matches!(one, Attribute::Type(name, _) if name == "byval"));
    info.argument_attrs.get(index).is_some_and(|one| by_value(one))
        || call_param_has(module, function, call, index, Facts::read_only)
        || call_param_has(module, function, call, index, Facts::read_none)
}

/// `CallBase::onlyWritesMemory(OpNo)`.
fn only_writes_operand(
    module: &Module,
    function: &Function,
    call: InstId,
    index: usize,
) -> bool {
    call_param_has(module, function, call, index, Facts::write_only)
        || call_param_has(module, function, call, index, Facts::read_none)
}

/// `determinePointerAccessAttrs` (FunctionAttrs.cpp:932,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L932>).
fn determine_pointer_access_attrs(
    module: &Module,
    arg: Arg,
    members: &BTreeSet<Arg>,
) -> Access {
    let function = module.global(arg.function).function().expect("a procedure");
    let attrs = &function.parameter_attrs[arg.index];
    // inalloca arguments are always clobbered by the call.
    if attrs.iter().any(|one| {
        matches!(
            one,
            Attribute::Flag(name) | Attribute::Type(name, _) if name == "inalloca" || name == "preallocated"
        )
    }) {
        return Access::ALL;
    }
    let value = function.parameters()[arg.index];
    let mut access = Access::default();
    let mut visited: HashSet<(InstId, u32)> = HashSet::default();
    let mut work: Vec<Use> = Vec::new();
    for &used in function.users(value) {
        visited.insert((used.user, used.index));
        work.push(used);
    }
    while let Some(used) = work.pop() {
        if access.has_all() {
            return access;
        }
        let op = function.instruction(used.user);
        if matches!(op.opcode, Opcode::Ret) {
            continue;
        }
        let mut info = determine_use_capture_kind(function, &module.globals, &module.context, used, value);
        let call = matches!(op.opcode, Opcode::Call(_) | Opcode::Invoke(_));
        if call && call_effects(module, function, used.user).only_reads_memory() {
            info.use_cc = info.use_cc.intersect(CaptureComponents::ADDRESS);
        }
        if info.use_cc.intersect(CaptureComponents::PROVENANCE).captures_anything() {
            // Indirect access through captured provenance.
            if info.use_cc.intersect(CaptureComponents::PROVENANCE) != CaptureComponents::READ_PROVENANCE {
                return Access::ALL;
            }
            access.read = true;
        }
        if info.result_cc.intersect(CaptureComponents::PROVENANCE).captures_anything()
            && let Some(result) = op.result
        {
            for &next in function.users(result) {
                if visited.insert((next.user, next.index)) {
                    work.push(next);
                }
            }
        }
        if call {
            let index = used.index as usize;
            if index + 1 == op.operands.len() {
                // The callee operand.
                access.read = true;
                continue;
            }
            let argument = call_effects(module, function, used.user).mod_ref(IRMemLocation::ArgMem);
            if argument.is_no_mod_ref() {
                continue;
            }
            // A pass to a parameter of the SCC under speculation.
            let callee = llrm_mir::memory::callee(&module.context, function, used.user);
            if let Some(callee) = callee
                && module.global(callee).function().is_some_and(|one| index < one.parameters().len())
                && members.contains(&Arg { function: callee, index })
            {
                continue;
            }
            if argument.is_ref_set() && !only_writes_operand(module, function, used.user, index) {
                access.read = true;
            }
            if argument.is_mod_set() && !only_reads_operand(module, function, used.user, index) {
                access.write = true;
                if !call_has_nofree(module, function, used.user)
                    && !call_param_has(module, function, used.user, index, Facts::no_free)
                {
                    access.free = true;
                }
            }
        } else {
            // The value operand of a store is not a use of its memory.
            if matches!(op.opcode, Opcode::Store { .. }) && used.index == 0 {
                continue;
            }
            let (reads, writes) = match op.opcode {
                Opcode::Load { volatile, .. } => (true, volatile),
                Opcode::Store { volatile, .. } => (volatile, true),
                _ => (false, false),
            };
            access.read |= reads;
            access.write |= writes;
        }
    }
    access
}

/// `Argument::hasNonNullAttr(AllowUndefOrPoison)`.
fn has_non_null_attr(
    module: &Module,
    function: &Function,
    index: usize,
    allow_undef_or_poison: bool,
) -> bool {
    let attrs = &function.parameter_attrs[index];
    let facts = Facts::of(attrs);
    if facts.non_null() && (allow_undef_or_poison || facts.no_undef()) {
        return true;
    }
    let dereferenceable = attrs
        .iter()
        .any(
            |one| matches!(
                one,
                Attribute::Int(name, bytes) if name == "dereferenceable" && *bytes > 0
            ),
        );
    let space = match module.context.types.get(function.value(function.parameters()[index]).ty) {
        Type::Pointer(space) => *space,
        _ => return false,
    };
    dereferenceable && !null_pointer_is_defined(Some(function), space)
}

/// `addArgumentAttrsFromCallsites` (FunctionAttrs.cpp:1022): the parameters of
/// `id` that a call of its entry block, reached on every call of it, passes to
/// a `nonnull noundef` parameter of the callee.
fn non_null_from_callsites(
    module: &Module,
    id: GlobalId,
) -> Vec<usize> {
    let function = module.global(id).function().expect("a procedure");
    let Some(entry) = function.entry() else { return Vec::new() };
    let layout = DataLayout::default();
    let q = Query::new(module, function, &layout);
    let mut found = Vec::new();
    for inst in function.iter_block(entry) {
        let instruction = function.instruction(inst);
        if matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_))
            && let Some(callee) =
                llrm_mir::memory::callee(&module.context, function, inst).and_then(|one| module.global(one).function())
        {
            for index in 0..callee.parameters().len().min(instruction.operands.len().saturating_sub(1)) {
                let Operand::Value(value) = instruction.operands[index] else { continue };
                let Some(ours) = function.parameters().iter().position(|&one| one == value) else { continue };
                if has_non_null_attr(module, callee, index, false)
                    && !has_non_null_attr(module, function, ours, true)
                    && !found.contains(&ours)
                {
                    found.push(ours);
                }
            }
        }
        if !is_guaranteed_to_transfer_execution_to_successor(&q, inst) {
            break;
        }
    }
    found
}

/// `addAccessAttrs` (FunctionAttrs.cpp:1074): whether the parameter changed.
fn add_access_attrs(
    module: &mut Module,
    arg: Arg,
    access: Access,
) -> bool {
    let mut changed = false;
    if !access.free && !Facts::of(parameter_mut(module, arg)).no_free() {
        parameter_mut(module, arg).extend(Fact::NoFree.attribute());
        changed = true;
    }
    if access.read && access.write {
        return changed;
    }
    let want = if access.read {
        Fact::ReadOnly
    } else if access.write {
        Fact::WriteOnly
    } else {
        Fact::ReadNone
    };
    let attrs = parameter_mut(module, arg);
    if attrs.iter().any(|one| Fact::of_attribute(one) == Some(want)) {
        return false;
    }
    attrs.retain(|one| !matches!(
        Fact::of_attribute(one),
        Some(Fact::ReadOnly | Fact::WriteOnly | Fact::ReadNone)
    ));
    attrs.extend(want.attribute());
    true
}

fn parameter_mut(
    module: &mut Module,
    arg: Arg,
) -> &mut Vec<Attribute> {
    match &mut module.globals[arg.function.0 as usize].kind {
        llrm_mir::module::GlobalKind::Function(function) => &mut function.parameter_attrs[arg.index],
        llrm_mir::module::GlobalKind::Variable(_) => unreachable!("a procedure is a function"),
    }
}

/// `DetermineAccessAttrsForSingleton`.
fn access_for_singleton(
    module: &mut Module,
    arg: Arg,
) -> bool {
    let access = determine_pointer_access_attrs(module, arg, &BTreeSet::from([arg]));
    add_access_attrs(module, arg, access)
}

/// Adds `captures(none)` for what `NewCI` is, if MIR can state it (DEVIATE A2).
fn state_capture(
    module: &mut Module,
    arg: Arg,
    new: CaptureInfo,
    orig: CaptureInfo,
) -> bool {
    if new == orig || new != CaptureInfo::NONE {
        return false;
    }
    parameter_mut(module, arg).extend(Fact::NoCapture.attribute());
    true
}

/// The argument graph: node numbers in creation order, edges to the callee
/// parameters a member passes its pointer to.
#[derive(Default)]
struct ArgumentGraph {
    index: BTreeMap<Arg, usize>,
    nodes: Vec<(Arg, CaptureComponents, Vec<usize>)>,
}

impl ArgumentGraph {
    fn node(
        &mut self,
        arg: Arg,
    ) -> usize {
        *self
            .index
            .entry(arg)
            .or_insert_with(
                || {
                    self.nodes.push((arg, CaptureComponents::NONE, Vec::new()));
                    self.nodes.len() - 1
                },
            )
    }

    /// The strongly connected components, a component after those it points
    /// to (`scc_iterator`'s order). Tarjan's algorithm, O(nodes + edges).
    fn components(&self) -> Vec<Vec<usize>> {
        struct Walk<'a> {
            graph: &'a ArgumentGraph,
            number: Vec<usize>,
            low: Vec<usize>,
            on_stack: Vec<bool>,
            stack: Vec<usize>,
            next: usize,
            out: Vec<Vec<usize>>,
        }
        impl Walk<'_> {
            fn visit(
                &mut self,
                node: usize,
            ) {
                self.next += 1;
                self.number[node] = self.next;
                self.low[node] = self.next;
                self.stack.push(node);
                self.on_stack[node] = true;
                for position in 0..self.graph.nodes[node].2.len() {
                    let to = self.graph.nodes[node].2[position];
                    if self.number[to] == 0 {
                        self.visit(to);
                        self.low[node] = self.low[node].min(self.low[to]);
                    } else if self.on_stack[to] {
                        self.low[node] = self.low[node].min(self.number[to]);
                    }
                }
                if self.low[node] == self.number[node] {
                    let mut component = Vec::new();
                    loop {
                        let member = self.stack.pop().expect("a stacked node");
                        self.on_stack[member] = false;
                        component.push(member);
                        if member == node {
                            break;
                        }
                    }
                    component.reverse();
                    self.out.push(component);
                }
            }
        }
        let count = self.nodes.len();
        let mut walk = Walk {
            graph: self,
            number: vec![0; count],
            low: vec![0; count],
            on_stack: vec![false; count],
            stack: Vec::new(),
            next: 0,
            out: Vec::new(),
        };
        for node in 0..count {
            if walk.number[node] == 0 {
                walk.visit(node);
            }
        }
        walk.out
    }
}

fn only_reads_memory(attrs: &[Attribute]) -> bool {
    let facts = Facts::of(attrs);
    facts.read_only() || facts.read_none()
}

/// `addArgumentAttrs` (FunctionAttrs.cpp:1238,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L1238>)
/// with `SkipInitializes`: the members whose parameter attributes changed.
pub fn add_argument_attrs(
    module: &mut Module,
    members: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    let mut graph = ArgumentGraph::default();
    for &id in members {
        if !exact_definition(module, id) {
            continue;
        }
        let from_callsites = non_null_from_callsites(module, id);
        if !from_callsites.is_empty() {
            for &index in &from_callsites {
                parameter_mut(module, Arg { function: id, index }).extend(Fact::NonNull.attribute());
            }
            changed.insert(id);
        }
        let function = module.global(id).function().expect("a procedure");
        let count = function.parameters().len();
        let pointers: Vec<bool> = (0..count).map(|index| is_pointer(module, function, index)).collect();
        let effects = MemoryEffects::of_attrs(&function.attrs);
        let facts = Facts::of(&function.attrs);
        let returns_void = matches!(
            module.context.types.get(function.ty),
            Type::Function { returns, .. } if matches!(module.context.types.get(*returns), Type::Void)
        );
        // Readonly, nounwind, willreturn, returning nothing: cannot keep a
        // pointer.
        if effects.only_reads_memory() && facts.no_unwind() && facts.will_return() && returns_void {
            for index in (0..count).filter(|&index| pointers[index]) {
                let arg = Arg { function: id, index };
                if !Facts::of(parameter(module, arg)).no_capture() {
                    parameter_mut(module, arg).extend(Fact::NoCapture.attribute());
                    changed.insert(id);
                }
            }
            continue;
        }
        for index in (0..count).filter(|&index| pointers[index]) {
            let arg = Arg { function: id, index };
            let mut non_local_uses = false;
            let orig = stated_capture(parameter(module, arg));
            if orig != CaptureInfo::NONE {
                let function = module.global(id).function().expect("a procedure");
                let value = function.parameters()[index];
                let mut tracker =
                    ArgumentUsesTracker { module, function, members, ci: CaptureInfo::NONE, uses: Vec::new() };
                pointer_may_be_captured_by(
                    function,
                    &module.globals,
                    &module.context,
                    value,
                    &mut tracker,
                    DEFAULT_MAX_USES_TO_EXPLORE,
                );
                let (new, uses) = (tracker.ci.intersect(orig), tracker.uses);
                if new != orig {
                    if uses.is_empty() {
                        // The information is complete.
                        if state_capture(module, arg, new, orig) {
                            changed.insert(id);
                        }
                    } else {
                        // Calls into the SCC: left to the argument graph.
                        let node = graph.node(arg);
                        graph.nodes[node].1 = new.other;
                        for used in uses {
                            let to = graph.node(used);
                            graph.nodes[node].2.push(to);
                            if used != arg {
                                non_local_uses = true;
                            }
                        }
                    }
                }
            }
            if !non_local_uses && !only_reads_memory(parameter(module, arg)) && access_for_singleton(module, arg) {
                changed.insert(id);
            }
        }
    }
    for component in graph.components() {
        if component.len() == 1 {
            let node = component[0];
            let (arg, cc, edges) = (graph.nodes[node].0, graph.nodes[node].1, graph.nodes[node].2.clone());
            // `void f(int *x) { if (...) f(x); f(x); }`. DEVIATE A3: LLVM
            // asks for exactly one edge, so a parameter passed to itself by
            // two calls is left bare (pinned opt, `walk` in
            // `a_parameter_passed_to_itself_twice_is_stated`).
            if !edges.is_empty() && edges.iter().all(|&to| to == node) {
                let orig = stated_capture(parameter(module, arg));
                if state_capture(module, arg, CaptureInfo::of(cc).intersect(orig), orig) {
                    changed.insert(arg.function);
                }
                if access_for_singleton(module, arg) {
                    changed.insert(arg.function);
                }
            }
            continue;
        }
        let args: BTreeSet<Arg> = component.iter().map(|&node| graph.nodes[node].0).collect();
        // At the SCC level only the merged components are tracked.
        let mut cc = CaptureComponents::NONE;
        for &node in &component {
            if let Some(&first) = graph.nodes[node].2.first() {
                let (to, to_cc) = (graph.nodes[first].0, graph.nodes[first].1);
                cc = cc.union(if args.contains(&to) { to_cc } else { stated_capture(parameter(module, to)).other });
            }
            if cc == CaptureComponents::ALL {
                break;
            }
        }
        if cc != CaptureComponents::ALL {
            for &node in &component {
                let (arg, own) = (graph.nodes[node].0, graph.nodes[node].1);
                let orig = stated_capture(parameter(module, arg));
                if state_capture(module, arg, CaptureInfo::of(own.union(cc)).intersect(orig), orig) {
                    changed.insert(arg.function);
                }
            }
        }
        if cc.intersect(CaptureComponents::PROVENANCE).captures_anything() {
            // The provenance may be captured: each parameter on its own.
            for &node in &component {
                let arg = graph.nodes[node].0;
                if access_for_singleton(module, arg) {
                    changed.insert(arg.function);
                }
            }
            continue;
        }
        let mut access = Access::default();
        for &node in &component {
            access = access.union(determine_pointer_access_attrs(module, graph.nodes[node].0, &args));
            if access.has_all() {
                break;
            }
        }
        if !access.has_all() {
            for &node in &component {
                let arg = graph.nodes[node].0;
                if add_access_attrs(module, arg, access) {
                    changed.insert(arg.function);
                }
            }
        }
    }
    changed
}

#[cfg(test)]
#[path = "argumentattrs_tests.rs"]
mod tests;
