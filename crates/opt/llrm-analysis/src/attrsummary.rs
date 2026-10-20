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
//! DEVIATE A1: no slices of tracked globals. `GlobalsModRef` answers those per
//! global; here they are the unknown access. Checked by
//! `the_unknown_access_stands_for_globals`.

use llrm_mir::facts::Facts;
use llrm_mir::module::Module;
use llrm_mir::types::Type;
use llrm_support::hash::IndexMap;

use crate::aa::{IRMemLocation, MemoryEffects, ModRefInfo};
use crate::alias::Summary;
use crate::memory::{Identity, Key, MemoryKind, MemoryObject, ObjectInterner, Slice};

/// The summaries of the defined functions of `module`, by name.
pub fn summaries(module: &Module) -> IndexMap<String, Summary> {
    let objects = ObjectInterner::of(&module.context);
    let mut out = IndexMap::default();
    for (_, global, function) in module.functions() {
        let Some(name) = global.name.as_ref() else { continue };
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
