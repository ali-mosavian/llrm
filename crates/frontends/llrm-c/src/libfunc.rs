//! What the C library's routines are known to do, by name: the one table.
//!
//! These are semantic contracts, not ABI contracts: register clobbers and
//! stack cleanup remain the backend's concern. A name loses the target's
//! one C decoration underscore before lookup, so a user function actually
//! named `_strlen` is not mistaken for the standard one.

/// A fact the table states of a routine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Known {
    /// Reads only what its pointer arguments reach and keeps none of them.
    ReadsArguments,
    /// Its result is a three-way compare of its data, as LLVM's LibFunc
    /// knows strcmp's.
    ThreeWay,
    /// Returns once as called and once more from the `longjmp` back to it.
    ReturnsTwice,
}

use Known::*;

// strlen: GCC marks it pure with argument zero its only use; LLVM adds
// readonly, argmemonly and nocapture(0). `_fstrcmp` and `_fmemcmp` are
// Borland's far forms.
const ROUTINES: [(&str, Known); 10] = [
    ("strlen", ReadsArguments),
    ("strcmp", ThreeWay),
    ("strncmp", ThreeWay),
    ("memcmp", ThreeWay),
    ("_fstrcmp", ThreeWay),
    ("_fmemcmp", ThreeWay),
    ("setjmp", ReturnsTwice),
    ("sigsetjmp", ReturnsTwice),
    ("savectx", ReturnsTwice),
    ("vfork", ReturnsTwice),
];

fn known(name: &str, fact: Known) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    ROUTINES.contains(&(name, fact))
}

/// Of `names`, the functions that only read what their pointer arguments
/// reach and keep none of them.
pub fn reads_arguments<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    names.into_iter().filter(|name| known(name, ReadsArguments)).map(str::to_owned).collect()
}

/// Whether the function of object name `name` returns a three-way compare.
pub fn three_way_compare(name: &str) -> bool {
    known(name, ThreeWay)
}

/// Whether the function of object name `name` returns twice.
pub fn returns_twice(name: &str) -> bool {
    known(name, ReturnsTwice)
}

#[cfg(test)]
mod tests {
    /// Exactly one decoration underscore comes off: `__strlen` is not strlen.
    #[test]
    fn names_lose_exactly_one_decoration_underscore() {
        assert_eq!(super::reads_arguments(["_strlen", "__strlen", "strlen", "_puts"]), ["_strlen", "strlen"]);
    }

    /// `setjmp` returns twice by its name, with or without the C decoration.
    #[test]
    fn setjmp_is_known_to_return_twice() {
        assert!(super::returns_twice("_setjmp") && super::returns_twice("setjmp") && super::returns_twice("_sigsetjmp") && super::returns_twice("vfork"));
        assert!(!super::returns_twice("_longjmp") && !super::returns_twice("_setjmp2"));
    }
}
