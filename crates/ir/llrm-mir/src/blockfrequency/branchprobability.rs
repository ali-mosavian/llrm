//! `BranchProbability`, a translation of
//! llvm/include/llvm/Support/BranchProbability.h and llvm/lib/Support/
//! BranchProbability.cpp at d1106deb71cc: a probability as a numerator over
//! 2^31.

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/BranchProbability.h#L32>
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BranchProbability {
    n: u32,
}

const D: u32 = 1 << 31;
const UNKNOWN_N: u32 = u32::MAX;

impl Default for BranchProbability {
    fn default() -> Self {
        Self::get_unknown()
    }
}

impl BranchProbability {
    /// `BranchProbability(Numerator, Denominator)`.
    pub fn new(
        numerator: u32,
        denominator: u32,
    ) -> Self {
        assert!(denominator > 0, "Denominator cannot be 0!");
        assert!(numerator <= denominator, "Probability cannot be bigger than 1!");
        if denominator == D {
            return Self { n: numerator };
        }
        let prob64 = (numerator as u64 * D as u64 + denominator as u64 / 2) / denominator as u64;
        Self { n: prob64 as u32 }
    }

    pub fn is_zero(self) -> bool {
        self.n == 0
    }

    pub fn is_one(self) -> bool {
        self.n == D
    }

    pub fn is_unknown(self) -> bool {
        self.n == UNKNOWN_N
    }

    pub const fn get_zero() -> Self {
        Self { n: 0 }
    }

    pub const fn get_one() -> Self {
        Self { n: D }
    }

    pub const fn get_unknown() -> Self {
        Self { n: UNKNOWN_N }
    }

    pub const fn get_raw(n: u32) -> Self {
        Self { n }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L39>
    pub fn get_branch_probability(
        numerator: u64,
        mut denominator: u64,
    ) -> Self {
        assert!(numerator <= denominator, "Probability cannot be bigger than 1!");
        let mut scale = 0;
        while denominator > u32::MAX as u64 {
            denominator >>= 1;
            scale += 1;
        }
        Self::new((numerator >> scale) as u32, denominator as u32)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L51>
    pub fn get_branch_probability_f64(prob: f64) -> Self {
        assert!((0.0..=1.0).contains(&prob), "Probability must be between 0 and 1!");
        Self::new((prob * D as f64).round() as u32, D)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/BranchProbability.h#L235>
    #[allow(clippy::manual_checked_ops)]
    pub fn normalize_probabilities(probs: &mut [BranchProbability]) {
        if probs.is_empty() {
            return;
        }
        let mut unknown = 0u64;
        let mut sum = 0u64;
        for one in probs.iter() {
            if one.is_unknown() {
                unknown += 1;
            } else {
                sum += one.n as u64;
            }
        }
        if unknown > 0 {
            let for_unknown =
                if sum < D as u64 { Self::get_raw(((D as u64 - sum) / unknown) as u32) } else { Self::get_zero() };
            for one in probs.iter_mut().filter(|one| one.is_unknown()) {
                *one = for_unknown;
            }
            if sum <= D as u64 {
                return;
            }
        }
        if sum == 0 {
            let each = Self::new(1, probs.len() as u32);
            probs.fill(each);
            return;
        }
        for one in probs.iter_mut() {
            one.n = ((one.n as u64 * D as u64 + sum / 2) / sum) as u32;
        }
    }

    pub fn get_numerator(self) -> u32 {
        self.n
    }

    pub fn get_denominator() -> u32 {
        D
    }

    pub fn to_f64(self) -> f64 {
        self.n as f64 / D as f64
    }

    pub fn get_compl(self) -> Self {
        Self { n: D - self.n }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L98>
    pub fn scale(
        self,
        num: u64,
    ) -> u64 {
        scale(num, self.n, D)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L102>
    pub fn scale_by_inverse(
        self,
        num: u64,
    ) -> u64 {
        scale(num, D, self.n)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L106>
    pub fn pow(
        self,
        n: u32,
    ) -> Self {
        let mut res = Self::get_one();
        for _ in 0..n {
            res = res * self;
        }
        res
    }

    fn known(self) {
        assert!(!self.is_unknown(), "Unknown probability cannot participate in arithmetics.");
    }

    pub fn mul_u32(
        self,
        rhs: u32,
    ) -> Self {
        self.known();
        let n = self.n as u64 * rhs as u64;
        Self { n: if n > D as u64 { D } else { n as u32 } }
    }

    pub fn div_u32(
        self,
        rhs: u32,
    ) -> Self {
        self.known();
        assert!(rhs > 0, "The divider cannot be zero.");
        Self { n: self.n / rhs }
    }
}

/// `scale<ConstD>`: `num * n / d` without overflow, saturating.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L60>
fn scale(
    num: u64,
    n: u32,
    d: u32,
) -> u64 {
    assert!(d != 0, "divide by 0");
    if num == 0 || d == n {
        return num;
    }
    let product_high = (num >> 32) * n as u64;
    let product_low = (num & u32::MAX as u64) * n as u64;
    let mut upper32 = (product_high >> 32) as u32;
    let lower32 = (product_low & u32::MAX as u64) as u32;
    let mid32_partial = (product_high & u32::MAX as u64) as u32;
    let mid32 = mid32_partial.wrapping_add((product_low >> 32) as u32);
    upper32 += (mid32 < mid32_partial) as u32;
    let rem = ((upper32 as u64) << 32) | mid32 as u64;
    let upper_q = rem / d as u64;
    if upper_q > u32::MAX as u64 {
        return u64::MAX;
    }
    let rem = ((rem % d as u64) << 32) | lower32 as u64;
    let lower_q = rem / d as u64;
    let q = (upper_q << 32).wrapping_add(lower_q);
    if q < lower_q { u64::MAX } else { q }
}

impl std::ops::Add for BranchProbability {
    type Output = Self;

    fn add(
        self,
        rhs: Self,
    ) -> Self {
        self.known();
        rhs.known();
        Self { n: if self.n as u64 + rhs.n as u64 > D as u64 { D } else { self.n + rhs.n } }
    }
}

impl std::ops::Sub for BranchProbability {
    type Output = Self;

    fn sub(
        self,
        rhs: Self,
    ) -> Self {
        self.known();
        rhs.known();
        Self { n: self.n.saturating_sub(rhs.n) }
    }
}

impl std::ops::Mul for BranchProbability {
    type Output = Self;

    fn mul(
        self,
        rhs: Self,
    ) -> Self {
        self.known();
        rhs.known();
        Self { n: ((self.n as u64 * rhs.n as u64 + D as u64 / 2) / D as u64) as u32 }
    }
}

impl std::ops::Div for BranchProbability {
    type Output = Self;

    fn div(
        self,
        rhs: Self,
    ) -> Self {
        self.known();
        rhs.known();
        Self { n: ((self.n as u64 * D as u64 + rhs.n as u64 / 2) / rhs.n as u64) as u32 }
    }
}

impl std::ops::AddAssign for BranchProbability {
    fn add_assign(
        &mut self,
        rhs: Self,
    ) {
        *self = *self + rhs;
    }
}

impl PartialOrd for BranchProbability {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        self.known();
        other.known();
        Some(self.n.cmp(&other.n))
    }
}

impl std::fmt::Display for BranchProbability {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/BranchProbability.cpp#L23>
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        if self.is_unknown() {
            return write!(f, "?%");
        }
        let percent = ((self.n as f64 / D as f64) * 100.0 * 100.0).round_ties_even() / 100.0;
        write!(f, "0x{:08x} / 0x{:08x} = {percent:.2}%", self.n, D)
    }
}
