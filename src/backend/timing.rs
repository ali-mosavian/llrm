//! Port of `qbopt/backend/timing.py`: audited instruction core-clock
//! bounds, separate from scoreboard estimates.
//!
//! Sources and limitations: docs/timing-audit.md. Bounds exclude decode,
//! prefix, memory and scheduling costs.

use crate::backend::cpu::{self as targets, ProfileOrName};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Clocks {
    pub minimum: i64,
    pub maximum: i64,
}

pub fn signed_multiply<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
    full: bool,
) -> Result<Option<Clocks>, String> {
    if ![2, 4].contains(&width) {
        return Ok(None);
    }
    match targets::profile(cpu)?.name.as_str() {
        "386" => {
            return Ok(Some(Clocks {
                minimum: 9,
                maximum: if width == 2 { 22 } else { 38 },
            }));
        }
        "486" => {
            return Ok(Some(Clocks {
                minimum: 13,
                maximum: if width == 2 { 26 } else { 42 },
            }));
        }
        "P5" => {
            let clocks = if full && width == 2 { 11 } else { 10 };
            return Ok(Some(Clocks {
                minimum: clocks,
                maximum: clocks,
            }));
        }
        _ => {}
    }
    Ok(None)
}

pub fn signed_divide<'a>(
    cpu: impl Into<ProfileOrName<'a>>,
    width: i64,
) -> Result<Option<Clocks>, String> {
    if ![2, 4].contains(&width) {
        return Ok(None);
    }
    let clocks = match targets::profile(cpu)?.name.as_str() {
        "386" | "486" => {
            if width == 2 {
                27
            } else {
                43
            }
        }
        "P5" => {
            if width == 2 {
                30
            } else {
                46
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(Clocks {
        minimum: clocks,
        maximum: clocks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::division;
    use crate::model::ir;

    #[test]
    fn test_full_product_bounds_follow_operand_width() {
        for (cpu, width, low, high) in [
            ("386", 2, 9, 22),
            ("386", 4, 9, 38),
            ("486", 2, 13, 26),
            ("486", 4, 13, 42),
            ("P5", 2, 11, 11),
            ("P5", 4, 10, 10),
        ] {
            assert_eq!(
                signed_multiply(cpu, width, true),
                Ok(Some(Clocks {
                    minimum: low,
                    maximum: high
                }))
            );
        }
    }

    /// LNGMXX was selected on 486 using 26 clocks for a product that may take 42.
    #[test]
    fn test_486_reciprocal_does_not_win_using_midpoint_multiply() {
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let results = [
            ir::Held { value: 2, width: 4 },
            ir::Held { value: 3, width: 4 },
        ];
        let held = ir::Held { value: 1, width: 4 };
        assert_eq!(
            division::reciprocal(held, 7, &results, &mut fresh, "486", true),
            Ok(None)
        );
    }

    #[test]
    fn test_unverified_p6_forms_are_not_silently_priced() {
        assert_eq!(signed_multiply("P6", 4, true), Ok(None));
        assert_eq!(signed_divide("P6", 4), Ok(None));
    }

    /// A quotient-only divide should not multiply back and subtract for a dead remainder.
    #[test]
    fn test_quotient_only_reciprocal_omits_remainder_reconstruction() {
        let (quotient, remainder) = (
            ir::Held { value: 2, width: 4 },
            ir::Held { value: 3, width: 4 },
        );
        let mut count = 4..;
        let mut fresh = || count.next().unwrap();
        let held = ir::Held { value: 1, width: 4 };
        let parts = division::reciprocal(held, 7, &[quotient, remainder], &mut fresh, "P5", false)
            .unwrap()
            .expect("a reciprocal");
        assert_eq!(parts[parts.len() - 1].dests, [ir::Loc::Held(quotient)]);
        assert!(
            !parts
                .iter()
                .any(|one| one.dests.contains(&ir::Loc::Held(remainder)))
        );
        assert!(!parts.iter().any(|one| one.name.as_deref() == Some("sub")));
    }
}
