//! `BlockFrequency`, a translation of
//! llvm/include/llvm/Support/BlockFrequency.h and llvm/lib/Support/
//! BlockFrequency.cpp at d1106deb71cc: a saturating u64.

use super::branchprobability::BranchProbability;

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/BlockFrequency.h#L27>
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockFrequency(pub u64);

impl BlockFrequency {
    pub fn max() -> Self {
        Self(u64::MAX)
    }

    pub fn get_frequency(self) -> u64 {
        self.0
    }

    /// `mul`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BlockFrequency.cpp#L43>
    pub fn mul_u64(
        self,
        factor: u64,
    ) -> Option<Self> {
        self.0.checked_mul(factor).map(Self)
    }

    /// `operator>>=`: never shifts to zero.
    pub fn shr_nonzero(
        self,
        count: u32,
    ) -> Self {
        assert!(self.0 != 0);
        let shifted = self.0 >> count;
        Self(shifted | (shifted == 0) as u64)
    }
}

impl std::ops::Mul<BranchProbability> for BlockFrequency {
    type Output = Self;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BlockFrequency.cpp#L21>
    fn mul(
        self,
        prob: BranchProbability,
    ) -> Self {
        Self(prob.scale(self.0))
    }
}

impl std::ops::Div<BranchProbability> for BlockFrequency {
    type Output = Self;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BlockFrequency.cpp#L32>
    fn div(
        self,
        prob: BranchProbability,
    ) -> Self {
        Self(prob.scale_by_inverse(self.0))
    }
}

impl std::ops::Add for BlockFrequency {
    type Output = Self;

    fn add(
        self,
        rhs: Self,
    ) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for BlockFrequency {
    type Output = Self;

    fn sub(
        self,
        rhs: Self,
    ) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl std::fmt::Display for BlockFrequency {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
