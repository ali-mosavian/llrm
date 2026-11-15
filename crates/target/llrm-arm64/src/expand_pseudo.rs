//! The pseudo instructions that become real ones only once registers are
//! known (`AArch64ExpandPseudoInsts`).
//!
//! # How it works
//! `BSPv16i8` selects `mask ? a : b` without saying which register is the
//! mask. After allocation the destination decides: the register of `b` makes
//! it `bit` (insert `a` where the mask is set), the register of `a` makes it
//! `bif`, the mask's makes it `bsl`, and any other starts with a copy of the
//! mask into the destination and then `bsl`s.
//!
//! # Runs
//! After the frame is laid out, before the blocks are folded.
//!
//! # See also
//! LLVM's `AArch64ExpandPseudo::expandMI`
//! ([L1334](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ExpandPseudoInsts.cpp#L1334)).

use crate::frame::Lowered;
use crate::mc::inst::{BitSelect, Inst, Vreg};

/// The real instructions of one `BSP`.
fn expand_bsp(
    quad: bool,
    rd: Vreg,
    mask: Vreg,
    rn: Vreg,
    rm: Vreg,
) -> Vec<Inst> {
    if rd == rm {
        vec![Inst::VectorBitSelect { op: BitSelect::Bit, quad, rd, rn, rm: mask }]
    } else if rd == rn {
        vec![Inst::VectorBitSelect { op: BitSelect::Bif, quad, rd, rn: rm, rm: mask }]
    } else if rd == mask {
        vec![Inst::VectorBitSelect { op: BitSelect::Bsl, quad, rd, rn, rm }]
    } else {
        vec![Inst::VectorMov { rd, rn: mask }, Inst::VectorBitSelect { op: BitSelect::Bsl, quad, rd, rn, rm }]
    }
}

/// `blocks` with every pseudo replaced by the real instructions.
pub fn run(blocks: Vec<Lowered>) -> Vec<Lowered> {
    blocks
        .into_iter()
        .map(|block| {
            if !block.insts.iter().any(|inst| matches!(inst, Inst::VectorBsp { .. })) {
                return block;
            }
            let mut insts = Vec::with_capacity(block.insts.len());
            let mut locs = Vec::with_capacity(block.locs.len());
            for (inst, loc) in block.insts.into_iter().zip(block.locs) {
                match inst {
                    Inst::VectorBsp { quad, rd, mask, rn, rm } => {
                        for real in expand_bsp(quad, rd, mask, rn, rm) {
                            insts.push(real);
                            locs.push(loc);
                        }
                    }
                    other => {
                        insts.push(other);
                        locs.push(loc);
                    }
                }
            }
            Lowered { label: block.label, insts, locs }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The destination decides: `b`'s register `bit`, `a`'s `bif`, the
    /// mask's `bsl`, and a fourth register a copy and a `bsl`.
    #[test]
    fn the_destination_picks_bit_bif_or_bsl() {
        let (d, m, a, b) = (Vreg(0), Vreg(1), Vreg(2), Vreg(3));
        let ops = |insts: Vec<Inst>| -> Vec<BitSelect> {
            insts
                .into_iter()
                .filter_map(|inst| match inst {
                    Inst::VectorBitSelect { op, .. } => Some(op),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(ops(expand_bsp(true, b, m, a, b)), [BitSelect::Bit]);
        assert_eq!(ops(expand_bsp(true, a, m, a, b)), [BitSelect::Bif]);
        assert_eq!(ops(expand_bsp(true, m, m, a, b)), [BitSelect::Bsl]);
        let four = expand_bsp(true, d, m, a, b);
        assert_eq!(four.len(), 2, "a copy of the mask, then bsl");
        assert!(matches!(four[0], Inst::VectorMov { rd, rn } if rd == d && rn == m));
    }
}
