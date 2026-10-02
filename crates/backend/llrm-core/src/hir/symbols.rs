//! Symbols every object built from the HIR names without declaring them.

use crate::objectfile::module::Space;
use crate::support::hash::IndexMap;

/// The callee name of an inline block's call: it names no routine.
pub const ASM: &str = "$asm";

/// The selector a far address of static data takes: DGROUP's.
pub const DGROUP: (Space, i64) = (Space::Group, 0);

/// The names of these symbols, which every object must define.
pub fn symbol_names() -> IndexMap<(Space, i64), String> {
    IndexMap::from_iter([(DGROUP, "DGROUP".to_owned())])
}
