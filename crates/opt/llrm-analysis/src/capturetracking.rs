//! `CaptureTracking.cpp` at d1106deb71cc: which components of a pointer a use
//! of it keeps (`DetermineUseCaptureKind`), and the walk over the uses of a
//! value that asks a tracker about each (`PointerMayBeCaptured`).
//!
//! # How it works
//! The walk starts at a value's uses. Each use is classified by its user: a
//! load or a store's address captures nothing, a store of the pointer, a
//! `ptrtoint`, a `ret` or an unknown user captures it all, a `gep`, cast,
//! `phi` or `select` passes it on to its result (the result's uses are then
//! walked), a call keeps what its argument and its callee's parameter
//! attributes allow. A use that keeps something is handed to the
//! [`CaptureTracker`], whose answer says whether to stop, go on, or go on
//! without following the user's result. Past 150 uses (`capture-tracking-max-
//! uses-to-explore`) the tracker is told `too_many_uses`.
//!
//! DEVIATE C1: MIR has no `captures(...)` attribute: `nocapture` is
//! `captures(none)`, anything else is all components. DEVIATE C2: no
//! pointer-returning intrinsics (`launder.invariant.group`, `ptrmask`,
//! `threadlocal.address`), atomics, `va_arg`, `ptrtoaddr`, volatile memory
//! intrinsics or `!captures` metadata exist in MIR, so those cases are absent.
//! `PointerMayBeCapturedBefore` is not translated; `FindEarliestCapture` is
//! ([`find_earliest_capture`]), and [`crate::captureanalysis`] answers
//! `getCapturesBefore` with it.
//!
//! # Example
//! `%p` is stored to memory, so it is captured; `%q` is only loaded through:
//!
//! ```mir
//! define void @f(ptr %p, ptr %q, ptr %slot) {
//! b0:
//!   store ptr %p, ptr %slot
//!   %v = load i32, ptr %q
//!   ret void
//! }
//! ```
//!
//! From unit test
//! `a_stored_pointer_is_captured_and_a_loaded_through_one_is_not` in
//! `capturetracking.rs`.
//!
//! # Provides
//! [`CaptureComponents`], [`UseCaptureInfo`], [`CaptureTracker`],
//! [`determine_use_capture_kind`], [`pointer_may_be_captured_by`],
//! [`pointer_may_be_captured`], [`find_earliest_capture`].
//!
//! # See also
//! LLVM `CaptureTracking.cpp`, `Support/ModRef.h` (`CaptureComponents`).

use llrm_mir::context::ConstantKind;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{Function, GlobalValue, InstId, Operand, Use, ValueId};
use llrm_mir::opcode::{CastOp, IntPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_mir::{Context, facts::Facts};
use llrm_support::hash::HashSet;

use crate::aa::MemoryEffects;

/// `capture-tracking-max-uses-to-explore`.
pub const DEFAULT_MAX_USES_TO_EXPLORE: usize = 150;

/// `CaptureComponents`: a bit set, `Address` including `AddressIsNull` and
/// `Provenance` including `ReadProvenance`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CaptureComponents(u8);

impl CaptureComponents {
    pub const NONE: Self = Self(0);
    pub const ADDRESS_IS_NULL: Self = Self(1);
    pub const ADDRESS: Self = Self(2 | 1);
    pub const READ_PROVENANCE: Self = Self(4);
    pub const PROVENANCE: Self = Self(8 | 4);
    pub const ALL: Self = Self(2 | 1 | 8 | 4);

    pub fn captures_nothing(self) -> bool {
        self == Self::NONE
    }

    pub fn captures_anything(self) -> bool {
        self != Self::NONE
    }

    pub fn union(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub fn intersect(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 & other.0)
    }

    /// `CC & ~Other`.
    pub fn without(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 & !other.0)
    }
}

/// `UseCaptureInfo`: what the use keeps, and what the user's result keeps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UseCaptureInfo {
    pub use_cc: CaptureComponents,
    pub result_cc: CaptureComponents,
}

impl UseCaptureInfo {
    pub fn new(
        use_cc: CaptureComponents,
        result_cc: CaptureComponents,
    ) -> Self {
        Self { use_cc, result_cc }
    }

    /// Nothing kept by the use, everything by the result.
    pub fn passthrough() -> Self {
        Self::new(CaptureComponents::NONE, CaptureComponents::ALL)
    }

    fn only(use_cc: CaptureComponents) -> Self {
        Self::new(use_cc, CaptureComponents::NONE)
    }
}

/// `CaptureTracker::Action`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Stop,
    Continue,
    ContinueIgnoringReturn,
}

/// `CaptureTracker`.
pub trait CaptureTracker {
    fn too_many_uses(&mut self);

    fn should_explore(
        &mut self,
        _: Use,
    ) -> bool {
        true
    }

    fn captured(
        &mut self,
        used: Use,
        info: UseCaptureInfo,
    ) -> Action;
}

/// What `PointerMayBeCaptured` found (`CaptureResult`): the components kept
/// by uses other than a `ret`, and with the `ret`s.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureResult {
    pub without_ret: CaptureComponents,
    pub with_ret: CaptureComponents,
}

/// What a use of the pointer by a call keeps: `CallBase::getCaptureInfo`
/// (Instructions.cpp:730), the argument's attributes met with the direct
/// callee's.
fn call_capture_info(
    function: &Function,
    globals: &[GlobalValue],
    context: &Context,
    call: InstId,
    argument: usize,
) -> CaptureComponents {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
        return CaptureComponents::ALL;
    };
    let by_value = |attrs: &[llrm_mir::opcode::Attribute]| {
        attrs
            .iter()
            .any(
                |one| matches!(
                    one,
                    llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
                ),
            )
    };
    let site = info.argument_attrs.get(argument).map(Vec::as_slice).unwrap_or_default();
    let callee = llrm_mir::memory::callee(context, function, call)
        .and_then(|one| globals.get(one.0 as usize))
        .and_then(GlobalValue::function);
    let declared = callee.and_then(|one| one.parameter_attrs.get(argument)).map(Vec::as_slice).unwrap_or_default();
    if by_value(site) || by_value(declared) {
        return CaptureComponents::NONE;
    }
    if Facts::of(site).no_capture() || Facts::of(declared).no_capture() {
        CaptureComponents::NONE
    } else {
        CaptureComponents::ALL
    }
}

/// `DetermineUseCaptureKind` (CaptureTracking.cpp:268,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CaptureTracking.cpp#L268>).
pub fn determine_use_capture_kind(
    function: &Function,
    globals: &[GlobalValue],
    context: &Context,
    used: Use,
    base: ValueId,
) -> UseCaptureInfo {
    let op = function.instruction(used.user);
    let index = used.index as usize;
    match &op.opcode {
        Opcode::Call(info) | Opcode::Invoke(info) => {
            // Calling a pointer does not capture it.
            if index + 1 == op.operands.len() {
                return UseCaptureInfo::only(CaptureComponents::NONE);
            }
            let kept = call_capture_info(function, globals, context, used.user, index);
            let site = MemoryEffects::of_attrs(&info.attrs);
            let declared = llrm_mir::memory::callee(context, function, used.user)
                .and_then(|one| globals.get(one.0 as usize))
                .and_then(GlobalValue::function)
                .map_or(MemoryEffects::unknown(), |one| MemoryEffects::of_attrs(&one.attrs));
            let returns_void = matches!(
                context.types.get(info.function_type),
                Type::Function { returns, .. } if matches!(context.types.get(*returns), Type::Void)
            );
            // A call that only reads and returns nothing keeps at most the
            // address.
            let mask = if site.intersect(declared).only_reads_memory() && returns_void {
                CaptureComponents::ADDRESS
            } else {
                CaptureComponents::ALL
            };
            UseCaptureInfo::new(kept.intersect(mask), kept)
        }
        Opcode::Load { volatile, .. } => {
            UseCaptureInfo::only(if *volatile { CaptureComponents::ADDRESS } else { CaptureComponents::NONE })
        }
        Opcode::Store { volatile, .. } => {
            // The stored value is kept; the address is not, unless volatile.
            if index == 0 {
                UseCaptureInfo::only(CaptureComponents::ALL)
            } else if *volatile {
                UseCaptureInfo::only(CaptureComponents::ADDRESS)
            } else {
                UseCaptureInfo::only(CaptureComponents::NONE)
            }
        }
        Opcode::GetElementPtr { .. } => {
            // A vector of pointers is not followed.
            let vector =
                op.result.is_some_and(|one| matches!(context.types.get(function.value(one).ty), Type::Vector { .. }));
            if vector { UseCaptureInfo::only(CaptureComponents::ALL) } else { UseCaptureInfo::passthrough() }
        }
        Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) | Opcode::Phi | Opcode::Select => {
            UseCaptureInfo::passthrough()
        }
        Opcode::ICmp(predicate) => {
            let other = op.operands[1 - index];
            let null = matches!(
                other,
                Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Null)
            );
            let equality = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);
            if null && equality && op.operands[index] == Operand::Value(base) {
                UseCaptureInfo::only(CaptureComponents::ADDRESS_IS_NULL)
            } else {
                UseCaptureInfo::only(CaptureComponents::ADDRESS)
            }
        }
        _ => UseCaptureInfo::only(CaptureComponents::ALL),
    }
}

/// `PointerMayBeCaptured(V, Tracker, MaxUsesToExplore)`
/// (CaptureTracking.cpp:397,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CaptureTracking.cpp#L397>).
pub fn pointer_may_be_captured_by(
    function: &Function,
    globals: &[GlobalValue],
    context: &Context,
    value: ValueId,
    tracker: &mut dyn CaptureTracker,
    max_uses: usize,
) {
    let max_uses = if max_uses == 0 { DEFAULT_MAX_USES_TO_EXPLORE } else { max_uses };
    let mut worklist: Vec<Use> = Vec::new();
    let mut visited: HashSet<(InstId, u32)> = HashSet::default();
    let mut add_uses = |of: ValueId, worklist: &mut Vec<Use>, tracker: &mut dyn CaptureTracker| {
        for &used in function.users(of) {
            if visited.len() >= max_uses {
                tracker.too_many_uses();
                return false;
            }
            if !visited.insert((used.user, used.index)) {
                continue;
            }
            if !tracker.should_explore(used) {
                continue;
            }
            worklist.push(used);
        }
        true
    };
    if !add_uses(value, &mut worklist, tracker) {
        return;
    }
    while let Some(used) = worklist.pop() {
        let info = determine_use_capture_kind(function, globals, context, used, value);
        if info.use_cc.captures_anything() {
            match tracker.captured(used, info) {
                Action::Stop => return,
                Action::ContinueIgnoringReturn => continue,
                // Followed only if the result keeps more than the use did.
                Action::Continue if info.result_cc.without(info.use_cc).captures_nothing() => continue,
                Action::Continue => {}
            }
        }
        if info.result_cc.captures_anything()
            && let Some(result) = function.instruction(used.user).result
            && !add_uses(result, &mut worklist, tracker)
        {
            return;
        }
    }
}

/// `SimpleCaptureTracker`.
struct Simple<'a> {
    mask: CaptureComponents,
    stop: fn(CaptureComponents) -> bool,
    cc: CaptureComponents,
    cc_with_ret: CaptureComponents,
    function: &'a Function,
}

impl CaptureTracker for Simple<'_> {
    fn too_many_uses(&mut self) {
        self.cc = self.mask;
        self.cc_with_ret = self.mask;
    }

    fn captured(
        &mut self,
        used: Use,
        info: UseCaptureInfo,
    ) -> Action {
        let kept = info.use_cc.intersect(self.mask);
        if kept.captures_nothing() {
            return Action::Continue;
        }
        self.cc_with_ret = self.cc_with_ret.union(kept);
        let ret = matches!(self.function.instruction(used.user).opcode, Opcode::Ret);
        if !ret {
            self.cc = self.cc.union(kept);
        }
        if (self.stop)(self.cc) { Action::Stop } else { Action::Continue }
    }
}

/// `PointerMayBeCaptured(V, Mask, StopFn, MaxUsesToExplore)`.
pub fn pointer_may_be_captured(
    function: &Function,
    globals: &[GlobalValue],
    context: &Context,
    value: ValueId,
    mask: CaptureComponents,
    stop: fn(CaptureComponents) -> bool,
    max_uses: usize,
) -> CaptureResult {
    let mut tracker =
        Simple { mask, stop, cc: CaptureComponents::NONE, cc_with_ret: CaptureComponents::NONE, function };
    pointer_may_be_captured_by(function, globals, context, value, &mut tracker, max_uses);
    CaptureResult { without_ret: tracker.cc, with_ret: tracker.cc_with_ret }
}

/// `EarliestCaptures`
/// (CaptureTracking.cpp:156,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CaptureTracking.cpp#L156>):
/// every capturing user, reduced to the nearest instruction that dominates
/// them all.
struct Earliest<'a> {
    function: &'a Function,
    tree: &'a DomTree,
    mask: CaptureComponents,
    earliest: Option<InstId>,
    cc: CaptureComponents,
    cc_with_ret: CaptureComponents,
}

impl CaptureTracker for Earliest<'_> {
    fn too_many_uses(&mut self) {
        self.cc = self.mask;
        self.cc_with_ret = self.mask;
        self.earliest = self.function.entry().and_then(|entry| self.function.iter_block(entry).next());
    }

    fn captured(
        &mut self,
        used: Use,
        info: UseCaptureInfo,
    ) -> Action {
        let kept = info.use_cc.intersect(self.mask);
        if kept.captures_anything() {
            self.cc_with_ret = self.cc_with_ret.union(kept);
            if !matches!(self.function.instruction(used.user).opcode, Opcode::Ret) {
                self.earliest = Some(match self.earliest {
                    None => used.user,
                    Some(held) => self.tree.find_nearest_common_dominator_inst(self.function, held, used.user),
                });
                self.cc = self.cc.union(kept);
            }
        }
        // Every potential capture is needed.
        Action::Continue
    }
}

/// `FindEarliestCapture`
/// (CaptureTracking.cpp:254,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/CaptureTracking.cpp#L254>):
/// the instruction that dominates every capture of `value` (none if it is not
/// captured; the entry block's first instruction if its uses are too many),
/// and which components may be captured by any use. The caller must make sure
/// an instruction compared against the answer is not in a cycle.
pub fn find_earliest_capture(
    function: &Function,
    globals: &[GlobalValue],
    context: &Context,
    tree: &DomTree,
    value: ValueId,
    mask: CaptureComponents,
    max_uses: usize,
) -> (Option<InstId>, CaptureResult) {
    let mut tracker = Earliest {
        function,
        tree,
        mask,
        earliest: None,
        cc: CaptureComponents::NONE,
        cc_with_ret: CaptureComponents::NONE,
    };
    pointer_may_be_captured_by(function, globals, context, value, &mut tracker, max_uses);
    (tracker.earliest, CaptureResult { without_ret: tracker.cc, with_ret: tracker.cc_with_ret })
}

#[cfg(test)]
#[path = "capturetracking_tests.rs"]
mod tests;
