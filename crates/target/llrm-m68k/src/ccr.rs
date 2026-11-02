//! What each instruction does to the condition codes, from the manual's
//! "Condition Codes" blocks (`isa/ccr.tsv`).
//!
//! # How it works
//! [`like_test`] names the flags an instruction sets as `tst` of its result
//! would: N and Z where the manual says "according to the result", V and C
//! where it says cleared. A branch that tests only those flags reads the same
//! after the instruction as after a `tst` of its result.
//!
//! # See also
//! `X86InstrInfo::optimizeCompareInstr`, which asks the same of
//! `isDefConvertible` and the flags each user reads.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::target::DIALECT;

/// One row of `ccr.tsv`: the symbols for X, N, Z, V and C.
type Row = [char; 5];

static ROWS: LazyLock<HashMap<&'static str, Row>> = LazyLock::new(|| {
    include_str!("isa/ccr.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut cells = line.split('\t');
            let mnemonic = cells.next().expect("a mnemonic");
            let mut row = [' '; 5];
            for slot in &mut row {
                *slot = cells.next().and_then(|cell| cell.chars().next()).expect("five symbols");
            }
            (mnemonic, row)
        })
        .collect()
});

/// The bits of the flags `name` (`C`, `V`, `Z`, `N`, `X`) stands for.
pub fn flag_bit(name: &str) -> u32 {
    DIALECT.flags.iter().find(|(one, _)| one == name).map(|(_, bit)| *bit).expect("a flag of the dialect")
}

/// The flags an instruction named by `mnemonic` leaves as `tst` of its result
/// would, or `None` for an instruction the table does not describe.
pub fn like_test(mnemonic: &str) -> Option<u32> {
    let row = ROWS.get(mnemonic)?;
    let same = [("N", row[1] == '*'), ("Z", row[2] == '*'), ("V", row[3] == '0'), ("C", row[4] == '0')];
    Some(same.iter().filter(|(_, yes)| *yes).map(|(flag, _)| flag_bit(flag)).fold(0, |all, bit| all | bit))
}

/// The flags condition `code` (an `M68k::CondCode`) tests.
pub fn tested_by(code: i64) -> Option<u32> {
    DIALECT.conditions.get(usize::try_from(code).ok()?).map(|(_, flags)| *flags)
}

/// The flags the condition named `name` (`ne`, `cc`...) tests.
pub fn tested_by_name(name: &str) -> Option<u32> {
    DIALECT.conditions.iter().find(|(one, _)| one == name).map(|(_, flags)| *flags)
}

/// Pairs of conditions that hold of the same operands of a compare when the
/// operands change places: `a < b` is `b > a`. The rest (`mi`, `pl`, `vs`,
/// `vc`) read flags that depend on the order.
const SWAPPED: [(&str, &str); 6] = [("lt", "gt"), ("le", "ge"), ("cs", "hi"), ("ls", "cc"), ("eq", "eq"), ("ne", "ne")];

/// The condition that holds on swapped operands where `name` held.
pub fn swapped_name(name: &str) -> Option<&'static str> {
    SWAPPED
        .iter()
        .find_map(
            |&(a, b)| match name {
                _ if name == a => Some(b),
                _ if name == b => Some(a),
                _ => None,
            },
        )
}

/// [`swapped_name`] for a condition code (an `M68k::CondCode`).
pub fn swapped_code(code: i64) -> Option<i64> {
    let name = &DIALECT.conditions.get(usize::try_from(code).ok()?)?.0;
    let swapped = swapped_name(name)?;
    DIALECT.conditions.iter().position(|(one, _)| one == swapped).map(|at| at as i64)
}
