//! Established source-language memory summaries for C library functions.
//!
//! Direct port of `qbopt/cfront/libfunc.py`.  These are semantic contracts,
//! not ABI contracts: register clobbers and stack cleanup remain the
//! backend's concern.  Object names are normalized by removing the target's
//! one C decoration underscore, so a user function actually named `_strlen`
//! is not mistaken for the standard function.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use llrm_core::support::hash::IndexMap;

use llrm_core::analysis::alias::Summary;
use llrm_core::model::memory::{Identity, MemoryKind, MemoryObject, Slice};

fn _readonly(parameters: &[i64]) -> Summary {
    Summary {
        reads: parameters
            .iter()
            .map(|parameter| {
                Slice::whole(MemoryObject {
                    identity: Some(Identity::Int(*parameter)),
                    ..MemoryObject::new(MemoryKind::Parameter)
                })
            })
            .collect::<BTreeSet<_>>(),
        ..Summary::default()
    }
}

// GCC marks strlen pure and constrains its use set to argument zero. LLVM adds
// readonly, argmemonly and nocapture(0). Open Watcom's implementation advances
// a local pointer while reading `*p` and performs no store.
static _SOURCE: LazyLock<IndexMap<&'static str, Summary>> =
    LazyLock::new(|| IndexMap::from_iter([("strlen", _readonly(&[0]))]));

/// Known summaries under the object names used at these call sites.
pub fn summaries<'a>(names: impl IntoIterator<Item = &'a str>) -> IndexMap<String, Summary> {
    let mut result = IndexMap::default();
    for name in names {
        let source_name = name.strip_prefix('_').unwrap_or(name);
        if let Some(summary) = _SOURCE.get(source_name) {
            result.insert(name.to_owned(), summary.clone());
        }
    }
    result
}

/// The C library `module` links against: each function it declares that
/// `summaries` knows, with what its summary promises as attributes.
pub fn library(module: &llrm_mir::Module) -> Result<llrm_mir::Module, String> {
    let mut out = llrm_mir::Module { datalayout: module.datalayout.clone(), ..llrm_mir::Module::default() };
    for (id, global, _) in module.functions().filter(|(_, _, one)| one.is_declaration()) {
        let Some(name) = global.name.as_deref() else { continue };
        let Some(summary) = summaries([name]).swap_remove(name) else { continue };
        let Some(declared) = out.declared(module, id)? else { continue };
        let llrm_mir::GlobalKind::Function(function) = &mut out.globals[declared.0 as usize].kind else { unreachable!("a function") };
        function.attrs.extend(memory(&summary));
        for at in 0..function.parameters().len() {
            let pointer = matches!(out.context.types.get(function.value(function.parameters()[at]).ty), llrm_mir::Type::Pointer(_));
            if pointer && !summary.unknown_read && !summary.captures.contains(&Some(Identity::Int(at as i64))) {
                function.parameter_attrs[at].push(llrm_mir::Attribute::Flag("nocapture".to_owned()));
            }
        }
    }
    Ok(out)
}

/// `memory(...)` of what `summary` reads and writes, where only its
/// parameters' memory.
fn memory(summary: &Summary) -> Option<llrm_mir::Attribute> {
    let arguments = |items: &BTreeSet<Slice>| items.iter().all(|one| one.object.kind == MemoryKind::Parameter);
    if summary.unknown_read || summary.unknown_write || !arguments(&summary.reads) || !arguments(&summary.writes) {
        return None;
    }
    let access = match (summary.reads.is_empty(), summary.writes.is_empty()) {
        (true, true) => return Some(llrm_mir::Attribute::Memory(vec![(None, "none".to_owned())])),
        (false, true) => "read",
        (true, false) => "write",
        (false, false) => "readwrite",
    };
    Some(llrm_mir::Attribute::Memory(vec![(Some("argmem".to_owned()), access.to_owned())]))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::summaries;
    use llrm_core::model::memory::{Identity, MemoryKind, MemoryObject, Slice};

    #[test]
    fn summaries_strip_exactly_one_decoration_underscore() {
        // Expected from Python: `list(libfunc.summaries(["_strlen", "__strlen", "strlen", "_puts"]))`.
        let found = summaries(["_strlen", "__strlen", "strlen", "_puts"]);

        assert_eq!(found.keys().collect::<Vec<_>>(), ["_strlen", "strlen"]);
        let parameter = MemoryObject {
            identity: Some(Identity::Int(0)),
            ..MemoryObject::new(MemoryKind::Parameter)
        };
        let strlen = &found["_strlen"];
        assert_eq!(strlen.reads, BTreeSet::from([Slice::whole(parameter)]));
        assert!(strlen.writes.is_empty() && strlen.captures.is_empty());
        assert!(!strlen.unknown_read && !strlen.unknown_write);
    }

    #[test]
    fn the_library_states_a_known_function_s_summary() {
        let module = llrm_mir::parse::module("declare i16 @_strlen(ptr)\ndeclare void @_puts(ptr)\n").unwrap();
        let library = llrm_mir::print::module(&super::library(&module).unwrap());
        assert!(library.contains("declare i16 @_strlen(ptr nocapture) memory(argmem: read)\n"), "{library}");
        assert!(!library.contains("puts"), "{library}");
    }
}
