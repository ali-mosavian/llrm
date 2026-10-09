//! An integer wider than the target's widest native one, split into halves of
//! that width: LLVM's `ExpandIntegerResult`. Which halves an operation makes,
//! and which of them take the carry (or borrow) of the one before, is
//! arithmetic and the same for every target; the target maps each half to a
//! form (x86's `add` then `adc`).

/// The operations whose halves are independent or chained by a carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Add,
    Sub,
    And,
    Or,
    Xor,
}

impl Kind {
    /// Whether a half is computed from the carry of the half below it.
    pub fn carries(self) -> bool {
        matches!(self, Self::Add | Self::Sub)
    }
}

/// One half of an expanded operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Half {
    /// Its place, low half first.
    pub index: usize,
    /// It takes the carry (borrow) of the half below it.
    pub carried: bool,
}

/// The halves of `kind` on `bits`-wide integers where `legal` is the widest
/// native width, low first: none where `bits` is native or not a whole number
/// of halves.
pub fn expand_wide(
    kind: Kind,
    bits: u32,
    legal: u32,
) -> Vec<Half> {
    if legal == 0 || bits <= legal || bits % legal != 0 {
        return Vec::new();
    }
    (0..(bits / legal) as usize).map(|index| Half { index, carried: kind.carries() && index > 0 }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule is the width's: i64 on a 32-bit machine, i32 on a 16-bit one
    /// and i128 on a 64-bit one are each two halves, i128 on 32 bits four,
    /// and only an add or a subtraction chains a carry.
    #[test]
    fn a_wide_operation_is_as_many_halves_as_the_native_width_goes_into_it() {
        let halves = |kind, bits, legal| expand_wide(kind, bits, legal).len();
        assert_eq!((halves(Kind::Add, 64, 32), halves(Kind::Add, 32, 16), halves(Kind::Add, 128, 64)), (2, 2, 2));
        assert_eq!(halves(Kind::Xor, 128, 32), 4);
        assert_eq!((halves(Kind::Add, 32, 32), halves(Kind::Add, 8, 32), halves(Kind::Add, 48, 32)), (0, 0, 0));
        let chain = expand_wide(Kind::Sub, 128, 32);
        assert_eq!(chain.iter().map(|half| half.carried).collect::<Vec<_>>(), [false, true, true, true]);
        assert!(expand_wide(Kind::And, 64, 32).iter().all(|half| !half.carried));
    }
}
