//! # How it works
//! `LLRM_CHECK_CAPTURES`: the frame objects the pointer facts say never escape,
//! checked against LLVM's capture tracking read straight off the uses, with no
//! fact followed: a pointer is captured when any use of it, or of what is made
//! of it by a gep, a cast, a phi or a select (every arm), stores it, returns
//! it, turns it into an integer or passes it where the callee keeps no copy is
//! not promised. A fact that misses one of these is an under-escape: a store to
//! the object may be dropped while a later load can still read it.
//!
//! # Example
//! A slot whose pointer is stored to a global is captured; the check names it
//! when the facts say it never escapes:
//!
//! ```text
//! %p = alloca i32
//! store ptr %p, ptr @g   ; captured
//! ```
//!
//! # Provides
//! The check behind `LLRM_CHECK_CAPTURES`.

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, ValueId};
use llrm_mir::opcode::CastOp;
use llrm_mir::opcode::Opcode;

use crate::alias::PointsTo;
use crate::memory::{MemoryKind, Unit, is_lifetime_marker};

/// Why a callee's parameter is not shown uncaptured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unproven {
    /// No body in the module.
    Undefined,
    TooDeep,
    /// The body captures it, or keeps it in a local.
    Captured,
}

/// The module whose bodies answer "is parameter `i` of `f` captured".
pub struct Verify<'a> {
    pub module: &'a llrm_mir::module::Module,
    pub layout: &'a llrm_mir::datalayout::DataLayout,
    depth: u32,
    /// A callee without a body here is taken as not capturing: the audit of a
    /// single module, whose other modules' bodies it cannot read.
    pub trust_external: bool,
    stack: std::cell::RefCell<Vec<(llrm_mir::context::GlobalId, usize)>>,
    /// Parameters shown uncaptured, by callee name, for the report.
    pub verified: std::cell::RefCell<Vec<String>>,
    pub unproven: std::cell::RefCell<Vec<(String, Unproven)>>,
}

impl<'a> Verify<'a> {
    pub fn new(
        module: &'a llrm_mir::module::Module,
        layout: &'a llrm_mir::datalayout::DataLayout,
        depth: u32,
    ) -> Self {
        Self {
            module,
            layout,
            depth,
            trust_external: false,
            stack: Default::default(),
            verified: Default::default(),
            unproven: Default::default(),
        }
    }

    /// `parameter`, for a callee as the unit names it: a clone the pass now
    /// running is making (`name@N`, a specialisation of `name`) is not in the
    /// module yet, so its original's body answers.
    pub fn parameter_named(
        &self,
        unit: &Unit,
        callee: llrm_mir::context::GlobalId,
        index: usize,
    ) -> Result<(), Unproven> {
        let named = unit.globals.get(callee.0 as usize).and_then(|global| global.name.as_deref());
        let held = self.module.globals.get(callee.0 as usize).and_then(|global| global.name.as_deref());
        if named.is_none() || named == held {
            return self.parameter(callee, index);
        }
        let base = named.and_then(|name| name.split('@').next()).unwrap_or_default();
        match self.module.globals.iter().position(|global| global.name.as_deref() == Some(base)) {
            Some(found) => self.parameter(llrm_mir::context::GlobalId(found as u32), index),
            None => Err(Unproven::Undefined),
        }
    }

    /// `Ok` when parameter `index` of `callee` is shown uncaptured by its body,
    /// recursively through the module's other bodies.
    pub fn parameter(
        &self,
        callee: llrm_mir::context::GlobalId,
        index: usize,
    ) -> Result<(), Unproven> {
        let global = self.module.globals.get(callee.0 as usize).ok_or(Unproven::Undefined)?;
        let name = global.name.owned().unwrap_or_default();
        let top = self.stack.borrow().is_empty();
        let found = (|| {
            let function = global.function().filter(|one| !one.is_declaration()).ok_or(Unproven::Undefined)?;
            // Each parameter on the stack is assumed uncaptured while its body
            // is read (LLVM's SCC step): a cycle that passes them
            // on only, in any order, is no capture.
            if self.stack.borrow().contains(&(callee, index)) {
                return Ok(());
            }
            if self.stack.borrow().len() as u32 >= self.depth {
                return Err(Unproven::TooDeep);
            }
            let root = *function.parameters().get(index).ok_or(Unproven::Undefined)?;
            self.stack.borrow_mut().push((callee, index));
            let unit = Unit::of(self.module, self.layout, function);
            let mut kept = Vec::new();
            let captured = captured_by(&unit, root, &mut kept, Some(self), false, false, usize::MAX);
            self.stack.borrow_mut().pop();
            // Kept in a local slot of the callee: read back it is the pointer
            // again (followed), and it is captured if the slot
            // itself is.
            let slot_captured = kept.iter().any(|holder| {
                captured_by(&unit, *holder, &mut Vec::new(), Some(self), false, false, usize::MAX).is_some()
            });
            if captured.is_some() || slot_captured { Err(Unproven::Captured) } else { Ok(()) }
        })();
        if top {
            match found {
                Ok(()) => self.verified.borrow_mut().push(name),
                Err(why) => self.unproven.borrow_mut().push((name, why)),
            }
        }
        found
    }
}

/// `check_with` over every body of `module`, on facts derived again from a
/// plain unit of each: the split of what the facts leave unescaped into
/// verified and not, printed (`LLRM_CHECK_CAPTURES_MODULE`).
pub fn check_module(
    module: &llrm_mir::module::Module,
    layout: &llrm_mir::datalayout::DataLayout,
) {
    let verify = Verify::new(module, layout, 8);
    let mut misses = 0usize;
    for global in &module.globals {
        let Some(function) = global.function().filter(|one| !one.is_declaration()) else { continue };
        let plain = Unit::of(module, layout, function);
        let known = crate::consts::known(&Unit { registers: None, ..plain });
        let shape = crate::cfg::Shape::of(function);
        let unit = plain.with_registers(&known).with_shape(&shape);
        let Ok(facts) = crate::alias::points_to(&unit, None, None) else { continue };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check_with(&unit, &facts, Some(&verify))));
        if let Err(why) = result {
            misses += 1;
            let text = why.downcast_ref::<String>().cloned().unwrap_or_default();
            eprintln!("CAPTURES-MODULE-MISS {}: {}", global.name.owned().unwrap_or_default(), text);
        }
    }
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, why) in verify.unproven.borrow().iter() {
        *reasons.entry(format!("{why:?}")).or_default() += 1;
    }
    eprintln!(
        "CAPTURES-MODULE verified {} unproven {:?} under-escape functions {}",
        verify.verified.borrow().len(),
        reasons,
        misses
    );
}

/// The alloca an address is made of by geps and casts.
fn frame_base(
    unit: &Unit,
    address: llrm_mir::module::Operand,
) -> Option<ValueId> {
    let function = unit.function;
    let mut at = address;
    loop {
        let llrm_mir::module::Operand::Value(value) = at else { return None };
        let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return None };
        let op = function.instruction(inst);
        match &op.opcode {
            Opcode::Alloca { .. } => return Some(value),
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => at = op.operands[0],
            _ => return None,
        }
    }
}

/// Whether a join's arm is a pointer nothing identifies: read from memory,
/// passed in, returned, or made from an integer.
fn unknown_arm(
    unit: &Unit,
    arm: llrm_mir::module::Operand,
    followed: &IdSet<ValueId>,
) -> bool {
    let function = unit.function;
    let mut at = arm;
    loop {
        let llrm_mir::module::Operand::Value(value) = at else { return false };
        // An arm already followed is no unknown: it is the pointer being
        // tracked.
        if followed.contains(&value) {
            return false;
        }
        let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { return true };
        let op = function.instruction(inst);
        match &op.opcode {
            Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => at = op.operands[0],
            Opcode::Load { .. } | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Cast(CastOp::IntToPtr) => return true,
            _ => return false,
        }
    }
}

/// The first instruction that captures the address `root`, if any.
fn captured_by(
    unit: &Unit,
    root: ValueId,
    held_in: &mut Vec<ValueId>,
    verify: Option<&Verify>,
    joins: bool,
    attr_calls: bool,
    max_uses: usize,
) -> Option<InstId> {
    let function = unit.function;
    let mut seen: IdSet<ValueId> = IdSet::new();
    let mut explored = 0usize;
    let mut work = vec![root];
    while let Some(value) = work.pop() {
        if !seen.insert(value) {
            continue;
        }
        for used in function.users(value) {
            // LLVM's cap: past this many uses explored the pointer is taken as
            // captured.
            explored += 1;
            if explored > max_uses {
                return Some(used.user);
            }
            let op = function.instruction(used.user);
            let at = used.index as usize;
            match &op.opcode {
                Opcode::GetElementPtr { .. } if at == 0 => work.extend(op.result),
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => work.extend(op.result),
                // A join with an arm no object is followed through is itself
                // none: the objects of its other arms are lost (a load through
                // it reads them).
                Opcode::Phi | Opcode::Select if at != 0 || matches!(op.opcode, Opcode::Phi) => {
                    // The facts lose a join with an arm no object is followed
                    // through; a callee's parameter is
                    // captured as LLVM counts it, which follows a
                    // join and keeps nothing.
                    if joins
                        && op.operands.iter().enumerate().any(|(index, one)| {
                            (matches!(op.opcode, Opcode::Phi) || index != 0)
                                && index != at
                                && unknown_arm(unit, *one, &seen)
                        })
                    {
                        return Some(used.user);
                    }
                    work.extend(op.result);
                }
                Opcode::Load { .. } | Opcode::ICmp(_) => {}
                Opcode::Store { .. } if at == 1 => {}
                // Kept in another frame object: captured if that one is.
                Opcode::Store { .. } if frame_base(unit, op.operands[1]).is_some() => {
                    let holder = frame_base(unit, op.operands[1]).expect("a frame base");
                    held_in.push(holder);
                    // What is read back from the slot is the pointer again.
                    let mut slots = vec![holder];
                    let mut walked: IdSet<ValueId> = IdSet::new();
                    while let Some(slot) = slots.pop() {
                        if !walked.insert(slot) {
                            continue;
                        }
                        for read in function.users(slot) {
                            let reader = function.instruction(read.user);
                            match &reader.opcode {
                                Opcode::Load { .. } if read.index == 0 => work.extend(reader.result),
                                Opcode::GetElementPtr { .. }
                                | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast)
                                    if read.index == 0 =>
                                {
                                    slots.extend(reader.result);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    let args = match op.opcode {
                        Opcode::Call(_) => op.operands.len() - 1,
                        _ => op.operands.len() - 3,
                    };
                    if at >= args || is_lifetime_marker(unit, used.user) {
                        continue;
                    }
                    let callee = llrm_mir::memory::callee(unit.context, function, used.user);
                    // The module defines the callee: its own body says (no
                    // attribute is trusted); one it does
                    // not define, or recursion, or a chain too
                    // long, keeps the pointer unless the declaration says
                    // nocapture.
                    match (callee, verify) {
                        (Some(callee), Some(verify)) => match verify.parameter_named(unit, callee, at) {
                            Ok(()) => {}
                            Err(Unproven::Undefined) if crate::alias::borrowed(unit, used.user, at) => {}
                            // A callee this compile has no body for is another
                            // module's: the audit leaves it, the module report
                            // counts it.
                            Err(Unproven::Undefined) if verify.trust_external => {}
                            Err(_) => return Some(used.user),
                        },
                        // A unit holds the other bodies as declarations and the
                        // facts follow the summaries of those the module
                        // defines, so a direct call is read only in the mode
                        // that reports them.
                        (Some(_), None) if !attr_calls && !llrm_support::env_flag!("LLRM_CHECK_CAPTURES_CALLS") => {}
                        _ => {
                            if llrm_support::env_flag!("LLRM_CHECK_CAPTURES_BLIND")
                                || !crate::alias::borrowed(unit, used.user, at)
                            {
                                return Some(used.user);
                            }
                        }
                    }
                }
                _ => return Some(used.user),
            }
        }
    }
    None
}

/// `capture-tracking-max-uses-to-explore`'s default (150): past it a pointer is
/// taken as captured.
pub const MAX_USES_TO_EXPLORE: usize = 150;

/// Panics on a frame object some use captures that `facts.escaped` lacks.
pub fn check(
    unit: &Unit,
    facts: &PointsTo,
) {
    check_with(unit, facts, None)
}

/// The frame objects a use captures that `facts.escaped` lacks, as text; with a
/// `Verify` a direct call's callee is read in its own body.
pub fn check_with(
    unit: &Unit,
    facts: &PointsTo,
    verify: Option<&Verify>,
) {
    let function = unit.function;
    let mut direct: Vec<(ValueId, Option<InstId>, Vec<ValueId>)> = Vec::new();
    for (_, inst) in function.walk() {
        let op = function.instruction(inst);
        let (Opcode::Alloca { .. }, Some(value)) = (&op.opcode, op.result) else { continue };
        let mut held_in = Vec::new();
        let by = captured_by(unit, value, &mut held_in, verify, true, false, usize::MAX);
        direct.push((value, by, held_in));
    }
    // Held in a captured object is captured, to a fixed point.
    let mut by: Vec<Option<InstId>> = direct.iter().map(|one| one.1).collect();
    loop {
        let mut changed = false;
        for (at, one) in direct.iter().enumerate() {
            if by[at].is_some() {
                continue;
            }
            if let Some(found) = one
                .2
                .iter()
                .find_map(|holder| direct.iter().position(|other| other.0 == *holder).and_then(|index| by[index]))
            {
                by[at] = Some(found);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut missed = Vec::new();
    for (at, (value, ..)) in direct.iter().enumerate() {
        let Some(by) = by[at] else { continue };
        let said = facts
            .escaped
            .iter()
            .any(
                |one| one.kind == MemoryKind::Frame
                    && matches!(
                        one.key,
                        crate::memory::Key::Value(number) if number == value.0
                    ),
            );
        if !said {
            missed.push((
                *value,
                by,
                format!(
                    "{:?} {:?}",
                    function.instruction(by).opcode,
                    llrm_mir::memory::callee(unit.context, function, by)
                        .and_then(|one| unit.globals.get(one.0 as usize).and_then(|global| global.name.owned()))
                ),
            ));
        }
    }
    if llrm_support::env_flag!("LLRM_CHECK_CAPTURES_COUNT") {
        for one in &missed {
            eprintln!("CAPTURES-MISS {}", one.2);
        }
        return;
    }
    assert!(
        missed.is_empty(),
        "LLRM_CHECK_CAPTURES: frame objects captured by a use that the facts say never escape: {:?}",
        missed
    );
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::alias::points_to;
    use crate::memory::Unit;
    use crate::testing::{DOS, function, layout, parsed};

    const KEEPS: &str =
        "define void @f(ptr %keep) {\nentry:\n  %s = alloca i32\n  call void %keep(ptr %s)\n  ret void\n}\n";

    /// A frame object handed to a function called through a pointer escapes.
    /// The check passes on the facts the solve gives, and stops on facts
    /// that forgot it: the under-escape that dropped a store (a join with
    /// an unknown arm).
    #[test]
    fn a_frame_object_passed_to_a_pointer_callee_must_be_in_the_escaped_set() {
        let module = parsed(&format!("{DOS}{KEEPS}"));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        check(&unit, &facts);
        facts.escaped.clear();
        let forgotten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&unit, &facts)));
        assert!(forgotten.is_err(), "facts that forgot the escape were accepted");
    }

    /// `phi [%slot, %b0], [%q, %b1]` with `%q` read from memory is a pointer no
    /// object is followed through: the slot behind it is lost, and the store
    /// to it was dropped (cgscc-poc's miscompile on main). The check names it.
    #[test]
    fn a_frame_object_behind_a_join_with_an_unknown_arm_must_be_in_the_escaped_set() {
        let module = parsed(&format!(
            "{DOS}define i32 @f(i1 %c, ptr %cell) {{\nentry:\n  %s = alloca i32\n  %q = load ptr, ptr %cell\n  br i1 %c, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %p = phi ptr [ %s, %a ], [ %q, %b ]\n  %v = load i32, ptr %p\n  ret i32 %v\n}}\n"
        ));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        facts.escaped.clear();
        let forgotten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&unit, &facts)));
        assert!(forgotten.is_err(), "facts that lost the slot behind the join were accepted");
    }

    /// `&local` passed to a function nobody defines is the commonest capture
    /// there is. The check read only calls through a pointer at first, so
    /// facts that forgot it were accepted (the review of #2146).
    #[test]
    fn a_frame_object_passed_to_an_opaque_direct_call_must_be_in_the_escaped_set() {
        let module = parsed(&format!(
            "{DOS}declare void @keep(ptr)\ndefine void @f() {{\nentry:\n  %s = alloca i32\n  call void @keep(ptr %s)\n  ret void\n}}\n"
        ));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        let verify = super::Verify::new(&module, &layout, 4);
        super::check_with(&unit, &facts, Some(&verify));
        facts.escaped.clear();
        let forgotten =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::check_with(&unit, &facts, Some(&verify))));
        assert!(forgotten.is_err(), "facts that forgot the escape to an opaque call were accepted");
    }

    /// A pointer passed to a function the module defines is read in that body:
    /// kept in a global it is captured, only loaded through it is not (the 285
    /// calls the facts rested on the stamp's summaries for).
    #[test]
    fn a_pointer_passed_to_a_defined_callee_is_read_in_the_callee_body() {
        let module = parsed(&format!(
            "{DOS}@g = global ptr null\ndefine void @keeps(ptr %p) {{\nentry:\n  store ptr %p, ptr @g\n  ret void\n}}\ndefine i32 @reads(ptr %p) {{\nentry:\n  %v = load i32, ptr %p\n  ret i32 %v\n}}\ndefine i32 @f() {{\nentry:\n  %s = alloca i32\n  %t = alloca i32\n  call void @keeps(ptr %s)\n  %r = call i32 @reads(ptr %t)\n  ret i32 %r\n}}\n"
        ));
        let layout = layout(&module);
        let unit = crate::testing::with_registers(Unit::of(&module, &layout, function(&module, "f")));
        let mut facts = points_to(&unit, None, None).unwrap();
        facts.escaped.clear();
        let verify = super::Verify::new(&module, &layout, 4);
        let forgotten = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::check_with(&unit, &facts, Some(&verify));
        }));
        let text = forgotten.err().and_then(|why| why.downcast_ref::<String>().cloned()).expect("%s is captured");
        assert!(text.contains("ValueId(0)"), "{text}");
        assert!(!text.contains("ValueId(1)"), "the read-only callee kept %t: {text}");
        assert_eq!(verify.verified.borrow().as_slice(), ["reads"]);
    }
}
