//! `computeKnownFPClass` against what LLVM answered on the same IR: the lines
//! of `tests/vt/fp/NAME.expected` are `tools/llvmport/fpdump.sh NAME.ll`, and
//! `NAME.deviates` lists the values llrm answers differently (a DEVIATE of the
//! `fp` header: none now). The set of lines
//! that differ must be exactly that list: a fold that closes a gap fails here
//! until its line is removed.

use std::collections::BTreeSet;

use super::fp::dump;
use crate::parse;

macro_rules! cases {
    ($($name:ident),*) => {
        const CASES: &[(&str, &str, &str, &str)] = &[$((
            stringify!($name),
            include_str!(concat!("../../tests/vt/fp/", stringify!($name), ".ll")),
            include_str!(concat!("../../tests/vt/fp/", stringify!($name), ".expected")),
            include_str!(concat!("../../tests/vt/fp/", stringify!($name), ".deviates")),
        )),*];
    };
}

cases!(ops, intrinsics, context, bitcast);

#[test]
fn llvm_and_llrm_know_the_same_classes_but_the_listed_values() {
    for (name, text, expected, deviates) in CASES {
        let module = parse::module(text).expect("parses");
        let ours = dump(&module, true);
        let (ours, expected): (Vec<_>, Vec<_>) = (ours.lines().collect(), expected.lines().collect());
        assert_eq!(ours.len(), expected.len(), "{name}: values");
        let key = |line: &str| line.split(' ').take(2).collect::<Vec<_>>().join(" ");
        let differing: BTreeSet<String> =
            ours.iter().zip(&expected).filter(|(mine, llvm)| mine != llvm).map(|(mine, _)| key(mine)).collect();
        let listed: BTreeSet<String> = deviates.lines().map(str::to_owned).collect();
        assert_eq!(differing, listed, "{name}: values that differ against {name}.deviates");
    }
}
