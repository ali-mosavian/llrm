//! The floating-point class masks and modes: LLVM's `FPClassTest`,
//! `DenormalMode` and `RoundingMode`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` (`llvm/include/llvm/ADT/
//! FloatingPointMode.h`, `llvm/lib/Support/FloatingPointMode.cpp`, and
//! `invertFPClassTestIfSimpler` of `llvm/lib/CodeGen/CodeGenCommonISel.cpp`):
//! same operations, same order of cases. Each function links the line it
//! translates.
//!
//! [`FpClassTest`] is the ten-bit mask; `!` is `operator~` of
//! `LLVM_DECLARE_ENUM_AS_BITMASK`, which stays inside the ten bits.
//!
//! DEVIATE: `operator<<(raw_ostream &, FPClassTest)` and
//! `DenormalMode::print` return a `String`; `raw_ostream` does not exist here
//! (FloatingPointMode.cpp:76-98, FloatingPointMode.h:204-206).
//!
//! # Example
//! `FC_NEG_INF | FC_NEG_NORMAL` is `0x000c`; `fneg` of it is
//! `FC_POS_INF | FC_POS_NORMAL`, and `ordered_strictly_less(FC_POSITIVE)` is
//! `FC_NEGATIVE`.
//!
//! # See also
//! - Used by: [`crate::apfloat`], [`crate::knownfpclass`]

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, Not};

/// `RoundingMode`, with LLVM's numbering.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L38>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i8)]
pub enum RoundingMode {
    TowardZero = 0,
    NearestTiesToEven = 1,
    TowardPositive = 2,
    TowardNegative = 3,
    NearestTiesToAway = 4,
    Dynamic = 7,
    Invalid = -1,
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L52>
pub fn spell(rounding: RoundingMode) -> &'static str {
    match rounding {
        RoundingMode::TowardZero => "towardzero",
        RoundingMode::NearestTiesToEven => "tonearest",
        RoundingMode::TowardPositive => "upward",
        RoundingMode::TowardNegative => "downward",
        RoundingMode::NearestTiesToAway => "tonearestaway",
        RoundingMode::Dynamic => "dynamic",
        RoundingMode::Invalid => "invalid",
    }
}

/// `DenormalMode::DenormalModeKind`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L74>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i8)]
pub enum DenormalModeKind {
    Invalid = -1,
    Ieee = 0,
    PreserveSign = 1,
    PositiveZero = 2,
    Dynamic = 3,
}

/// Subnormal handling for the inputs and the outputs of an instruction.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L71>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DenormalMode {
    pub output: DenormalModeKind,
    pub input: DenormalModeKind,
}

impl Default for DenormalMode {
    /// `DenormalMode()`: both invalid.
    fn default() -> Self {
        Self::get_invalid()
    }
}

impl DenormalMode {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L101>
    pub const fn new(
        output: DenormalModeKind,
        input: DenormalModeKind,
    ) -> Self {
        Self { output, input }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L106>
    pub const fn get_invalid() -> Self {
        Self::new(DenormalModeKind::Invalid, DenormalModeKind::Invalid)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L111>
    pub const fn get_default() -> Self {
        Self::get_ieee()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L115>
    pub const fn get_ieee() -> Self {
        Self::new(DenormalModeKind::Ieee, DenormalModeKind::Ieee)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L119>
    pub const fn get_preserve_sign() -> Self {
        Self::new(DenormalModeKind::PreserveSign, DenormalModeKind::PreserveSign)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L124>
    pub const fn get_positive_zero() -> Self {
        Self::new(DenormalModeKind::PositiveZero, DenormalModeKind::PositiveZero)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L129>
    pub const fn get_dynamic() -> Self {
        Self::new(DenormalModeKind::Dynamic, DenormalModeKind::Dynamic)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L133>
    pub fn to_int_value(&self) -> u32 {
        assert!(self.input != DenormalModeKind::Invalid && self.output != DenormalModeKind::Invalid);
        ((self.input as u32) << 2) | (self.output as u32)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L138>
    pub fn create_from_int_value(data: u32) -> Self {
        Self::new(kind_of(data & 0x3), kind_of((data >> 2) & 0x3))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L154>
    pub fn is_simple(&self) -> bool {
        self.input == self.output
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L156>
    pub fn is_valid(&self) -> bool {
        self.output != DenormalModeKind::Invalid && self.input != DenormalModeKind::Invalid
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L162>
    pub fn inputs_are_zero(&self) -> bool {
        self.input == DenormalModeKind::PreserveSign || self.input == DenormalModeKind::PositiveZero
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L168>
    pub fn inputs_may_be_zero(&self) -> bool {
        self.inputs_are_zero() || self.input == DenormalModeKind::Dynamic
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L173>
    pub fn outputs_are_zero(&self) -> bool {
        self.output == DenormalModeKind::PreserveSign || self.output == DenormalModeKind::PositiveZero
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L179>
    pub fn outputs_may_be_zero(&self) -> bool {
        self.outputs_are_zero() || self.output == DenormalModeKind::Dynamic
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L184>
    pub fn inputs_may_be_positive_zero(&self) -> bool {
        self.input == DenormalModeKind::PositiveZero || self.input == DenormalModeKind::Dynamic
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L190>
    pub fn outputs_may_be_positive_zero(&self) -> bool {
        self.output == DenormalModeKind::PositiveZero || self.output == DenormalModeKind::Dynamic
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L198>
    pub fn merge_callee_mode(
        &self,
        callee: DenormalMode,
    ) -> DenormalMode {
        let mut merged = callee;
        if callee.input == DenormalModeKind::Dynamic {
            merged.input = self.input;
        }
        if callee.output == DenormalModeKind::Dynamic {
            merged.output = self.output;
        }
        merged
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L269>
    pub fn print(
        &self,
        legacy: bool,
        omit_if_same: bool,
    ) -> String {
        let mut text = denormal_mode_kind_name(self.output, legacy).to_owned();
        if !omit_if_same || self.input != self.output {
            text.push(if legacy { ',' } else { '|' });
            text.push_str(denormal_mode_kind_name(self.input, legacy));
        }
        text
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L210>
    pub fn str(&self) -> String {
        self.print(true, false)
    }
}

fn kind_of(value: u32) -> DenormalModeKind {
    match value {
        0 => DenormalModeKind::Ieee,
        1 => DenormalModeKind::PreserveSign,
        2 => DenormalModeKind::PositiveZero,
        _ => DenormalModeKind::Dynamic,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L225>
pub fn parse_denormal_fp_attribute_component(text: &str) -> DenormalModeKind {
    match text {
        "" | "ieee" => DenormalModeKind::Ieee,
        "preservesign" | "preserve-sign" => DenormalModeKind::PreserveSign,
        "positivezero" | "positive-zero" => DenormalModeKind::PositiveZero,
        "dynamic" => DenormalModeKind::Dynamic,
        _ => DenormalModeKind::Invalid,
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L237>
pub fn denormal_mode_kind_name(
    mode: DenormalModeKind,
    legacy_name: bool,
) -> &'static str {
    match mode {
        DenormalModeKind::Ieee => "ieee",
        DenormalModeKind::PreserveSign => {
            if legacy_name {
                "preserve-sign"
            } else {
                "preservesign"
            }
        }
        DenormalModeKind::PositiveZero => {
            if legacy_name {
                "positive-zero"
            } else {
                "positivezero"
            }
        }
        DenormalModeKind::Dynamic => "dynamic",
        DenormalModeKind::Invalid => "",
    }
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L254>
pub fn parse_denormal_fp_attribute(text: &str) -> DenormalMode {
    let (output_text, input_text) = text.split_once(',').unwrap_or((text, ""));
    let output = parse_denormal_fp_attribute_component(output_text);
    let input = if input_text.is_empty() { output } else { parse_denormal_fp_attribute_component(input_text) };
    DenormalMode { output, input }
}

/// The denormal controls of a function: the default mode and the one of
/// `float`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L279>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DenormalFpEnv {
    pub default_mode: DenormalMode,
    pub f32_mode: DenormalMode,
}

impl DenormalFpEnv {
    const BITS_PER_MODE: u32 = 4;

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L289>
    pub fn new(
        base_mode: DenormalMode,
        float_mode: DenormalMode,
    ) -> Self {
        let output = if float_mode.output == DenormalModeKind::Invalid { base_mode.output } else { float_mode.output };
        let input = if float_mode.input == DenormalModeKind::Invalid { base_mode.input } else { float_mode.input };
        Self { default_mode: base_mode, f32_mode: DenormalMode::new(output, input) }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L297>
    pub fn get_default() -> Self {
        Self::new(DenormalMode::get_ieee(), DenormalMode::get_ieee())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L301>
    pub fn to_int_value(&self) -> u32 {
        assert!(self.default_mode.is_valid() && self.f32_mode.is_valid());
        let data = self.default_mode.to_int_value() | (self.f32_mode.to_int_value() << Self::BITS_PER_MODE);
        assert!(data < 256);
        data
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L310>
    pub fn create_from_int_value(data: u32) -> Self {
        Self::new(
            DenormalMode::create_from_int_value(data),
            DenormalMode::create_from_int_value(data >> Self::BITS_PER_MODE),
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L325>
    pub fn merge_callee_mode(
        &self,
        callee: DenormalFpEnv,
    ) -> DenormalFpEnv {
        DenormalFpEnv::new(
            self.default_mode.merge_callee_mode(callee.default_mode),
            self.f32_mode.merge_callee_mode(callee.f32_mode),
        )
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L110>
    pub fn print(
        &self,
        omit_if_same: bool,
    ) -> String {
        if self.f32_mode == self.default_mode {
            return self.default_mode.print(false, omit_if_same);
        }
        let mut text = String::new();
        if self.default_mode != DenormalMode::get_default() {
            text.push_str(&self.default_mode.print(false, omit_if_same));
            text.push_str(", ");
        }
        text.push_str("float: ");
        text.push_str(&self.f32_mode.print(false, omit_if_same));
        text
    }
}

/// A test of the class of a floating-point value: an OR of the ten basic ones.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/ADT/FloatingPointMode.h#L338>
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct FpClassTest(pub u32);

pub const FC_NONE: FpClassTest = FpClassTest(0);
pub const FC_SNAN: FpClassTest = FpClassTest(0x0001);
pub const FC_QNAN: FpClassTest = FpClassTest(0x0002);
pub const FC_NEG_INF: FpClassTest = FpClassTest(0x0004);
pub const FC_NEG_NORMAL: FpClassTest = FpClassTest(0x0008);
pub const FC_NEG_SUBNORMAL: FpClassTest = FpClassTest(0x0010);
pub const FC_NEG_ZERO: FpClassTest = FpClassTest(0x0020);
pub const FC_POS_ZERO: FpClassTest = FpClassTest(0x0040);
pub const FC_POS_SUBNORMAL: FpClassTest = FpClassTest(0x0080);
pub const FC_POS_NORMAL: FpClassTest = FpClassTest(0x0100);
pub const FC_POS_INF: FpClassTest = FpClassTest(0x0200);
pub const FC_NAN: FpClassTest = FpClassTest(FC_SNAN.0 | FC_QNAN.0);
pub const FC_INF: FpClassTest = FpClassTest(FC_POS_INF.0 | FC_NEG_INF.0);
pub const FC_NORMAL: FpClassTest = FpClassTest(FC_POS_NORMAL.0 | FC_NEG_NORMAL.0);
pub const FC_SUBNORMAL: FpClassTest = FpClassTest(FC_POS_SUBNORMAL.0 | FC_NEG_SUBNORMAL.0);
pub const FC_ZERO: FpClassTest = FpClassTest(FC_POS_ZERO.0 | FC_NEG_ZERO.0);
pub const FC_POS_FINITE: FpClassTest = FpClassTest(FC_POS_NORMAL.0 | FC_POS_SUBNORMAL.0 | FC_POS_ZERO.0);
pub const FC_NEG_FINITE: FpClassTest = FpClassTest(FC_NEG_NORMAL.0 | FC_NEG_SUBNORMAL.0 | FC_NEG_ZERO.0);
pub const FC_FINITE: FpClassTest = FpClassTest(FC_POS_FINITE.0 | FC_NEG_FINITE.0);
pub const FC_POSITIVE: FpClassTest = FpClassTest(FC_POS_FINITE.0 | FC_POS_INF.0);
pub const FC_NEGATIVE: FpClassTest = FpClassTest(FC_NEG_FINITE.0 | FC_NEG_INF.0);
pub const FC_ALL_FLAGS: FpClassTest = FpClassTest(FC_NAN.0 | FC_INF.0 | FC_FINITE.0);

impl BitOr for FpClassTest {
    type Output = FpClassTest;

    fn bitor(
        self,
        other: FpClassTest,
    ) -> FpClassTest {
        FpClassTest(self.0 | other.0)
    }
}

impl BitAnd for FpClassTest {
    type Output = FpClassTest;

    fn bitand(
        self,
        other: FpClassTest,
    ) -> FpClassTest {
        FpClassTest(self.0 & other.0)
    }
}

impl BitXor for FpClassTest {
    type Output = FpClassTest;

    fn bitxor(
        self,
        other: FpClassTest,
    ) -> FpClassTest {
        FpClassTest(self.0 ^ other.0)
    }
}

impl Not for FpClassTest {
    type Output = FpClassTest;

    /// `operator~` of `LLVM_DECLARE_ENUM_AS_BITMASK`: inside the ten bits.
    fn not(self) -> FpClassTest {
        FpClassTest(!self.0 & 0x3ff)
    }
}

impl BitOrAssign for FpClassTest {
    fn bitor_assign(
        &mut self,
        other: FpClassTest,
    ) {
        self.0 |= other.0;
    }
}

impl BitAndAssign for FpClassTest {
    fn bitand_assign(
        &mut self,
        other: FpClassTest,
    ) {
        self.0 &= other.0;
    }
}

impl FpClassTest {
    /// Whether any class of `other` is in the test.
    pub fn any(
        self,
        other: FpClassTest,
    ) -> bool {
        self.0 & other.0 != 0
    }
}

/// The test mask which returns true if the value's sign bit is flipped.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L14>
pub fn fneg(mask: FpClassTest) -> FpClassTest {
    let mut new_mask = mask & FC_NAN;
    if mask.any(FC_NEG_INF) {
        new_mask |= FC_POS_INF;
    }
    if mask.any(FC_NEG_NORMAL) {
        new_mask |= FC_POS_NORMAL;
    }
    if mask.any(FC_NEG_SUBNORMAL) {
        new_mask |= FC_POS_SUBNORMAL;
    }
    if mask.any(FC_NEG_ZERO) {
        new_mask |= FC_POS_ZERO;
    }
    if mask.any(FC_POS_ZERO) {
        new_mask |= FC_NEG_ZERO;
    }
    if mask.any(FC_POS_SUBNORMAL) {
        new_mask |= FC_NEG_SUBNORMAL;
    }
    if mask.any(FC_POS_NORMAL) {
        new_mask |= FC_NEG_NORMAL;
    }
    if mask.any(FC_POS_INF) {
        new_mask |= FC_NEG_INF;
    }
    new_mask
}

/// The test mask which returns true after fabs is applied to the value.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L35>
pub fn inverse_fabs(mask: FpClassTest) -> FpClassTest {
    let mut new_mask = mask & FC_NAN;
    if mask.any(FC_POS_ZERO) {
        new_mask |= FC_ZERO;
    }
    if mask.any(FC_POS_SUBNORMAL) {
        new_mask |= FC_SUBNORMAL;
    }
    if mask.any(FC_POS_NORMAL) {
        new_mask |= FC_NORMAL;
    }
    if mask.any(FC_POS_INF) {
        new_mask |= FC_INF;
    }
    new_mask
}

/// The test mask which returns true if the value could have the same set of
/// classes, but with a different sign.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L48>
pub fn unknown_sign(mask: FpClassTest) -> FpClassTest {
    let mut new_mask = mask & FC_NAN;
    if mask.any(FC_ZERO) {
        new_mask |= FC_ZERO;
    }
    if mask.any(FC_SUBNORMAL) {
        new_mask |= FC_SUBNORMAL;
    }
    if mask.any(FC_NORMAL) {
        new_mask |= FC_NORMAL;
    }
    if mask.any(FC_INF) {
        new_mask |= FC_INF;
    }
    new_mask
}

const NO_FP_CLASS_NAME: [(FpClassTest, &str); 16] = [
    (FC_ALL_FLAGS, "all"),
    (FC_NAN, "nan"),
    (FC_SNAN, "snan"),
    (FC_QNAN, "qnan"),
    (FC_INF, "inf"),
    (FC_NEG_INF, "ninf"),
    (FC_POS_INF, "pinf"),
    (FC_ZERO, "zero"),
    (FC_NEG_ZERO, "nzero"),
    (FC_POS_ZERO, "pzero"),
    (FC_SUBNORMAL, "sub"),
    (FC_NEG_SUBNORMAL, "nsub"),
    (FC_POS_SUBNORMAL, "psub"),
    (FC_NORMAL, "norm"),
    (FC_NEG_NORMAL, "nnorm"),
    (FC_POS_NORMAL, "pnorm"),
];

/// A human readable form of a mask.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L86>
pub fn print_fp_class_test(mut mask: FpClassTest) -> String {
    let mut text = String::from("(");
    if mask == FC_NONE {
        text.push_str("none)");
        return text;
    }
    let mut first = true;
    for (bit_test, name) in NO_FP_CLASS_NAME {
        if mask & bit_test == bit_test {
            if !first {
                text.push(' ');
            }
            first = false;
            text.push_str(name);
            mask = mask & !bit_test;
        }
    }
    assert!(mask == FC_NONE, "didn't print some mask bits");
    text.push(')');
    text
}

/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L126>
fn cannot_order_strictly_greater_impl(
    lhs: FpClassTest,
    rhs: FpClassTest,
    or_equal: bool,
    ordered_zero: bool,
) -> bool {
    let lhs = lhs & !FC_NAN;
    let rhs = rhs & !FC_NAN;
    if lhs == FC_NONE || rhs == FC_NONE {
        return true;
    }
    let mut lowest_bit_rhs = FpClassTest(rhs.0 & rhs.0.wrapping_neg());
    let mut highest_bit_lhs = FpClassTest(1 << (31 - lhs.0.leading_zeros()));
    if !ordered_zero {
        // Introduce conflict in zero bits if we're treating them as equal.
        if lowest_bit_rhs == FC_NEG_ZERO {
            lowest_bit_rhs = FC_POS_ZERO;
        }
        if highest_bit_lhs == FC_NEG_ZERO {
            highest_bit_lhs = FC_POS_ZERO;
        }
    }
    if lowest_bit_rhs.0 > highest_bit_lhs.0 {
        assert!((lhs & rhs) == FC_NONE, "no bits should intersect");
        return true;
    }
    if lowest_bit_rhs.0 < highest_bit_lhs.0 {
        return false;
    }
    let exact_values_mask = FC_ZERO | FC_INF;
    !or_equal && (lowest_bit_rhs & exact_values_mask) != FC_NONE
}

/// Whether all values in `lhs` must be less than or equal to those in `rhs`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L157>
pub fn cannot_order_strictly_greater(
    lhs: FpClassTest,
    rhs: FpClassTest,
    ordered_zero_sign: bool,
) -> bool {
    cannot_order_strictly_greater_impl(lhs, rhs, false, ordered_zero_sign)
}

/// Whether all values in `lhs` must be less than those in `rhs`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L162>
pub fn cannot_order_strictly_greater_eq(
    lhs: FpClassTest,
    rhs: FpClassTest,
    ordered_zero_sign: bool,
) -> bool {
    cannot_order_strictly_greater_impl(lhs, rhs, true, ordered_zero_sign)
}

/// Whether all values in `lhs` must be greater than or equal to those in
/// `rhs`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L167>
pub fn cannot_order_strictly_less(
    lhs: FpClassTest,
    rhs: FpClassTest,
    ordered_zero_sign: bool,
) -> bool {
    cannot_order_strictly_greater_impl(rhs, lhs, false, ordered_zero_sign)
}

/// Whether all values in `lhs` must be greater than those in `rhs`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L172>
pub fn cannot_order_strictly_less_eq(
    lhs: FpClassTest,
    rhs: FpClassTest,
    ordered_zero_sign: bool,
) -> bool {
    cannot_order_strictly_greater_impl(rhs, lhs, true, ordered_zero_sign)
}

/// All classes which are less than all values in `mask`; ignores nan.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L177>
pub fn ordered_strictly_less(
    mask: FpClassTest,
    ordered_zero_sign: bool,
) -> FpClassTest {
    let mask = mask & !FC_NAN;
    let trailing = mask.0.trailing_zeros();
    let ones = if trailing >= 32 { u32::MAX } else { (1_u32 << trailing) - 1 };
    let mut new_mask = FpClassTest(ones & FC_ALL_FLAGS.0);
    new_mask &= !FC_NAN;
    // We cannot conclude that only one zero is smaller if the zeroes are not
    // ordered.
    if !ordered_zero_sign && (new_mask & FC_ZERO) != FC_ZERO {
        new_mask &= !FC_ZERO;
    }
    new_mask
}

/// All classes which are greater than all values in `mask`; ignores nan.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Support/FloatingPointMode.cpp#L198>
pub fn ordered_strictly_greater(
    mask: FpClassTest,
    ordered_zero_sign: bool,
) -> FpClassTest {
    let leading = mask.0.leading_zeros();
    let ones = if leading == 0 { 0 } else { u32::MAX << (32 - leading) };
    let mut new_mask = FpClassTest(ones & FC_ALL_FLAGS.0);
    new_mask &= !FC_NAN;
    if !ordered_zero_sign && (new_mask & FC_ZERO) != FC_ZERO {
        new_mask &= !FC_ZERO;
    }
    new_mask
}

/// The inverted test when it is simpler than `test`, else `FC_NONE`.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenCommonISel.cpp#L197>
pub fn invert_fp_class_test_if_simpler(
    test: FpClassTest,
    use_fcmp: bool,
) -> FpClassTest {
    let inverted = !test;
    let simple = [
        FC_NAN,
        FC_SNAN,
        FC_QNAN,
        FC_INF,
        FC_POS_INF,
        FC_NEG_INF,
        FC_NORMAL,
        FC_POS_NORMAL,
        FC_NEG_NORMAL,
        FC_SUBNORMAL,
        FC_POS_SUBNORMAL,
        FC_NEG_SUBNORMAL,
        FC_ZERO,
        FC_POS_ZERO,
        FC_NEG_ZERO,
        FC_FINITE,
        FC_POS_FINITE,
        FC_NEG_FINITE,
        FC_ZERO | FC_NAN,
        FC_SUBNORMAL | FC_ZERO,
        FC_SUBNORMAL | FC_ZERO | FC_NAN,
    ];
    if simple.contains(&inverted) {
        return inverted;
    }
    if [FC_INF | FC_NAN, FC_POS_INF | FC_NAN, FC_NEG_INF | FC_NAN].contains(&inverted) {
        // If we're trying to use fcmp, we can take advantage of the nan check
        // behavior of the compare (but this is more instructions in the integer
        // expansion).
        return if use_fcmp { inverted } else { FC_NONE };
    }
    FC_NONE
}
