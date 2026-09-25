//! Port of `qbopt/backend/pointers.py`: target pointer representation, kept
//! below the MIR boundary.

use crate::model::ir::{self, Loc, Operation};

/// The huge-pointer ABI's selector stride, as a constant or a runtime cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HugeShift {
    Fixed(i64),
    Cell(ir::Mem),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    pub huge_shift: HugeShift,
}

impl Model {
    /// `__post_init__`.
    pub fn new(huge_shift: HugeShift) -> Result<Self, String> {
        match &huge_shift {
            HugeShift::Cell(mem) if mem.width != 1 => {
                return Err("huge-pointer selector shift must be a byte".into());
            }
            HugeShift::Fixed(shift) if !(0..16).contains(shift) => {
                return Err("unsupported huge-pointer selector shift".into());
            }
            _ => {}
        }
        Ok(Self { huge_shift })
    }

    /// Correct a packed addition's unit selector carry to the ABI's stride.
    pub fn offset(
        &self,
        pointer: &Loc,
        displacement: &Loc,
        result: &Loc,
        fresh: &mut dyn FnMut() -> u32,
    ) -> Result<Vec<ir::Semantics>, String> {
        let wide = |arg: &Loc| matches!(arg, Loc::Held(ir::Held { width: 4, .. }) | Loc::Imm(ir::Imm { width: 4, .. }));
        if !wide(pointer) || !wide(displacement) || !matches!(result, Loc::Held(ir::Held { width: 4, .. })) {
            return Err("pointer offset requires a pointer, displacement and result at width 4".into());
        }
        let mut parts = Vec::new();
        let low = binary(&mut parts, fresh, "and", pointer.clone(), imm(0xffff, 4), None);
        let total = binary(&mut parts, fresh, "add", low, displacement.clone(), None);
        let pages = binary(&mut parts, fresh, "shr", total, imm(16, 1), None);
        let shift = match &self.huge_shift {
            HugeShift::Cell(mem) => {
                let shift = Loc::Held(ir::Held { value: fresh(), width: 1 });
                parts.push(ir::Semantics {
                    name: Some("mov".into()),
                    dests: vec![shift.clone()],
                    sources: vec![Loc::Mem(mem.clone())],
                    ..ir::Semantics::new(Operation::Move)
                });
                shift
            }
            HugeShift::Fixed(shift) => imm(*shift, 1),
        };
        let delta = binary(&mut parts, fresh, "shl", pages.clone(), shift, None);
        let correction = binary(&mut parts, fresh, "sub", delta, pages, None);
        let high = binary(&mut parts, fresh, "shl", correction, imm(16, 1), None);
        let advanced = binary(&mut parts, fresh, "add", pointer.clone(), displacement.clone(), None);
        binary(&mut parts, fresh, "add", advanced, high, Some(result.clone()));
        Ok(parts)
    }
}

fn imm(value: i64, width: u32) -> Loc {
    Loc::Imm(ir::Imm { value, width, address: None })
}

fn binary(
    parts: &mut Vec<ir::Semantics>,
    fresh: &mut dyn FnMut() -> u32,
    name: &str,
    left: Loc,
    right: Loc,
    destination: Option<Loc>,
) -> Loc {
    let destination = destination.unwrap_or_else(|| Loc::Held(ir::Held { value: fresh(), width: 4 }));
    parts.push(ir::Semantics {
        name: Some(name.into()),
        dests: vec![destination.clone()],
        sources: vec![left, right],
        ..ir::Semantics::new(Operation::Binary)
    });
    destination
}

#[cfg(test)]
pub mod tests {
    use crate::support::hash::HashMap;

    use super::*;
    use crate::objectfile::module::{Addr, Space};

    fn held(value: u32) -> Loc {
        Loc::Held(ir::Held { value, width: 4 })
    }

    fn counter() -> impl FnMut() -> u32 {
        let mut next = 10;
        move || {
            next += 1;
            next - 1
        }
    }

    fn parts(model: &Model) -> Vec<ir::Semantics> {
        model.offset(&held(1), &held(2), &held(3), &mut counter()).unwrap()
    }

    /// The test file's `execute`: interpret the parts over 32-bit values.
    pub fn execute(parts: &[ir::Semantics], pointer: i64, offset: i64, memory: &HashMap<Addr, i64>) -> i64 {
        let mut values: HashMap<u32, i64> = HashMap::from_iter([(1, pointer), (2, offset)]);
        let dest = |part: &ir::Semantics| match &part.dests[0] {
            Loc::Held(one) => one.value,
            other => panic!("{other:?}"),
        };
        for part in parts {
            if part.op == Operation::Move {
                let Loc::Mem(source) = &part.sources[0] else { panic!("{part:?}") };
                values.insert(dest(part), memory[&source.addr.unwrap()]);
                continue;
            }
            let read = |arg: &Loc| match arg {
                Loc::Imm(one) => one.value,
                Loc::Held(one) => values[&one.value],
                other => panic!("{other:?}"),
            };
            let (left, right) = (read(&part.sources[0]), read(&part.sources[1]));
            let answer = match part.name.as_deref().unwrap() {
                "and" => left & right,
                "add" => left + right,
                "sub" => left - right,
                "shr" => left >> right,
                "shl" => left << right,
                "or" => left | right,
                _ => panic!("{part:?}"),
            };
            values.insert(dest(part), answer & 0xffff_ffff);
        }
        values[&3]
    }

    /// NDARR's pointer stride expanded to nine arithmetic operations on every iteration.
    #[test]
    fn test_huge_pointer_advance_does_not_rebuild_both_halves() {
        let parts = parts(&Model::new(HugeShift::Fixed(12)).unwrap());
        assert!(parts.len() <= 8);
        assert_eq!(execute(&parts, 0x2000_fffe, 2, &HashMap::default()), 0x3000_0000);
    }

    /// NDARR/HUGELP strides must retain carries and borrows for every supported selector ABI.
    #[test]
    fn test_packed_correction_matches_independent_selector_and_offset_arithmetic() {
        for shift in 0..16 {
            let parts = parts(&Model::new(HugeShift::Fixed(shift)).unwrap());
            for pointer in [0, 0xffff, 0x1234_fffe, 0xffff_0000, 0xffff_ffff_i64] {
                for displacement in
                    [0, 1, 2, 65535, 65536, 0x7fff_ffff, 0x8000_0000, 0xffff_fffe, 0xffff_ffff_i64]
                {
                    let sum = (pointer & 0xffff) + displacement;
                    let (pages, offset) = (sum / 65536, sum % 65536);
                    let selector = ((pointer >> 16) + pages * (1 << shift)) & 0xffff;
                    assert_eq!(execute(&parts, pointer, displacement, &HashMap::default()), (selector << 16) | offset);
                }
            }
        }
    }

    #[test]
    fn test_unknown_selector_models_are_rejected() {
        for shift in [-1, 16] {
            assert!(Model::new(HugeShift::Fixed(shift)).unwrap_err().contains("selector shift"));
        }
    }

    /// Byte 65536 uses the runtime selector stride, not a CPU-derived DOS constant.
    #[test]
    fn test_runtime_pointer_abi_controls_crossing() {
        for (shift, expected) in [(12, 0x3000_0000), (3, 0x2008_0000)] {
            let address = Addr { index: 7, ..Addr::new(Space::External, 0) };
            let model =
                Model::new(HugeShift::Cell(ir::Mem { disp_width: 2, ..ir::Mem::new(Some(address), 1) })).unwrap();
            let parts = parts(&model);
            assert_eq!(execute(&parts, 0x2000_fffe, 2, &HashMap::from_iter([(address, shift)])), expected);
        }
    }

    /// Huge-loop pointer induction must preserve carries, borrows and wrapped byte offsets.
    #[test]
    fn test_pointer_recurrence_matches_recomputed_offsets() {
        for shift in [0, 3, 12] {
            for (start, stride) in [(65534, 2), (1934, 402), (2, -4), (0xffff_fffe_i64, 4)] {
                let parts = parts(&Model::new(HugeShift::Fixed(shift)).unwrap());
                let none = HashMap::default();
                let base = 0xf000_fffe;
                let mut current = execute(&parts, base, start, &none);
                for iteration in 0..5 {
                    assert_eq!(current, execute(&parts, base, (start + iteration * stride) & 0xffff_ffff, &none));
                    current = execute(&parts, current, stride & 0xffff_ffff, &none);
                }
            }
        }
    }
}
