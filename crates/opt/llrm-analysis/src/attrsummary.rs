//! `Summary` for each defined function, derived from its attributes alone.
//!
//! The drop-in for `alias::summaries` that the call-effect readers move onto:
//! what `FunctionAttrs` states (`memory(...)`, `readonly`, `writeonly`,
//! `nocapture`) in the shape `Summary` has.
//!
//! - `argmem` access is a whole-object slice of each pointer parameter that
//!   `readnone`, `readonly` and `writeonly` do not rule out.
//! - Any other location (`other`, `inaccessiblemem`, `errnomem`) is an unknown
//!   read or write.
//! - A pointer parameter without `nocapture` is captured.
//!
//! # Example
//! A function that only reads through its pointer parameter:
//!
//! ```mir
//! define void @f(ptr readonly %p) memory(argmem: readwrite) {
//! b0:
//!   ret void
//! }
//! ```
//!
//! `summaries(module)["f"]` reads parameter 0 and writes nothing.
//!
//! From unit test `a_readonly_parameter_is_read_not_written` in
//! `attrsummary_tests.rs`.
//!
//! A tracked global (`GlobalsModRef::is_non_address_taken`) of a function that
//! has a `FunctionInfo` is a whole-object slice in `reads` or `writes` as
//! `mod_ref_for_global` says, and `globals_listed` keeps the unknown access
//! from reaching it. Without a `FunctionInfo` the unknown access reaches it.

use llrm_mir::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::Facts;
use llrm_mir::module::{GlobalKind, Module};
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use crate::aa::{IRMemLocation, MemoryEffects, ModRefInfo};
use crate::alias::Summary;
use crate::memory::{Identity, Key, MemoryKind, MemoryObject, ObjectInterner, Slice};

thread_local! {
    static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has made the summaries of a module, for a test
/// that an edit to a body does not make them again.
pub fn summaries_made() -> usize {
    MADE.with(std::cell::Cell::get)
}

/// The summaries of the defined functions of `module`, by name.
///
/// `tracked` are the globals alias's queries treat as uncaptured (`GlobalsAA`);
/// the slices of one are the object alias makes of it, so the unknown access is
/// held off them only where `GlobalsModRef` tracks every one.
pub fn summaries(
    module: &Module,
    layout: &DataLayout,
    tracked: &std::collections::BTreeSet<GlobalId>,
    found: &crate::globalsmodref::GlobalsModRef,
) -> IndexMap<String, Summary> {
    MADE.with(|made| made.set(made.get() + 1));
    let objects = ObjectInterner::of(&module.context);
    let listable = tracked.iter().all(|&id| found.is_non_address_taken(id));
    let mut out = IndexMap::default();
    for (id, global, function) in module.functions() {
        // A declaration is not described: a call to one is answered from its
        // own attributes (`calls_by_attributes`).
        let Some(name) = global.name.as_ref().filter(|_| !function.is_declaration()) else { continue };
        let me = MemoryEffects::of_attrs(&function.attrs);
        let elsewhere = [IRMemLocation::Other, IRMemLocation::InaccessibleMem, IRMemLocation::ErrnoMem]
            .map(|at| me.mod_ref(at))
            .into_iter()
            .fold(ModRefInfo::NoModRef, ModRefInfo::union);
        let mut summary = Summary {
            unknown_read: elsewhere.is_ref_set(),
            unknown_write: elsewhere.is_mod_set(),
            ..Summary::default()
        };
        if let Some(info) = found.function_info(id).filter(|_| listable) {
            summary.globals_listed = true;
            for &global in tracked {
                let access = info.mod_ref_for_global(global);
                let GlobalKind::Variable(variable) = &module.global(global).kind else { continue };
                let object = objects.intern(MemoryObject {
                    identity: Some(Identity::Global(global.0)),
                    extent: Some(layout.alloc_size(&module.context.types, variable.ty) as i64),
                    captured: false,
                    constant: variable.constant,
                    ..MemoryObject::new(MemoryKind::Global)
                });
                if access.is_ref_set() {
                    summary.reads.insert(Slice::whole(object));
                }
                if access.is_mod_set() {
                    summary.writes.insert(Slice::whole(object));
                }
            }
        }
        for (index, &value) in function.parameters().iter().enumerate() {
            let pointer = function
                .operand_type(&module.context, llrm_mir::module::Operand::Value(value))
                .is_some_and(|ty| matches!(module.context.types.get(ty), Type::Pointer(_)));
            if !pointer {
                continue;
            }
            let stated = Facts::param(function, index);
            if !stated.no_capture() {
                summary.captures.insert(Some(Identity::Int(index as i64)));
            }
            let access = if stated.read_none() {
                ModRefInfo::NoModRef
            } else if stated.read_only() {
                me.mod_ref(IRMemLocation::ArgMem).intersect(ModRefInfo::Ref)
            } else if stated.write_only() {
                me.mod_ref(IRMemLocation::ArgMem).intersect(ModRefInfo::Mod)
            } else {
                me.mod_ref(IRMemLocation::ArgMem)
            };
            let object = objects.intern(MemoryObject {
                identity: Some(Identity::Int(index as i64)),
                ..MemoryObject::new(MemoryKind::Parameter)
            });
            debug_assert_eq!(object.key, Key::Int(index as i64));
            let whole = Slice::whole(object);
            if access.is_ref_set() {
                summary.reads.insert(whole);
            }
            if access.is_mod_set() {
                summary.writes.insert(whole);
            }
        }
        out.insert(name.clone(), summary);
    }
    out
}

#[cfg(test)]
#[path = "attrsummary_tests.rs"]
mod tests;
