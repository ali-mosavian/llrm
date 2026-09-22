//! Established source-language memory summaries for C library functions.
//!
//! Direct port of `qbopt/cfront/libfunc.py`.  These are semantic contracts,
//! not ABI contracts: register clobbers and stack cleanup remain the
//! backend's concern.  Object names are normalized by removing the target's
//! one C decoration underscore, so a user function actually named `_strlen`
//! is not mistaken for the standard function.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use indexmap::IndexMap;

use crate::analysis::alias::Summary;
use crate::model::memory::{Identity, MemoryKind, MemoryObject, Slice};

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
    LazyLock::new(|| IndexMap::from([("strlen", _readonly(&[0]))]));

/// Known summaries under the object names used at these call sites.
pub fn summaries<'a>(names: impl IntoIterator<Item = &'a str>) -> IndexMap<String, Summary> {
    let mut result = IndexMap::new();
    for name in names {
        let source_name = name.strip_prefix('_').unwrap_or(name);
        if let Some(summary) = _SOURCE.get(source_name) {
            result.insert(name.to_owned(), summary.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::summaries;
    use crate::model::memory::{Identity, MemoryKind, MemoryObject, Slice};

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
}
