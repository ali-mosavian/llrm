//! C's implicit conversions, parameterized by the target's `int`.

use std::cmp::Ordering;

use super::semantic::is_float;
use super::semantic::is_integer;
use super::semantic::is_signed;
use super::semantic::scalar_width;
use super::syntax::TypeName;

pub struct Rules {
    /// What an operand narrower than it is promoted to.
    pub int: TypeName,
}

pub const I386_REAL_MODE: Rules = Rules { int: TypeName::I16 };

impl Rules {
    pub fn promoted(
        &self,
        type_name: TypeName,
    ) -> TypeName {
        if is_integer(type_name) && scalar_width(type_name) < scalar_width(self.int) { self.int } else { type_name }
    }

    /// An integer literal's own type: `int`, else `i32`, else `u32`, as C
    /// types a hex constant; `None` past all three.
    pub fn literal(
        &self,
        value: i64,
    ) -> Option<TypeName> {
        [self.int, TypeName::I32, TypeName::U32].into_iter().find(|one| fits(value, *one))
    }

    /// The usual arithmetic conversions; `None` when there is no common type.
    ///
    /// Where C would convert a signed operand to an unsigned type of the same
    /// width, there is none, unless that operand was promoted from an unsigned
    /// type and so cannot be negative.
    pub fn common(
        &self,
        left: TypeName,
        right: TypeName,
    ) -> Option<TypeName> {
        let common = self.common_plain(left, right)?;
        // Where a word and its plain twin meet, the word is the type: usize
        // stays usize.
        Some(
            [left, right]
                .into_iter()
                .find(|one| matches!(one, TypeName::Word { .. }) && one.plain() == common.plain())
                .unwrap_or(common),
        )
    }

    fn common_plain(
        &self,
        left: TypeName,
        right: TypeName,
    ) -> Option<TypeName> {
        if !implicit(left) || !implicit(right) {
            return (left == right).then_some(left);
        }
        if is_float(left) || is_float(right) {
            let wide = [left, right].contains(&TypeName::F64);
            return Some(if wide { TypeName::F64 } else { TypeName::F32 });
        }
        let (promoted_left, promoted_right) = (self.promoted(left), self.promoted(right));
        Some(match scalar_width(promoted_left).cmp(&scalar_width(promoted_right)) {
            Ordering::Greater => promoted_left,
            Ordering::Less => promoted_right,
            Ordering::Equal if is_signed(promoted_left) == is_signed(promoted_right) => promoted_left,
            Ordering::Equal => {
                let (signed, unsigned) =
                    if is_signed(promoted_left) { (left, promoted_right) } else { (right, promoted_left) };
                if is_signed(signed) {
                    return None;
                }
                unsigned
            }
        })
    }
}

/// Whether `value` is one of `type_name`'s values.
pub fn fits(
    value: i64,
    type_name: TypeName,
) -> bool {
    match type_name.plain() {
        TypeName::Char | TypeName::U8 => u8::try_from(value).is_ok(),
        TypeName::I8 => i8::try_from(value).is_ok(),
        TypeName::I16 => i16::try_from(value).is_ok(),
        TypeName::U16 => u16::try_from(value).is_ok(),
        TypeName::I32 => i32::try_from(value).is_ok(),
        TypeName::U32 => u32::try_from(value).is_ok(),
        _ => false,
    }
}

/// The bits `value` takes: its magnitude's, and a sign bit when negative.
pub fn literal_bits(value: i64) -> u32 {
    if value < 0 { 65 - (!value).leading_zeros() } else { 64 - value.leading_zeros() }
}

/// Whether a type converts implicitly: the integers and floats.
pub fn implicit(type_name: TypeName) -> bool {
    is_integer(type_name) || is_float(type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_operands_meet_at_int_and_same_width_sign_mixing_has_no_common_type() {
        let rules = I386_REAL_MODE;
        assert_eq!(rules.common(TypeName::U8, TypeName::U8), Some(TypeName::I16));
        assert_eq!(rules.common(TypeName::U8, TypeName::U16), Some(TypeName::U16));
        assert_eq!(rules.common(TypeName::I8, TypeName::U16), None);
        assert_eq!(rules.common(TypeName::I8, TypeName::U8), Some(TypeName::I16));
        assert_eq!(rules.common(TypeName::I16, TypeName::U32), Some(TypeName::U32));
        assert_eq!(rules.common(TypeName::U16, TypeName::I32), Some(TypeName::I32));
        assert_eq!(rules.common(TypeName::I32, TypeName::F32), Some(TypeName::F32));
        assert_eq!(rules.common(TypeName::F32, TypeName::F64), Some(TypeName::F64));
        assert_eq!(rules.common(TypeName::I16, TypeName::U16), None);
        assert_eq!(rules.common(TypeName::Bool, TypeName::I16), None);
        let wide = Rules { int: TypeName::I32 };
        assert_eq!(wide.common(TypeName::U16, TypeName::U16), Some(TypeName::I32));
    }
}
