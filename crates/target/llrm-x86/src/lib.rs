//! What every x86 target shares. Each target's `x86.instr` holds only the forms
//! it adds to `instructions::FAMILY`.

pub mod select;

/// A calling convention's registers as the x86 family names them: the
/// architecture's, the same in every x86 target.
pub mod calling {
    use iced_x86::Register;
    use llrm_target::calling::Convention;

    /// `name` (`ebx`, `si`, `st0`), as the family spells it.
    pub fn register(name: &str) -> Register {
        static NAMES: std::sync::LazyLock<llrm_support::hash::HashMap<String, Register>> =
            std::sync::LazyLock::new(|| {
                Register::values().map(|one| (format!("{one:?}").to_ascii_lowercase(), one)).collect()
            });
        *NAMES.get(name).unwrap_or_else(|| panic!("calling.toml names no x86 register {name}"))
    }

    /// The most bytes `ret imm16` (and `retf imm16`) removes: the immediate is
    /// a word.
    pub const RET_POPS_MOST: i64 = 0xFFFF;

    /// The register a frame's cells are addressed through.
    pub fn frame(convention: &Convention) -> Register {
        register(&convention.frame)
    }

    pub fn stack(convention: &Convention) -> Register {
        register(&convention.stack)
    }

    /// Each register kept for the caller that a value may be held in: its full
    /// register and the one pushed.
    pub fn callee_saved(convention: &Convention) -> Vec<(Register, Register)> {
        convention.callee_saved().into_iter().map(|kept| (register(&kept.full), register(&kept.pushed))).collect()
    }

    /// The registers a result `width` bytes wide leaves in, low part first.
    pub fn results(
        convention: &Convention,
        width: u32,
    ) -> Vec<Register> {
        convention
            .result_registers(i64::from(width))
            .expect("calling.toml states a result for every width")
            .iter()
            .map(|name| register(name))
            .collect()
    }

    /// The return address a call leaves: the convention's, and a far call's
    /// extra bytes (the far first argument is that much further from the
    /// frame register).
    pub fn return_address_bytes(
        convention: &Convention,
        far: bool,
    ) -> i64 {
        convention.return_address_bytes
            + if far { first_argument_offset(convention, true) - convention.first_argument_offset } else { 0 }
    }

    /// Where register `name`'s low word is in an interrupt handler's frame,
    /// from its pointer: a 16-bit name (`ax`) is the low word of its 32-bit
    /// slot (`eax`).
    pub fn interrupt_slot(
        convention: &Convention,
        name: &str,
    ) -> Option<i64> {
        let wide = format!("e{name}");
        let mut at = 0;
        for (slot, size) in &convention.interrupt_frame {
            if slot == name || (*slot == wide && matches!(name, "ax" | "bx" | "cx" | "dx" | "si" | "di" | "bp" | "sp"))
            {
                return Some(at);
            }
            at += size;
        }
        None
    }

    /// Where the first argument lies from the frame register.
    pub fn first_argument_offset(
        convention: &Convention,
        far: bool,
    ) -> i64 {
        if far {
            convention.first_argument_offset_far.unwrap_or(convention.first_argument_offset)
        } else {
            convention.first_argument_offset
        }
    }
}

/// The x86 general register file's views, in the order iced and the manuals
/// list them: the architecture's, the same in every x86 target.
pub mod registers {
    use iced_x86::Register;

    /// The general registers a value or an address is held in, the frame
    /// pointer's included, the stack pointer's not.
    pub const ROOTS: [Register; 7] =
        [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI, Register::EBP];
    /// The word view of a dword register, where it has one.
    pub fn word_of(dword: Register) -> Option<Register> {
        DWORDS.iter().position(|one| *one == dword).map(|at| WORDS[at])
    }
    /// The segment registers.
    pub const SEGMENTS: [Register; 6] =
        [Register::ES, Register::CS, Register::SS, Register::DS, Register::FS, Register::GS];
    /// The dword registers.
    pub const DWORDS: [Register; 8] = [
        Register::EAX,
        Register::ECX,
        Register::EDX,
        Register::EBX,
        Register::ESI,
        Register::EDI,
        Register::EBP,
        Register::ESP,
    ];
    /// Their low words.
    pub const WORDS: [Register; 8] = [
        Register::AX,
        Register::CX,
        Register::DX,
        Register::BX,
        Register::SI,
        Register::DI,
        Register::BP,
        Register::SP,
    ];
    /// The byte halves of the first four.
    pub const BYTES: [Register; 8] = [
        Register::AL,
        Register::CL,
        Register::DL,
        Register::BL,
        Register::AH,
        Register::CH,
        Register::DH,
        Register::BH,
    ];

    /// A row by register number, as the tables built from it have always been
    /// walked.
    fn in_order(row: &[Register; 8]) -> Vec<Register> {
        let mut sorted = row.to_vec();
        sorted.sort_by_key(|one| *one as u32);
        sorted
    }

    /// The width in bytes each register names.
    pub static WIDTHS: std::sync::LazyLock<llrm_support::hash::IndexMap<Register, i64>> =
        std::sync::LazyLock::new(|| {
            let mut widths = llrm_support::hash::IndexMap::default();
            for (row, size) in [(&DWORDS, 4), (&WORDS, 2), (&BYTES, 1)] {
                for one in in_order(row) {
                    widths.insert(one, size);
                }
            }
            widths
        });

    /// Each register file entry at each width, by its root: the first view of
    /// that width where several share it (AL and AH both root to EAX: the
    /// later one resolved a width-1 value to AH).
    pub static AT_WIDTH: std::sync::LazyLock<
        llrm_support::hash::IndexMap<Register, llrm_support::hash::IndexMap<i64, Register>>,
    > = std::sync::LazyLock::new(|| {
        let mut at_width: llrm_support::hash::IndexMap<Register, llrm_support::hash::IndexMap<i64, Register>> =
            llrm_support::hash::IndexMap::default();
        for (row, size) in [(&DWORDS, 4), (&WORDS, 2), (&BYTES, 1)] {
            for one in in_order(row) {
                at_width.entry(llrm_lir::root(one)).or_default().entry(size).or_insert(one);
            }
        }
        at_width
    });

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Each word is the low half of the dword at its place, and each byte a
        /// half of one of the first four.
        #[test]
        fn the_views_are_the_register_file_iced_knows() {
            for (word, dword) in WORDS.iter().zip(&DWORDS) {
                assert_eq!(word.full_register32(), *dword);
            }
            assert_eq!(word_of(Register::EBX), Some(Register::BX));
            assert_eq!(word_of(Register::AX), None);
            assert!(ROOTS.iter().all(|one| DWORDS.contains(one) && *one != Register::ESP));
            assert!(SEGMENTS.iter().all(|one| one.is_segment_register()));
            for (at, byte) in BYTES.iter().enumerate() {
                assert_eq!(byte.full_register32(), DWORDS[at % 4], "{byte:?}");
            }
        }
    }
}

/// What 16-bit x86 addressing (the ModRM byte without an address-size prefix)
/// is made of: `[base+index+disp]` with a base from BX or BP and an index from
/// SI or DI. The architecture's, the same in every 16-bit target.
pub mod asm;

pub mod addressing16 {
    use iced_x86::Register;

    pub const BASES: [Register; 2] = [Register::BX, Register::BP];
    pub const INDEXES: [Register; 2] = [Register::SI, Register::DI];
}

/// The bytes of the encodings the selector prices for size, where an operand of
/// other than the target's default size (`operand`) takes the 66h prefix.
pub mod encoding {
    fn prefix_bytes(
        width: i64,
        operand: i64,
    ) -> i64 {
        i64::from(width != operand)
    }

    /// Bytes of `op r, r` on `width`-byte registers: the opcode and ModRM.
    pub fn register_bytes(
        width: i64,
        operand: i64,
    ) -> i64 {
        prefix_bytes(width, operand) + 2
    }

    /// Bytes of a shift of a `width`-byte register by `count`: `D1` for one,
    /// `C1` with a byte count otherwise.
    pub fn shift_bytes(
        count: i64,
        width: i64,
        operand: i64,
    ) -> i64 {
        prefix_bytes(width, operand) + if count == 1 { 2 } else { 3 }
    }

    /// Bytes of `imul r, r, number` on `width`-byte registers: a byte immediate
    /// where `number` fits one, else the operand's width.
    pub fn imul_immediate_bytes(
        number: i64,
        width: i64,
        operand: i64,
    ) -> i64 {
        prefix_bytes(width, operand) + 2 + if (-128..=127).contains(&number) { 1 } else { width }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The prefix is for the size that is not the default: a dword in real
        /// mode, a word when flat.
        #[test]
        fn the_operand_size_prefix_is_for_the_size_that_is_not_the_default() {
            assert_eq!(
                (register_bytes(2, 2), register_bytes(4, 2), register_bytes(2, 4), register_bytes(4, 4)),
                (2, 3, 3, 2)
            );
            assert_eq!((shift_bytes(1, 4, 4), shift_bytes(3, 2, 4), imul_immediate_bytes(446, 4, 4)), (2, 4, 6));
        }
    }
}

pub mod instructions {
    /// The forms every x86 target has, `x86.instr`.
    pub const FAMILY: &str = include_str!("instructions/x86.instr");

    /// The family's forms, then `own`: a target's description is the two
    /// joined.
    pub fn joined(own: &str) -> String {
        format!("{FAMILY}\n{own}")
    }
}

/// What DOS gives a program (function numbers, the extender's calls), shared by
/// the targets that run under it.
pub const DOS_FACTS: &str = include_str!("../../../../runtime/shared/dos/facts.toml");

/// The physical addresses `platform.toml` names, each by its name.
pub fn physical_addresses() -> Vec<(String, u64)> {
    let table: toml::Table = include_str!("../platform.toml").parse().expect("platform.toml parses");
    table["physical"]
        .as_table()
        .expect("[physical] is a table")
        .iter()
        .map(|(name, address)| {
            (
                name.clone(),
                u64::try_from(address.as_integer().expect("an address is an integer"))
                    .expect("an address is not negative"),
            )
        })
        .collect()
}

#[cfg(test)]
mod platform_tests {
    #[test]
    fn the_text_screen_is_at_its_physical_address() {
        assert_eq!(super::physical_addresses(), [("text_screen".to_owned(), 0xB8000)]);
    }
}
