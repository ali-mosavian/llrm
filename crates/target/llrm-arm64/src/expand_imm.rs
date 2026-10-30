//! The instructions that put a constant in a register, translated from LLVM's
//! `AArch64ExpandImm.cpp`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64ExpandImm.cpp>):
//! the order of the attempts, and so the choice among sequences of one length,
//! is LLVM's.

use crate::mc::immediate;
use crate::mc::inst::*;

/// The instructions that set `rd` to `value` as a register of `bits` (32 or 64)
/// bits: `AArch64_IMM::expandMOVImm`.
pub fn mov_imm(
    value: u64,
    bits: u32,
    rd: Gpr,
) -> Vec<Inst> {
    let width = if bits == 32 { Width::W } else { Width::X };
    let mut out = Sequence { rd, width, insns: Vec::new() };
    out.expand(value, bits);
    out.insns
}

struct Sequence {
    rd: Gpr,
    width: Width,
    insns: Vec<Inst>,
}

fn chunk(
    value: u64,
    index: u32,
) -> u64 {
    (value >> (index * 16)) & 0xFFFF
}

fn is_mask(value: u64) -> bool {
    value != 0 && value & value.wrapping_add(1) == 0
}

fn is_shifted_mask(value: u64) -> bool {
    value != 0 && is_mask((value - 1) | value)
}

/// The code of a 64-bit logical immediate (`processLogicalImmediate`).
fn logical(value: u64) -> Option<u32> {
    immediate::logical(value, 64)
}

impl Sequence {
    /// `ORR rd, xzr, #value`, `rd` read from the zero register for the first
    /// instruction of a sequence and from itself after.
    fn logic_imm(
        &mut self,
        op: Logic,
        from_zero: bool,
        code: u32,
    ) {
        let bits = self.width.bits();
        let _ = bits;
        // The value the code stands for; the encoder takes the value, not the
        // code, so decode it again through the 64-bit pattern.
        let value = decode_logical(code, self.width.bits());
        self.insns.push(Inst::LogicImm {
            op,
            width: self.width,
            rd: self.rd,
            rn: if from_zero { Gpr::R31 } else { self.rd },
            imm: value,
        });
    }

    fn move_wide(
        &mut self,
        op: Wide,
        imm16: u64,
        shift: u32,
    ) {
        self.insns.push(Inst::MoveWide { op, width: self.width, rd: self.rd, imm16: imm16 as u16, lsl: shift as u8 });
    }

    fn orr_imm_value(
        &mut self,
        value: u64,
        from_zero: bool,
    ) {
        let code = immediate::logical(value, self.width.bits()).expect("an encodable logical immediate");
        self.logic_imm(Logic::Orr, from_zero, code);
    }

    /// `rd = rd op rd, lsl #n` (`EORXrs`, `EONXrs`, `ORRXrs`).
    fn shifted_self(
        &mut self,
        op: Logic,
        invert: bool,
        amount: u32,
    ) {
        self.insns.push(Inst::LogicShifted {
            op,
            invert,
            width: Width::X,
            rd: self.rd,
            rn: self.rd,
            rm: self.rd,
            shift: Shift::Lsl,
            amount: amount as u8,
        });
    }

    fn expand(
        &mut self,
        imm: u64,
        bit_size: u32,
    ) {
        // Count the 16-bit chunks that are all ones or all zeros.
        let (mut one_chunks, mut zero_chunks) = (0u32, 0u32);
        let mut shift = 0;
        while shift < bit_size {
            let piece = (imm >> shift) & 0xFFFF;
            if piece == 0xFFFF {
                one_chunks += 1;
            } else if piece == 0 {
                zero_chunks += 1;
            }
            shift += 16;
        }
        // Prefer MOVZ/MOVN over ORR because of the rules for the "mov" alias.
        if bit_size / 16 - one_chunks <= 1 || bit_size / 16 - zero_chunks <= 1 {
            self.simple(imm, bit_size, one_chunks, zero_chunks);
            return;
        }
        // Try a single ORR.
        let uimm = imm << (64 - bit_size) >> (64 - bit_size);
        if immediate::logical(uimm, bit_size).is_some() {
            self.orr_imm_value(uimm, true);
            return;
        }
        // One to three instruction sequences: prefer MOVZ/MOVN followed by
        // MOVK.
        if one_chunks >= bit_size / 16 - 2 || zero_chunks >= bit_size / 16 - 2 {
            self.simple(imm, bit_size, one_chunks, zero_chunks);
            return;
        }
        assert_eq!(bit_size, 64, "every 32-bit immediate expands to a MOVZ/MOVK pair");
        // 64-bit ORR followed by MOVK: the ORR immediate is built by zeroing
        // the chunk to replace, filling it with ones, or taking the
        // other half's.
        let mut shift = 0;
        while shift < bit_size {
            let shifted_mask = 0xFFFFu64 << shift;
            let zero_chunk = uimm & !shifted_mask;
            let one_chunk = uimm | shifted_mask;
            let rotated = uimm.rotate_left(32);
            let replicate_chunk = zero_chunk | (rotated & shifted_mask);
            let pick = [zero_chunk, one_chunk, replicate_chunk]
                .into_iter()
                .find(|candidate| immediate::logical(*candidate, bit_size).is_some());
            if let Some(candidate) = pick {
                self.orr_imm_value(candidate, true);
                self.move_wide(Wide::Keep, chunk(uimm, shift / 16), shift);
                return;
            }
            shift += 16;
        }
        if self.orr_of_logical_immediates(imm)
            || self.and_of_logical_immediates(imm)
            || self.eor_of_logical_immediates(uimm)
        {
            return;
        }
        if self.copy_with_negation(imm, false) {
            return;
        }
        // Three instruction sequences.
        if one_chunks != 0 || zero_chunks != 0 {
            self.simple(imm, bit_size, one_chunks, zero_chunks);
            return;
        }
        if self.replicate_chunks(uimm) || self.sequence_of_ones(uimm) || self.copy_with_negation(imm, true) {
            return;
        }
        self.simple(imm, bit_size, one_chunks, zero_chunks);
    }

    /// `expandMOVImmSimple`: a MOVZ or MOVN, then MOVKs.
    fn simple(
        &mut self,
        mut imm: u64,
        bit_size: u32,
        one_chunks: u32,
        zero_chunks: u32,
    ) {
        // MOVN builds the high bits when more chunks are ones than zeros.
        let negated = one_chunks > zero_chunks;
        if negated {
            imm = !imm;
        }
        if bit_size == 32 {
            imm &= (1u64 << 32) - 1;
        }
        let (mut shift, mut last_shift) = (0, 0);
        if imm != 0 {
            let leading = imm.leading_zeros();
            let trailing = imm.trailing_zeros();
            shift = (trailing / 16) * 16;
            last_shift = ((63 - leading) / 16) * 16;
        }
        let first = (imm >> shift) & 0xFFFF;
        self.move_wide(if negated { Wide::Not } else { Wide::Zero }, first, shift);
        if shift == last_shift {
            return;
        }
        // The rest of the bits go back for MOVK.
        if negated {
            imm = !imm;
        }
        while shift < last_shift {
            shift += 16;
            let imm16 = (imm >> shift) & 0xFFFF;
            if imm16 == if negated { 0xFFFF } else { 0 } {
                continue;
            }
            self.move_wide(Wide::Keep, imm16, shift);
        }
        // With equal high and low halves, an ORR of the low half replaces the
        // rest.
        if self.insns.len() > 2 && (imm >> 32) == (imm & 0xFFFF_FFFF) {
            self.insns.truncate(2);
            self.shifted_self(Logic::Orr, false, 32);
        }
    }

    /// `tryToreplicateChunks`: a repeated 16-bit chunk by one ORR, the rest by
    /// MOVK.
    fn replicate_chunks(
        &mut self,
        uimm: u64,
    ) -> bool {
        for (chunk_value, count) in counts_in_map_order(uimm) {
            if count != 2 && count != 3 {
                continue;
            }
            let replicated = chunk_value | chunk_value << 16 | chunk_value << 32 | chunk_value << 48;
            if logical(replicated).is_none() {
                continue;
            }
            self.orr_imm_value(replicated, true);
            let mut shift = 0;
            let mut imm16 = 0;
            while shift < 64 {
                imm16 = (uimm >> shift) & 0xFFFF;
                if imm16 != chunk_value {
                    break;
                }
                shift += 16;
            }
            self.move_wide(Wide::Keep, imm16, shift);
            if count == 3 {
                return true;
            }
            shift += 16;
            while shift < 64 {
                imm16 = (uimm >> shift) & 0xFFFF;
                if imm16 != chunk_value {
                    break;
                }
                shift += 16;
            }
            self.move_wide(Wide::Keep, imm16, shift);
            return true;
        }
        false
    }

    /// `trySequenceOfOnes`: a run of ones, interrupted by up to two chunks, by
    /// an ORR and MOVKs.
    fn sequence_of_ones(
        &mut self,
        uimm: u64,
    ) -> bool {
        const NOT_SET: i32 = -1;
        let (mut start, mut end) = (NOT_SET, NOT_SET);
        for index in 0..4i32 {
            let piece = chunk(uimm, index as u32);
            let signed = ((piece << 48) as i64) >> 48;
            if is_start_chunk(signed as u64) {
                start = index;
            } else if is_end_chunk(signed as u64) {
                end = index;
            }
        }
        if start == NOT_SET || end == NOT_SET {
            return false;
        }
        let (mut outside, mut inside) = (0u64, 0xFFFFu64);
        if start > end {
            std::mem::swap(&mut start, &mut end);
            std::mem::swap(&mut outside, &mut inside);
        }
        let mut orr_imm = uimm;
        let (mut first_movk, mut second_movk) = (NOT_SET, NOT_SET);
        for index in 0..4i32 {
            let piece = chunk(uimm, index as u32);
            if (index < start || end < index) && piece != outside {
                orr_imm = update_imm(orr_imm, index as u32, outside == 0);
            } else if index > start && index < end && piece != inside {
                orr_imm = update_imm(orr_imm, index as u32, inside != 0xFFFF);
            } else {
                continue;
            }
            if first_movk == NOT_SET {
                first_movk = index;
            } else {
                second_movk = index;
            }
        }
        assert_ne!(first_movk, NOT_SET, "a constant materializable with a single ORR");
        self.orr_imm_value(orr_imm, true);
        self.move_wide(Wide::Keep, chunk(uimm, first_movk as u32), first_movk as u32 * 16);
        if second_movk == NOT_SET {
            return true;
        }
        self.move_wide(Wide::Keep, chunk(uimm, second_movk as u32), second_movk as u32 * 16);
        true
    }

    /// `tryCopyWithNegation`: MOVN and MOVK for the low half, then an EOR or
    /// EON of the register with itself shifted.
    fn copy_with_negation(
        &mut self,
        imm: u64,
        allow_three_sequence: bool,
    ) -> bool {
        if imm == 0 || is_shifted_mask(imm) {
            return false;
        }
        for n in 17..48u32 {
            // Attempt EOR.
            let c = 0xFFFF_FFFF_0000_0000u64 | (imm ^ (imm << n));
            if (c ^ (c << n)) == imm && self.negation_expansion(false, c, n, allow_three_sequence) {
                return true;
            }
            // Attempt EON.
            let c = 0xFFFF_FFFF_0000_0000u64 | (imm ^ !(!imm << n));
            if (c ^ !(c << n)) == imm && self.negation_expansion(true, c, n, allow_three_sequence) {
                return true;
            }
        }
        false
    }

    fn negation_expansion(
        &mut self,
        invert: bool,
        c: u64,
        n: u32,
        allow_three_sequence: bool,
    ) -> bool {
        debug_assert_eq!(c >> 32, 0xFFFF_FFFF);
        let imm0 = c & 0xFFFF;
        let imm16 = (c >> 16) & 0xFFFF;
        if imm0 != 0xFFFF && imm16 != 0xFFFF && !allow_three_sequence {
            return false;
        }
        if imm0 != 0xFFFF {
            self.move_wide(Wide::Not, imm0 ^ 0xFFFF, 0);
            if imm16 != 0xFFFF {
                self.move_wide(Wide::Keep, imm16, 16);
            }
        } else {
            self.move_wide(Wide::Not, imm16 ^ 0xFFFF, 16);
        }
        self.shifted_self(Logic::Eor, invert, n);
        true
    }

    /// `tryOrrOfLogicalImmediates`.
    fn orr_of_logical_immediates(
        &mut self,
        uimm: u64,
    ) -> bool {
        let Some((first, second)) = decompose_into_orr(uimm) else { return false };
        if logical(first).is_none() || logical(second).is_none() {
            return false;
        }
        self.orr_imm_value(first, true);
        self.orr_imm_value(second, false);
        true
    }

    /// `tryAndOfLogicalImmediates`: DeMorgan's law on the ORR problem.
    fn and_of_logical_immediates(
        &mut self,
        uimm: u64,
    ) -> bool {
        let Some((first, second)) = decompose_into_orr(!uimm) else { return false };
        if logical(!first).is_none() || logical(!second).is_none() {
            return false;
        }
        self.orr_imm_value(!first, true);
        let code = logical(!second).expect("checked");
        self.logic_imm(Logic::And, false, code);
        true
    }

    /// `tryEorOfLogicalImmediates`: an ORR then an EOR, each of a logical
    /// immediate.
    fn eor_of_logical_immediates(
        &mut self,
        imm: u64,
    ) -> bool {
        // The larger repetition size of the two logical immediates.
        let mut big_size = 64u32;
        loop {
            big_size /= 2;
            let mask = (1u64 << big_size) - 1;
            if (imm & mask) != ((imm >> big_size) & mask) {
                big_size *= 2;
                break;
            }
            if big_size <= 2 {
                break;
            }
        }
        let big_mask = u64::MAX >> (64 - big_size);
        // The last bit of each run of ones, circularly.
        let run_starts = imm & !imm.rotate_left(1);
        let runs_per_big_chunk = (run_starts & big_mask).count_ones() as usize;
        const BIG_TO_SMALL_SIZE: [i8; 32] = [
            -1, -1, 0, 1, 2, 2, -1, 3, 3, 3, -1, -1, -1, -1, -1, 4, 4, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
            -1, -1, 5,
        ];
        let Some(&shift) = BIG_TO_SMALL_SIZE.get(runs_per_big_chunk) else { return false };
        if shift == -1 {
            return false;
        }
        let small_size = big_size >> shift;
        const REPEATED_ONES: [u64; 7] = [
            0xFFFF_FFFF_FFFF_FFFF,
            0x5555_5555_5555_5555,
            0x1111_1111_1111_1111,
            0x0101_0101_0101_0101,
            0x0001_0001_0001_0001,
            0x0000_0001_0000_0001,
            0x0000_0000_0000_0001,
        ];
        let small_ones = REPEATED_ONES[small_size.trailing_zeros() as usize];
        let mut rotation = run_starts.trailing_zeros();
        let mut rotated = imm.rotate_right(rotation);
        for _ in 0..3 {
            let run_length = rotated.trailing_ones();
            let small_imm = (small_ones << run_length).wrapping_sub(small_ones).rotate_left(rotation);
            let big_imm = imm ^ small_imm;
            if logical(big_imm).is_some() && logical(small_imm).is_some() {
                self.orr_imm_value(small_imm, true);
                let code = logical(big_imm).expect("checked");
                self.logic_imm(Logic::Eor, false, code);
                return true;
            }
            // Rotate to the next run of ones.
            rotation += (run_starts.rotate_right(rotation) & !1).trailing_zeros();
            rotated = imm.rotate_right(rotation);
        }
        false
    }
}

fn is_start_chunk(piece: u64) -> bool {
    piece != 0 && piece != u64::MAX && is_mask(!piece)
}

fn is_end_chunk(piece: u64) -> bool {
    piece != 0 && piece != u64::MAX && is_mask(piece)
}

fn update_imm(
    imm: u64,
    index: u32,
    clear: bool,
) -> u64 {
    let mask = 0xFFFFu64;
    if clear { imm & !(mask << (index * 16)) } else { imm | (mask << (index * 16)) }
}

/// The 16-bit chunks of `uimm` with their counts, in the order LLVM's
/// `DenseMap<uint64_t, unsigned>` iterates them: 64 buckets, the hash `Val *
/// 37`, quadratic probing.
fn counts_in_map_order(uimm: u64) -> Vec<(u64, u32)> {
    let mut buckets: [Option<(u64, u32)>; 64] = [None; 64];
    for index in 0..4 {
        let key = chunk(uimm, index);
        let mut bucket = (key.wrapping_mul(37) as u32 as usize) & 63;
        let mut probe = 1;
        loop {
            match &mut buckets[bucket] {
                Some((found, count)) if *found == key => {
                    *count += 1;
                    break;
                }
                slot @ None => {
                    *slot = Some((key, 1));
                    break;
                }
                Some(_) => {
                    bucket = (bucket + probe) & 63;
                    probe += 1;
                }
            }
        }
    }
    buckets.into_iter().flatten().collect()
}

fn run_of_ones_starting_at(
    v: u64,
    start: u32,
) -> u64 {
    let ones = (v >> start).trailing_ones();
    let unshifted = if ones == 64 { !0u64 } else { (1u64 << ones) - 1 };
    unshifted << start
}

fn maximally_replicate_sub_immediate(
    v: u64,
    subset: u64,
) -> u64 {
    let mut result = subset;
    // 64, 32, 16, 8, 4, 2
    for i in 0..6u32 {
        let rotation = 1u32 << (6 - i);
        let closure = result | result.rotate_left(rotation);
        if closure != (closure & v) {
            break;
        }
        result = closure;
    }
    result
}

fn maximal_logical_imm_within(
    remaining_bits: u64,
    original_bits: u64,
) -> u64 {
    let position = remaining_bits.trailing_zeros();
    let first_run = run_of_ones_starting_at(original_bits, position);
    maximally_replicate_sub_immediate(original_bits, first_run)
}

fn decompose_into_orr(uimm: u64) -> Option<(u64, u64)> {
    if uimm == 0 || !uimm == 0 {
        return None;
    }
    // A run of ones split around the rotation boundary is made whole.
    let initial_trailing_ones = uimm.trailing_ones();
    let rotated_bits = uimm.rotate_right(initial_trailing_ones);
    let maximal_imm1 = maximal_logical_imm_within(rotated_bits, rotated_bits);
    let remaining_bits = rotated_bits & !maximal_imm1;
    let maximal_imm2 = maximal_logical_imm_within(remaining_bits, rotated_bits);
    if remaining_bits & !maximal_imm2 != 0 {
        return None;
    }
    Some((maximal_imm1.rotate_left(initial_trailing_ones), maximal_imm2.rotate_left(initial_trailing_ones)))
}

/// The value a `N:immr:imms` code stands for in a register of `bits` bits:
/// `decodeLogicalImmediate`.
pub fn decode_logical(
    code: u32,
    bits: u32,
) -> u64 {
    let n = (code >> 12) & 1;
    let immr = (code >> 6) & 0x3F;
    let imms = code & 0x3F;
    assert!(bits == 64 || n == 0, "undefined logical immediate encoding");
    let len = 31 - ((n << 6) | (!imms & 0x3F)).leading_zeros();
    let size = 1u32 << len;
    let r = immr & (size - 1);
    let s = imms & (size - 1);
    assert_ne!(s, size - 1, "undefined logical immediate encoding");
    let mut pattern = (1u64 << (s + 1)) - 1;
    for _ in 0..r {
        pattern = ror(pattern, size);
    }
    // Replicate the element across the register.
    let mut size = size;
    while size != bits {
        pattern |= pattern << size;
        size *= 2;
    }
    pattern
}

fn ror(
    elt: u64,
    size: u32,
) -> u64 {
    ((elt & 1) << (size - 1)) | (elt >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(insns: &[Inst]) -> u64 {
        // Interpret the sequence on one register, which starts at zero.
        let mut value = 0u64;
        for inst in insns {
            match inst.clone() {
                Inst::MoveWide { op, width, imm16, lsl, .. } => {
                    let piece = u64::from(imm16) << lsl;
                    value = match op {
                        Wide::Zero => piece,
                        Wide::Not => !piece,
                        Wide::Keep => (value & !(0xFFFFu64 << lsl)) | piece,
                    };
                    if width == Width::W {
                        value &= 0xFFFF_FFFF;
                    }
                }
                Inst::LogicImm { op, width, rn, imm, .. } => {
                    let source = if rn == Gpr::R31 { 0 } else { value };
                    value = match op {
                        Logic::Orr => source | imm,
                        Logic::And => source & imm,
                        Logic::Eor => source ^ imm,
                        Logic::Ands => unreachable!(),
                    };
                    if width == Width::W {
                        value &= 0xFFFF_FFFF;
                    }
                }
                Inst::LogicShifted { op, invert, amount, .. } => {
                    let other = value << amount;
                    let other = if invert { !other } else { other };
                    value = match op {
                        Logic::Orr => value | other,
                        Logic::Eor => value ^ other,
                        _ => unreachable!(),
                    };
                }
                other => panic!("{other:?}"),
            }
        }
        value
    }

    /// Every sequence puts back the constant it was made for, and takes at most
    /// the four instructions a 64-bit constant can need.
    #[test]
    fn each_sequence_reproduces_its_constant() {
        let mut values: Vec<u64> = vec![
            0,
            1,
            0xFFFF,
            0x1_0000,
            0xFFFF_FFFF,
            0x1234_5678,
            0x1234_5678_9ABC_DEF0,
            0x1234_5678_EDCB_A987,
            0xFFFF_5678_0000_A987,
            0x0000_5678_FFFF_A987,
            0xAAAA_BBBB_AAAA_AAAA,
            0x0000_FFFF_FFFF_FFFF,
            0x00FF_00FF_00FF_00FF,
            0x5555_5555_5555_5554,
            0xDEAD_BEEF_DEAD_BEEF,
            u64::MAX,
            u64::MAX - 1,
            0x8000_0000_0000_0000,
        ];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..4000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state);
            values.push(state & 0x0000_FFFF_0000_FFFF);
            values.push(state | 0xFFFF_0000_0000_FFFF);
        }
        for value in values {
            let long = mov_imm(value, 64, Gpr(64));
            assert_eq!(run(&long), value, "{value:#x}: {long:?}");
            assert!(long.len() <= 4, "{value:#x}: {long:?}");
            let narrow = mov_imm(value & 0xFFFF_FFFF, 32, Gpr(64));
            assert_eq!(run(&narrow), value & 0xFFFF_FFFF, "{value:#x}: {narrow:?}");
            assert!(narrow.len() <= 2, "{value:#x}: {narrow:?}");
        }
    }

    /// The pinned `llc` expands a constant to the instructions we do: the same
    /// words, for constants of each shape the expansion tries.
    #[test]
    fn llc_expands_the_same_constants_the_same_way() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        use crate::mc::encode::encode;
        use crate::mc::tests::{assembled, pinned_mc};

        let Some(mc) = pinned_mc() else {
            eprintln!("skipped: no pinned LLVM with AArch64");
            return;
        };
        let llc = mc.replace("llvm-mc", "llc");
        let mut values: Vec<u64> = vec![
            0x1234_5678_9ABC_DEF0,
            0x1234_5678_EDCB_A987,
            0xFFFF_5678_0000_A987,
            0x0000_5678_FFFF_A987,
            0xAAAA_BBBB_AAAA_AAAA,
            0x00FF_00FF_00FF_00F0,
            0xDEAD_BEEF_DEAD_BEEF,
            0x1_0000_0001,
            0xFFFF_0000_FFFF_0001,
            0x0123_4567_89AB_CDEF,
            0x8000_0000_0000_0001,
            0x7FFF_FFFF_FFFF_FFF0,
            0xFEDC_BA98_7654_3210,
            0x0000_FFFF_0000_FFFE,
        ];
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..150 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.extend([
                state,
                state & 0xFFFF_0000_FFFF_0000,
                state | 0xFFFF_0000_0000_0000,
                state & 0x0000_FFFF_FFFF_0000,
            ]);
        }
        let module: String = values
            .iter()
            .enumerate()
            .map(|(at, value)| format!("define i64 @f{at}() {{\n  ret i64 {}\n}}\n", *value as i64))
            .collect();
        let mut child = Command::new(llc)
            .args(["-mtriple=arm64-apple-macos", "-O2", "-o", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(module.as_bytes()).unwrap();
        let said = child.wait_with_output().unwrap();
        assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
        let text = String::from_utf8_lossy(&said.stdout).into_owned();
        // Each function's instructions, between its label and its ret.
        let mut bodies: Vec<Vec<String>> = Vec::new();
        for line in text.lines() {
            let code = line.split(';').next().unwrap_or("").trim_end();
            if code.starts_with("_f") && code.ends_with(':') {
                bodies.push(Vec::new());
            } else if let Some(body) = bodies.last_mut() {
                let line = code.trim();
                if line.is_empty() || line.starts_with('.') || line == "ret" {
                    continue;
                }
                body.push(line.to_owned());
            }
        }
        // Premise: one body per constant, each with at least one instruction.
        assert_eq!(bodies.len(), values.len());
        assert!(bodies.iter().all(|body| !body.is_empty()));
        let (mut all, mut cuts) = (Vec::new(), Vec::new());
        for body in &bodies {
            all.extend(body.iter().cloned());
            cuts.push(all.len());
        }
        let theirs = assembled(&mc, &all);
        let mut at = 0;
        let mut bad = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let ours: Vec<u32> =
                mov_imm(*value, 64, Gpr(0)).iter().map(|inst| encode(inst).expect("encodes").word).collect();
            if ours != theirs[at..cuts[index]] {
                bad.push(format!(
                    "{value:#x}: ours {ours:08x?}, llc {:08x?}\n{:?}",
                    &theirs[at..cuts[index]],
                    bodies[index]
                ));
            }
            at = cuts[index];
        }
        assert!(bad.is_empty(), "{} of {} differ:\n{}", bad.len(), values.len(), bad.join("\n"));
    }
}
