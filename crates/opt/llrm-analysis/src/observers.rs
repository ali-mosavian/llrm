//! Finds the memory cells nothing outside a function can read, so dead store
//! elimination can drop a store that no later code observes.
//!
//! # How it works
//! A store is dead when everything after it overwrites the cell before reading
//! it, or when nothing after it can read the cell at all. `avail::dead_stores`
//! answers the first; this answers the second: at a call or an exit, which
//! cells are still observable.
//!
//! `private` returns a test on a `MemRef`. A reference is private when its
//! provenance is non-empty and every slice lies inside a `Frame` object, within
//! that object's extent, whose address did not escape (`PointsTo::escaped`: a
//! call, a return, a `ptrtoint` or a store outside the frame) and that is not
//! published. Globals are never private: other functions and, unless
//! `internal`, other modules reach them.
//!
//! An object is published when something reads it without naming it:
//!
//! 1. A load or store whose `MemRef` is not `named` (a variable index into a
//!    local is not a name) publishes every known object in its provenance.
//! 2. A call publishes the objects of its read arguments. `nocapture` keeps the
//!    address from escaping, not the callee from reading it during the call.
//! 3. A pointer stored into a published object publishes its pointee, repeated
//!    to a fixed point: a callee given a struct loads the pointer in it.
//!
//! ```text
//!  %s = alloca {ptr}      store %buf -> %s      call @f(ptr %s)
//!                                                   |
//!    %s  published (read argument) ----------------+
//!    %buf published (stored into %s, step 3)
//! ```
//!
//! # Example
//! `%x` is never read and its address never leaves the function, so it is
//! private:
//!
//! ```mir
//! define void @f(i1 %c, i1 %d) {
//! b0:
//!   %x = alloca i16
//!   br label %b1
//!
//! b1:
//!   br i1 %c, label %b2, label %b3
//!
//! b2:
//!   store i16 1, ptr %x
//!   br label %b3
//!
//! b3:
//!   br i1 %d, label %b1, label %b4
//!
//! b4:
//!   ret void
//! }
//! ```
//!
//! | `avail::dead_stores`     | dead stores |
//! |--------------------------|-------------|
//! | with `private(..)`       | `b2`[0]     |
//! | without                  | none        |
//!
//! From unit test `test_a_store_no_iteration_reads_is_dead_inside_its_loop` in
//! `observers_tests.rs`.
//!
//! # Provides
//! `private` and `private_of` (the same test with `Published`, the manager's
//! cached set, passed in), and the `Published` analysis. Consumer: the `dse`
//! pass in llrm-transforms.
//!
//! # Needs
//! [`crate::manager`]'s `Pointers` (points-to) and `ExposedFrames`,
//! [`crate::alias`], [`crate::memory`].

use std::borrow::Cow;
use std::collections::BTreeSet;

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, Analysis};

use crate::alias::PointsTo;
use crate::manager::Pointers;
use crate::memory::{MemRef, MemoryKind, ObjectRef, Unit};

/// A test for the cells no call and no exit of `unit`'s function can
/// observe: each slice of the reference's provenance lies inside a frame
/// object whose address did not escape, and that no access reaches without
/// naming it.
pub fn private<'a>(
    unit: Unit<'a>,
    pointers: &'a PointsTo,
) -> impl Fn(&MemRef) -> bool + 'a {
    let published = _published(&unit, pointers);
    _private(unit, pointers, Cow::Owned(published))
}

/// `private`, `published` being the manager's `Published`.
pub fn private_of<'a>(
    unit: Unit<'a>,
    pointers: &'a PointsTo,
    published: &'a BTreeSet<ObjectRef>,
) -> impl Fn(&MemRef) -> bool + 'a {
    _private(unit, pointers, Cow::Borrowed(published))
}

fn _private<'a>(
    unit: Unit<'a>,
    pointers: &'a PointsTo,
    published: Cow<'a, BTreeSet<ObjectRef>>,
) -> impl Fn(&MemRef) -> bool + 'a {
    move |reference: &MemRef| {
        let provenance = reference.provenance.clone().or_else(|| pointers.reference(&unit, reference));
        provenance.is_some_and(|provenance| {
            !provenance.slices.is_empty()
                && provenance.slices.iter().all(|one| {
                    one.object.kind == MemoryKind::Frame
                        && one.object.extent.is_some_and(|extent| {
                            0 <= one.low && one.low < one.high && one.high + one.width - 1 <= extent
                        })
                        && !pointers.escaped.contains(&one.object)
                        && !published.contains(&one.object)
                })
        })
    }
}

/// `_published`: the objects `private` rules out.
pub struct Published;

impl Analysis for Published {
    type Result = Result<BTreeSet<ObjectRef>, String>;
    const NAME: &'static str = "published";

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        let pointers = analyses.get::<Pointers>(context, layout, function);
        let pointers = Result::as_ref(&*pointers).map_err(String::clone)?;
        let exposed = analyses.get::<crate::manager::ExposedFrames>(context, layout, function);
        Ok(_published(&Unit::within(context, layout, function, analyses.outer()).with_exposed(&exposed), pointers))
    }
}

/// Objects some load or store reaches through a value rather than by name:
/// what its provenance holds, the old `observers` rule for an access
/// through a pointer. A slot read back through its address kept in memory
/// is not private to its name.
///
/// So is what a call may read through an argument: `nocapture` keeps the
/// address from escaping, not the callee from reading it during the call.
fn _published(
    unit: &Unit,
    pointers: &PointsTo,
) -> BTreeSet<ObjectRef> {
    let mut published = BTreeSet::new();
    for (_, inst) in unit.function.walk() {
        for argument in crate::alias::read_arguments(unit, inst) {
            let Operand::Value(value) = argument else { continue };
            let passed = pointers.values.get(&value).into_iter().flat_map(|provenance| provenance.slices.iter());
            published.extend(passed.map(|one| one.object.clone()).filter(|one| one.kind != MemoryKind::Unknown));
        }
        let Some(reference) = MemRef::of(unit, inst).or_else(|| MemRef::filled(unit, inst)) else { continue };
        let Some(provenance) = reference.provenance.clone().or_else(|| pointers.reference(unit, &reference)) else {
            continue;
        };
        // As `avail::dead_stores` asks it: of the access itself, not of what
        // points-to resolves. A variable index into a local is not a name.
        if !reference.named() {
            published.extend(
                provenance.slices.iter().map(|one| one.object).filter(|one| one.kind != MemoryKind::Unknown),
            );
        }
    }
    // What a published object holds is read too: a callee given a struct
    // loads the pointer in it and reads what it points to. A pointer stored
    // into a published object publishes its pointee, and so on in.
    let stores: Vec<(ValueId, ValueId)> = unit
        .function
        .walk()
        .filter_map(|(_, inst)| {
            let instruction = unit.function.instruction(inst);
            match (&instruction.opcode, &instruction.operands[..]) {
                (Opcode::Store { .. }, [Operand::Value(value), Operand::Value(address), ..]) => {
                    Some((*value, *address))
                }
                _ => None,
            }
        })
        .collect();
    let objects = |value: &ValueId| -> Vec<ObjectRef> {
        pointers
            .values
            .get(value)
            .into_iter()
            .flat_map(|provenance| provenance.slices.iter())
            .map(|one| one.object.clone())
            .filter(|one| one.kind != MemoryKind::Unknown)
            .collect()
    };
    loop {
        let before = published.len();
        for (value, address) in &stores {
            if objects(address).iter().any(|one| published.contains(one)) {
                published.extend(objects(value));
            }
        }
        if published.len() == before {
            return published;
        }
    }
}

#[cfg(test)]
#[path = "observers_tests.rs"]
mod tests;
