//! `X86AsmBackend::generateCompactUnwindEncoding` for x86-64
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/X86/MCTargetDesc/X86AsmBackend.cpp#L1503>):
//! the 32-bit word of Mach-O's `__compact_unwind` that tells the unwinder how
//! to undo a prologue without the DWARF frame.

use llrm_object::assembler::Cfi;

const MODE_BP_FRAME: u32 = 0x0100_0000;
const MODE_STACK_IMMD: u32 = 0x0200_0000;
const MODE_STACK_IND: u32 = 0x0300_0000;
const MODE_DWARF: u32 = 0x0400_0000;

/// DWARF numbers of the registers.
const RBX: u16 = 3;
const RBP: u16 = 6;
const R12: u16 = 12;
const R15: u16 = 15;

/// The encoding's number of a callee-saved register
/// (`encodeCompactUnwindRegisterNumber`): rbx 1, r12 2, r13 3, r14 4, r15 5,
/// rbp 6.
fn number(register: u16) -> Option<u32> {
    match register {
        RBX => Some(1),
        R12..=R15 => Some(u32::from(register) - 10),
        RBP => Some(6),
        _ => None,
    }
}

/// Bytes of the `push` of `register`: `r12`..`r15` have a REX prefix.
fn push_size(register: u16) -> u32 {
    if (R12..=R15).contains(&register) { 2 } else { 1 }
}

/// `CompactUnwindPermutation`: the order of the saved registers, each as its
/// number less one less the saved registers before it that have a smaller one.
fn permutation(registers: &[u32]) -> u32 {
    let renumbered: Vec<u32> = registers
        .iter()
        .enumerate()
        .map(|(index, &register)| {
            register - 1 - registers[..index].iter().filter(|&&one| one < register).count() as u32
        })
        .collect();
    let weights: &[u32] = match registers.len() {
        6 | 5 => &[120, 24, 6, 2, 1],
        4 => &[60, 12, 3, 1],
        3 => &[20, 4, 1],
        2 => &[5, 1],
        1 => &[1],
        _ => &[],
    };
    renumbered.iter().zip(weights).map(|(one, weight)| one * weight).sum()
}

/// The encoding for a function whose prologue `cfi` states; 0 where it states
/// nothing, `MODE_DWARF` where the compact form cannot say it.
pub fn encoding(cfi: &[Cfi]) -> u32 {
    if cfi.is_empty() {
        return 0;
    }
    let (mut frame, mut offsets, mut saves) = (false, Vec::new(), Vec::new());
    for one in cfi {
        match *one {
            Cfi::DefCfaRegister { register } if register == RBP => frame = true,
            Cfi::DefCfaOffset { offset } => offsets.push(offset),
            Cfi::Offset { register, offset } => saves.push((register, offset)),
            _ => return MODE_DWARF,
        }
    }
    // The frame pointer's own save is the frame, not a register of the list;
    // the others are listed from the deepest slot up.
    if frame {
        saves.retain(|&(register, _)| register != RBP);
    }
    saves.sort_by_key(|&(_, offset)| offset);
    let Some(numbers) = saves.iter().map(|&(register, _)| number(register)).collect::<Option<Vec<_>>>() else {
        return MODE_DWARF;
    };
    if frame {
        if numbers.len() > 5 {
            return MODE_DWARF;
        }
        let deepest = saves.first().map_or(16, |&(_, offset)| -offset);
        let packed: u32 = numbers.iter().enumerate().map(|(slot, &register)| register << (3 * slot)).sum();
        return MODE_BP_FRAME | (((deepest - 16) / 8) as u32) << 16 | packed;
    }
    let Some(&size) = offsets.last() else { return MODE_DWARF };
    if numbers.len() > 6 {
        return MODE_DWARF;
    }
    let tail = (numbers.len() as u32) << 10 | permutation(&numbers);
    if size / 8 <= 0xFF {
        return MODE_STACK_IMMD | ((size / 8) as u32) << 16 | tail;
    }
    // A stack too big for the field: the unwinder reads the `sub rsp, imm32`
    // at the byte it names, and adds the pushes and the return address.
    let before = if offsets.len() >= 2 { offsets[offsets.len() - 2] } else { 8 };
    let adjust = (before / 8) as u32;
    let pushes: u32 = saves.iter().map(|&(register, _)| push_size(register)).sum();
    if adjust > 7 {
        return MODE_DWARF;
    }
    MODE_STACK_IND | (3 + pushes) << 16 | adjust << 13 | tail
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every function in the pin's fixture file encodes as pin clang's
    /// `__compact_unwind` says, frame pointer or not.
    #[test]
    fn the_encoding_is_the_pins_for_every_fixture() {
        let mut checked = 0;
        for line in include_str!("fixtures/compact_unwind_x86_64.txt").lines().filter(|one| !one.starts_with('#')) {
            let (want, ops) = line.split_once(' ').unwrap_or((line, ""));
            let cfi: Vec<Cfi> = ops
                .split(';')
                .filter(|one| !one.is_empty())
                .map(|op| {
                    let words: Vec<&str> = op.split(' ').collect();
                    match words[0] {
                        "offset" => Cfi::DefCfaOffset { offset: words[1].parse().unwrap() },
                        "register" => Cfi::DefCfaRegister { register: words[1].parse().unwrap() },
                        _ => Cfi::Offset { register: words[1].parse().unwrap(), offset: words[2].parse().unwrap() },
                    }
                })
                .collect();
            let want = u32::from_str_radix(want, 16).unwrap();
            assert_eq!(encoding(&cfi), want, "{line}: got {:08x}", encoding(&cfi));
            checked += 1;
        }
        assert!(checked >= 70, "{checked}");
    }

    /// What the compact form cannot say goes to the DWARF frame.
    #[test]
    fn a_frame_in_another_register_is_left_to_dwarf() {
        assert_eq!(encoding(&[Cfi::DefCfaRegister { register: 5 }]), MODE_DWARF);
        assert_eq!(encoding(&[Cfi::DefCfa { register: 7, offset: 8 }]), MODE_DWARF);
        assert_eq!(encoding(&[]), 0);
    }
}
