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

/// Of `names`, the functions that only read what their pointer arguments
/// reach and keep none of them.
pub fn reads_arguments<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let parameters = |items: &BTreeSet<Slice>| items.iter().all(|one| one.object.kind == MemoryKind::Parameter);
    summaries(names)
        .into_iter()
        .filter(|(_, one)| !one.unknown_read && !one.unknown_write && one.writes.is_empty() && one.captures.is_empty() && parameters(&one.reads))
        .map(|(name, _)| name)
        .collect()
}

/// The C library's routines whose result is a three-way compare of their
/// data, as LLVM's LibFunc knows strcmp's: `_fstrcmp` and `_fmemcmp` are
/// Borland's far forms.
const THREE_WAY: [&str; 5] = ["strcmp", "strncmp", "memcmp", "_fstrcmp", "_fmemcmp"];

/// Whether the function of object name `name` is one of them.
pub fn three_way_compare(name: &str) -> bool {
    THREE_WAY.contains(&name.strip_prefix('_').unwrap_or(name))
}

/// The C library's routines that return twice: once as called, once more
/// from the `longjmp` that jumps back to them.
const RETURNS_TWICE: [&str; 3] = ["setjmp", "sigsetjmp", "savectx"];

/// Whether the function of object name `name` is one of them.
pub fn returns_twice(name: &str) -> bool {
    RETURNS_TWICE.contains(&name.strip_prefix('_').unwrap_or(name))
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

    /// `setjmp` returns twice by its name, with or without the C decoration.
    #[test]
    fn setjmp_is_known_to_return_twice() {
        assert!(super::returns_twice("_setjmp") && super::returns_twice("setjmp") && super::returns_twice("_sigsetjmp"));
        assert!(!super::returns_twice("_longjmp") && !super::returns_twice("_setjmp2"));
    }
}
