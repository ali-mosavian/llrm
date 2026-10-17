//! `p2` and `p2z` against LLVM's answers on `tests/vt/nonzero/pow2.ll`; see
//! `nonzero_tests` for `pending`.

use super::nonzero_tests::check;

#[test]
fn powers_of_two_are_as_llvm_says() {
    check(
        include_str!("../../tests/vt/nonzero/pow2.ll"),
        include_str!("../../tests/vt/nonzero/pow2.expected"),
        &[
            ("shifts", 3),
            ("shifts", 4),
            ("shifts", 7),
            ("shifts", 8),
            ("shifts", 10),
            ("shifts", 12),
            ("shifts", 13),
            ("shifts", 17),
            ("shifts", 19),
            ("shifts", 22),
            ("ands", 3),
            ("ands", 10),
            ("ands", 11),
            ("ands", 13),
            ("ands", 15),
            ("ands", 19),
            ("ands", 23),
            ("selects", 4),
            ("selects", 5),
            ("selects", 6),
            ("selects", 7),
            ("selects", 8),
            ("intrin", 3),
            ("intrin", 4),
            ("intrin", 5),
            ("intrin", 6),
            ("intrin", 8),
            ("intrin", 9),
            ("intrin", 10),
            ("phis", 4),
            ("phis", 5),
            ("phis", 6),
            ("phis", 8),
        ],
    );
}
