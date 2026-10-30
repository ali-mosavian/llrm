//! `ScaledNumber<uint64_t>` (`Scaled64`) and the `ScaledNumbers` helpers it
//! calls, a translation of llvm/include/llvm/Support/ScaledNumber.h and
//! llvm/lib/Support/ScaledNumber.cpp at d1106deb71cc: an unsigned float of 64
//! digits and a 16-bit binary exponent.
//!
//! DEVIATE 1: only the 64-bit digits BlockFrequencyInfoImpl uses, so the
//! 32-bit paths (`divide32`, `getAdjusted<uint32_t>`) are left out.
//! DEVIATE 2: no `toString`/`print` (APFloat formatting); nothing here prints
//! a scaled number.

const MAX_SCALE: i32 = 16383;
const MIN_SCALE: i32 = -16382;
const WIDTH: i32 = 64;

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L53>
fn get_rounded(
    digits: u64,
    scale: i16,
    should_round: bool,
) -> (u64, i16) {
    if should_round {
        let digits = digits.wrapping_add(1);
        if digits == 0 {
            return (1 << (WIDTH - 1), scale + 1);
        }
        return (digits, scale);
    }
    (digits, scale)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/ScaledNumber.cpp#L22>
fn multiply64(
    lhs: u64,
    rhs: u64,
) -> (u64, i16) {
    let product = lhs as u128 * rhs as u128;
    let mut upper = (product >> 64) as u64;
    let lower = product as u64;
    if upper == 0 {
        return (lower, 0);
    }
    let leading_zeros = upper.leading_zeros();
    let shift = 64 - leading_zeros as i32;
    if leading_zeros != 0 {
        upper = upper << leading_zeros | lower >> shift;
    }
    get_rounded(upper, shift as i16, shift != 0 && lower & 1 << (shift - 1) != 0)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L115>
fn get_product(
    lhs: u64,
    rhs: u64,
) -> (u64, i16) {
    if lhs <= u32::MAX as u64 && rhs <= u32::MAX as u64 {
        return (lhs * rhs, 0);
    }
    multiply64(lhs, rhs)
}

fn get_half(n: u64) -> u64 {
    (n >> 1) + (n & 1)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/ScaledNumber.cpp#L89>
fn divide64(
    mut dividend: u64,
    mut divisor: u64,
) -> (u64, i16) {
    assert!(dividend != 0, "expected non-zero dividend");
    assert!(divisor != 0, "expected non-zero divisor");
    let mut shift = 0i32;
    let zeros = divisor.trailing_zeros();
    if zeros != 0 {
        shift -= zeros as i32;
        divisor >>= zeros;
    }
    if divisor == 1 {
        return (dividend, shift as i16);
    }
    let zeros = dividend.leading_zeros();
    if zeros != 0 {
        shift -= zeros as i32;
        dividend <<= zeros;
    }
    let mut quotient = dividend / divisor;
    dividend %= divisor;
    while quotient >> 63 == 0 && dividend != 0 {
        let is_overflow = dividend >> 63 != 0;
        dividend <<= 1;
        shift -= 1;
        quotient <<= 1;
        if is_overflow || divisor <= dividend {
            quotient |= 1;
            dividend = dividend.wrapping_sub(divisor);
        }
    }
    get_rounded(quotient, shift as i16, dividend >= get_half(divisor))
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L156>
fn get_quotient(
    dividend: u64,
    divisor: u64,
) -> (u64, i16) {
    if dividend == 0 {
        return (0, 0);
    }
    if divisor == 0 {
        return (u64::MAX, MAX_SCALE as i16);
    }
    divide64(dividend, divisor)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L191>
fn get_lg_impl(
    digits: u64,
    scale: i16,
) -> (i32, i32) {
    if digits == 0 {
        return (i32::MIN, 0);
    }
    let local_floor = 63 - digits.leading_zeros() as i32;
    let floor = scale as i32 + local_floor;
    if digits == 1 << local_floor {
        return (floor, 0);
    }
    assert!(local_floor >= 1);
    let round = digits & 1 << (local_floor - 1) != 0;
    (floor + round as i32, if round { 1 } else { -1 })
}

fn get_lg_floor(
    digits: u64,
    scale: i16,
) -> i32 {
    let (lg, error) = get_lg_impl(digits, scale);
    lg - (error > 0) as i32
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/ScaledNumber.cpp#L133>
fn compare_impl(
    l: u64,
    r: u64,
    scale_diff: i32,
) -> i32 {
    assert!(scale_diff >= 0, "wrong argument order");
    assert!(scale_diff < 64, "numbers too far apart");
    let l_adjusted = l >> scale_diff;
    if l_adjusted < r {
        return -1;
    }
    if l_adjusted > r {
        return 1;
    }
    (l > l_adjusted << scale_diff) as i32
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L255>
fn compare(
    l_digits: u64,
    l_scale: i16,
    r_digits: u64,
    r_scale: i16,
) -> i32 {
    if l_digits == 0 {
        return if r_digits != 0 { -1 } else { 0 };
    }
    if r_digits == 0 {
        return 1;
    }
    let (lg_l, lg_r) = (get_lg_floor(l_digits, l_scale), get_lg_floor(r_digits, r_scale));
    if lg_l != lg_r {
        return if lg_l < lg_r { -1 } else { 1 };
    }
    if l_scale < r_scale {
        return compare_impl(l_digits, r_digits, r_scale as i32 - l_scale as i32);
    }
    -compare_impl(r_digits, l_digits, l_scale as i32 - r_scale as i32)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L290>
fn match_scales(
    l_digits: &mut u64,
    l_scale: &mut i16,
    r_digits: &mut u64,
    r_scale: &mut i16,
) -> i16 {
    if *l_scale < *r_scale {
        return match_scales(r_digits, r_scale, l_digits, l_scale);
    }
    if *l_digits == 0 {
        return *r_scale;
    }
    if *r_digits == 0 || *l_scale == *r_scale {
        return *l_scale;
    }
    let scale_diff = *l_scale as i32 - *r_scale as i32;
    if scale_diff >= 2 * WIDTH {
        *r_digits = 0;
        return *l_scale;
    }
    let shift_l = (l_digits.leading_zeros() as i32).min(scale_diff);
    assert!(shift_l < WIDTH, "can't shift more than width");
    let shift_r = scale_diff - shift_l;
    if shift_r >= WIDTH {
        *r_digits = 0;
        return *l_scale;
    }
    *l_digits <<= shift_l;
    *r_digits >>= shift_r;
    *l_scale -= shift_l as i16;
    *r_scale += shift_r as i16;
    assert!(*l_scale == *r_scale, "scales should match");
    *l_scale
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L336>
fn get_sum(
    mut l_digits: u64,
    mut l_scale: i16,
    mut r_digits: u64,
    mut r_scale: i16,
) -> (u64, i16) {
    assert!(l_scale < i16::MAX && r_scale < i16::MAX, "scale too large");
    let scale = match_scales(&mut l_digits, &mut l_scale, &mut r_digits, &mut r_scale);
    let sum = l_digits.wrapping_add(r_digits);
    if sum >= r_digits {
        return (sum, scale);
    }
    let high_bit = 1u64 << (WIDTH - 1);
    (high_bit | sum >> 1, scale + 1)
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L376>
fn get_difference(
    mut l_digits: u64,
    mut l_scale: i16,
    mut r_digits: u64,
    mut r_scale: i16,
) -> (u64, i16) {
    let (saved_r_digits, saved_r_scale) = (r_digits, r_scale);
    match_scales(&mut l_digits, &mut l_scale, &mut r_digits, &mut r_scale);
    if l_digits <= r_digits {
        return (0, 0);
    }
    if r_digits != 0 || saved_r_digits == 0 {
        return (l_digits - r_digits, l_scale);
    }
    let r_lg_floor = get_lg_floor(saved_r_digits, saved_r_scale);
    if compare(l_digits, l_scale, 1, (r_lg_floor + WIDTH) as i16) == 0 {
        return (u64::MAX, r_lg_floor as i16);
    }
    (l_digits, l_scale)
}

/// `ScaledNumber<uint64_t>`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L496>
#[derive(Clone, Copy, Debug, Default)]
pub struct Scaled64 {
    digits: u64,
    scale: i16,
}

impl Scaled64 {
    pub const fn new(
        digits: u64,
        scale: i16,
    ) -> Self {
        Self { digits, scale }
    }

    fn of((digits, scale): (u64, i16)) -> Self {
        Self { digits, scale }
    }

    pub fn get_zero() -> Self {
        Self::new(0, 0)
    }

    pub fn get_one() -> Self {
        Self::new(1, 0)
    }

    pub fn get_largest() -> Self {
        Self::new(u64::MAX, MAX_SCALE as i16)
    }

    /// `get(N)`: `adjustToWidth(N, 0)`, which for 64 digits is `N` itself.
    pub fn get(n: u64) -> Self {
        Self::new(n, 0)
    }

    pub fn get_inverse(n: u64) -> Self {
        Self::get(n).inverse()
    }

    pub fn get_fraction(
        n: u64,
        d: u64,
    ) -> Self {
        Self::of(get_quotient(n, d))
    }

    pub fn get_scale(self) -> i16 {
        self.scale
    }

    pub fn get_digits(self) -> u64 {
        self.digits
    }

    pub fn is_zero(self) -> bool {
        self.digits == 0
    }

    pub fn is_largest(self) -> bool {
        self == Self::get_largest()
    }

    pub fn lg_floor(self) -> i32 {
        get_lg_floor(self.digits, self.scale)
    }

    pub fn compare(
        self,
        x: Self,
    ) -> i32 {
        compare(self.digits, self.scale, x.digits, x.scale)
    }

    pub fn compare_to(
        self,
        n: u64,
    ) -> i32 {
        compare(self.digits, self.scale, n, 0)
    }

    pub fn inverse(self) -> Self {
        Self::get(1) / self
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L774>
    pub fn scale_u64(
        self,
        n: u64,
    ) -> u64 {
        (Self::get(n) * self).to_int()
    }

    /// `toInt<uint64_t>`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L784>
    pub fn to_int(self) -> u64 {
        if self.compare_to(1) < 0 {
            return 0;
        }
        if self.compare_to(u64::MAX) >= 0 {
            return u64::MAX;
        }
        let n = self.digits;
        if self.scale > 0 {
            assert!((self.scale as u32) < 64);
            return n << self.scale;
        }
        if self.scale < 0 {
            assert!((-(self.scale as i32) as u32) < 64);
            return n >> -self.scale;
        }
        n
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L837>
    fn shift_left(
        &mut self,
        mut shift: i32,
    ) {
        if shift == 0 || self.is_zero() {
            return;
        }
        assert!(shift != i32::MIN);
        if shift < 0 {
            self.shift_right(-shift);
            return;
        }
        let scale_shift = shift.min(MAX_SCALE - self.scale as i32);
        self.scale += scale_shift as i16;
        if scale_shift == shift {
            return;
        }
        if self.is_largest() {
            return;
        }
        shift -= scale_shift;
        if shift > self.digits.leading_zeros() as i32 {
            *self = Self::get_largest();
            return;
        }
        self.digits <<= shift;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L867>
    fn shift_right(
        &mut self,
        mut shift: i32,
    ) {
        if shift == 0 || self.is_zero() {
            return;
        }
        assert!(shift != i32::MIN);
        if shift < 0 {
            self.shift_left(-shift);
            return;
        }
        let scale_shift = shift.min(self.scale as i32 - MIN_SCALE);
        self.scale -= scale_shift as i16;
        if scale_shift == shift {
            return;
        }
        shift -= scale_shift;
        if shift >= WIDTH {
            *self = Self::get_zero();
            return;
        }
        self.digits >>= shift;
    }
}

impl PartialEq for Scaled64 {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.compare(*other) == 0
    }
}

impl PartialOrd for Scaled64 {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.compare(*other).cmp(&0))
    }
}

impl std::ops::Add for Scaled64 {
    type Output = Self;

    fn add(
        self,
        x: Self,
    ) -> Self {
        let sum = Self::of(get_sum(self.digits, self.scale, x.digits, x.scale));
        if sum.scale as i32 > MAX_SCALE { Self::get_largest() } else { sum }
    }
}

impl std::ops::AddAssign for Scaled64 {
    fn add_assign(
        &mut self,
        x: Self,
    ) {
        *self = *self + x;
    }
}

impl std::ops::Sub for Scaled64 {
    type Output = Self;

    fn sub(
        self,
        x: Self,
    ) -> Self {
        Self::of(get_difference(self.digits, self.scale, x.digits, x.scale))
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl std::ops::Mul for Scaled64 {
    type Output = Self;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L805>
    fn mul(
        self,
        x: Self,
    ) -> Self {
        if self.is_zero() {
            return self;
        }
        if x.is_zero() {
            return x;
        }
        let scales = self.scale as i32 + x.scale as i32;
        let mut out = Self::of(get_product(self.digits, x.digits));
        out.shift_left(scales);
        out
    }
}

impl std::ops::MulAssign for Scaled64 {
    fn mul_assign(
        &mut self,
        x: Self,
    ) {
        *self = *self * x;
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl std::ops::Div for Scaled64 {
    type Output = Self;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Support/ScaledNumber.h#L822>
    fn div(
        self,
        x: Self,
    ) -> Self {
        if self.is_zero() {
            return self;
        }
        if x.is_zero() {
            return Self::get_largest();
        }
        let scales = self.scale as i32 - x.scale as i32;
        let mut out = Self::of(get_quotient(self.digits, x.digits));
        out.shift_left(scales);
        out
    }
}

impl std::ops::Shl<i16> for Scaled64 {
    type Output = Self;

    fn shl(
        mut self,
        shift: i16,
    ) -> Self {
        self.shift_left(shift as i32);
        self
    }
}

impl std::ops::Shr<i16> for Scaled64 {
    type Output = Self;

    fn shr(
        mut self,
        shift: i16,
    ) -> Self {
        self.shift_right(shift as i32);
        self
    }
}
