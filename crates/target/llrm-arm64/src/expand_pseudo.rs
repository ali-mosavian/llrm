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
use crate::mc::inst::{Arrangement, BitSelect, Inst, Vreg, VectorOp};

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

/// The vector `member` of the tuple whose first register is `first`.
fn member(
    first: Vreg,
    at: u8,
) -> Vreg {
    first.in_run(u32::from(at))
}

/// `REG_SEQUENCE`: the moves that put `members` in the tuple's registers, as
/// one parallel copy. A move goes once its target is no pending source; what
/// is left is cycles, each broken by swapping with three `eor`s.
fn expand_tuple_build(
    tuple: Vreg,
    count: u8,
    members: &[Vreg; 4],
) -> Vec<Inst> {
    let mut pending: Vec<(Vreg, Vreg)> =
        (0..count).map(|at| (member(tuple, at), members[usize::from(at)])).filter(|(to, from)| to != from).collect();
    let eor = |rd, rn, rm| Inst::VectorBinary { op: VectorOp::Eor, lanes: Arrangement::B16, rd, rn, rm };
    let mut out = Vec::new();
    while !pending.is_empty() {
        if let Some(at) = (0..pending.len()).find(|&at| pending.iter().all(|(_, from)| *from != pending[at].0)) {
            let (to, from) = pending.remove(at);
            out.push(Inst::VectorMov { rd: to, rn: from });
            continue;
        }
        // Every target is another move's source: swap one pair, which leaves
        // the other moves reading the swapped register.
        let (to, from) = pending.remove(0);
        out.extend([eor(to, to, from), eor(from, from, to), eor(to, to, from)]);
        for (_, source) in &mut pending {
            if *source == to {
                *source = from;
            } else if *source == from {
                *source = to;
            }
        }
        pending.retain(|(target, source)| target != source);
    }
    out
}

/// `blocks` with every pseudo replaced by the real instructions.
pub fn run(blocks: Vec<Lowered>) -> Vec<Lowered> {
    blocks
        .into_iter()
        .map(|block| {
            let pseudo = |inst: &Inst| {
                matches!(inst, Inst::VectorBsp { .. } | Inst::VectorTupleExtract { .. } | Inst::VectorTupleBuild { .. })
            };
            if !block.insts.iter().any(pseudo) {
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
                    Inst::VectorTupleExtract { rd, tuple, index } => {
                        let from = member(tuple, index);
                        if rd != from {
                            insts.push(Inst::VectorMov { rd, rn: from });
                            locs.push(loc);
                        }
                    }
                    Inst::VectorTupleBuild { tuple, count, members } => {
                        for real in expand_tuple_build(tuple, count, &members) {
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
