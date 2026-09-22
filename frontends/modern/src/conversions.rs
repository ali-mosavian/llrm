//! C's implicit conversions, parameterized by the target's `int`.

use std::cmp::Ordering;

use crate::semantic::is_float;
use crate::semantic::is_integer;
use crate::semantic::is_signed;
use crate::semantic::width;
use crate::syntax::TypeName;

pub struct Rules {
    /// What an operand narrower than it is promoted to.
    pub int: TypeName,
}

pub const I386_REAL_MODE: Rules = Rules { int: TypeName::I16 };

impl Rules {
    pub fn promoted(&self, type_name: TypeName) -> TypeName {
        if is_integer(type_name) && width(type_name) < width(self.int) {
            self.int
        } else {
            type_name
        }
    }

    /// The usual arithmetic conversions; `None` when there is no common type.
    ///
    /// Where C would convert a signed operand to an unsigned type of the same
    /// width, there is none, unless that operand was promoted from an unsigned
    /// type and so cannot be negative.
    pub fn common(&self, left: TypeName, right: TypeName) -> Option<TypeName> {
        if !implicit(left) || !implicit(right) {
            return (left == right).then_some(left);
        }
        if is_float(left) || is_float(right) {
            let wide = [left, right].contains(&TypeName::F64);
            return Some(if wide { TypeName::F64 } else { TypeName::F32 });
        }
        let (promoted_left, promoted_right) = (self.promoted(left), self.promoted(right));
        Some(match width(promoted_left).cmp(&width(promoted_right)) {
            Ordering::Greater => promoted_left,
            Ordering::Less => promoted_right,
            Ordering::Equal if is_signed(promoted_left) == is_signed(promoted_right) => promoted_left,
            Ordering::Equal => {
                let (signed, unsigned) = if is_signed(promoted_left) {
                    (left, promoted_right)
                } else {
                    (right, promoted_left)
                };
                if is_signed(signed) {
                    return None;
                }
                unsigned
            }
        })
    }
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
