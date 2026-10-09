//! An integer wider than the target's widest native one, split into halves of
//! that width: LLVM's `ExpandIntegerResult`. Which halves an operation makes,
//! and which of them take the carry (or borrow) of the one before, is
//! arithmetic and the same for every target; the target maps each half to a
//! form (x86's `add` then `adc`).

/// One half of an expanded operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Half {
    /// Its place, low half first.
    pub index: usize,
    /// Its bytes' distance from the integer's first, in memory (little-endian:
    /// low half first).
    pub offset: i64,
    /// Its width in bytes.
    pub bytes: u32,
    /// It takes the carry (borrow) of the half below it.
    pub carried: bool,
}

/// The halves of an operation on `bits`-wide integers where `legal` is the
/// widest native width, low first, each above the lowest taking the carry
/// (borrow) of the one below where `chained` (an add or a subtraction): none
/// where `bits` is native or not a whole number of halves.
pub fn expand_wide(
    chained: bool,
    bits: u32,
    legal: u32,
) -> Vec<Half> {
    if legal == 0 || bits <= legal || bits % legal != 0 {
        return Vec::new();
    }
    (0..(bits / legal) as usize)
        .map(|index| Half {
            index,
            offset: index as i64 * i64::from(legal / 8),
            bytes: legal / 8,
            carried: chained && index > 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule is the width's: i64 on a 32-bit machine, i32 on a 16-bit one
    /// and i128 on a 64-bit one are each two halves, i128 on 32 bits four,
    /// and only an add or a subtraction chains a carry.
    #[test]
    fn a_wide_operation_is_as_many_halves_as_the_native_width_goes_into_it() {
        let halves = |chained, bits, legal| expand_wide(chained, bits, legal).len();
        assert_eq!((halves(true, 64, 32), halves(true, 32, 16), halves(true, 128, 64)), (2, 2, 2));
        assert_eq!(halves(false, 128, 32), 4);
        assert_eq!((halves(true, 32, 32), halves(true, 8, 32), halves(true, 48, 32)), (0, 0, 0));
        let chain = expand_wide(true, 128, 32);
        assert_eq!(chain.iter().map(|half| half.carried).collect::<Vec<_>>(), [false, true, true, true]);
        assert!(expand_wide(false, 64, 32).iter().all(|half| !half.carried));
        let places = |bits, legal| {
            expand_wide(false, bits, legal).iter().map(|half| (half.offset, half.bytes)).collect::<Vec<_>>()
        };
        assert_eq!((places(64, 32), places(32, 16)), (vec![(0, 4), (4, 4)], vec![(0, 2), (2, 2)]));
    }
}
