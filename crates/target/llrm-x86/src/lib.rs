//! What every x86 target shares. Each target's `x86.instr` holds only the forms
//! it adds to `instructions::FAMILY`.

/// What 16-bit x86 addressing (the ModRM byte without an address-size prefix) is made of: `[base+index+disp]`
/// with a base from BX or BP and an index from SI or DI. The architecture's, the same in every 16-bit target.
pub mod addressing16 {
    use iced_x86::Register;

    pub const BASES: [Register; 2] = [Register::BX, Register::BP];
    pub const INDEXES: [Register; 2] = [Register::SI, Register::DI];
}

/// The bytes of the encodings the selector prices for size, where an operand of other than
/// the target's default size (`operand`) takes the 66h prefix.
pub mod encoding {
    fn prefix_bytes(width: i64, operand: i64) -> i64 {
        i64::from(width != operand)
    }

    /// Bytes of `op r, r` on `width`-byte registers: the opcode and ModRM.
    pub fn register_bytes(width: i64, operand: i64) -> i64 {
        prefix_bytes(width, operand) + 2
    }

    /// Bytes of a shift of a `width`-byte register by `count`: `D1` for one, `C1` with a byte count otherwise.
    pub fn shift_bytes(count: i64, width: i64, operand: i64) -> i64 {
        prefix_bytes(width, operand) + if count == 1 { 2 } else { 3 }
    }

    /// Bytes of `imul r, r, number` on `width`-byte registers: a byte immediate where `number` fits one, else the operand's width.
    pub fn imul_immediate_bytes(number: i64, width: i64, operand: i64) -> i64 {
        prefix_bytes(width, operand) + 2 + if (-128..=127).contains(&number) { 1 } else { width }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The prefix is for the size that is not the default: a dword in real mode, a word when flat.
        #[test]
        fn the_operand_size_prefix_is_for_the_size_that_is_not_the_default() {
            assert_eq!((register_bytes(2, 2), register_bytes(4, 2), register_bytes(2, 4), register_bytes(4, 4)), (2, 3, 3, 2));
            assert_eq!((shift_bytes(1, 4, 4), shift_bytes(3, 2, 4), imul_immediate_bytes(446, 4, 4)), (2, 4, 6));
        }
    }
}

pub mod instructions {
    /// The forms every x86 target has, `x86.instr`.
    pub const FAMILY: &str = include_str!("instructions/x86.instr");

    /// The family's forms, then `own`: a target's description is the two joined.
    pub fn joined(own: &str) -> String {
        format!("{FAMILY}\n{own}")
    }
}

/// The physical addresses `platform.toml` names, each by its name.
pub fn physical_addresses() -> Vec<(String, u64)> {
    let table: toml::Table = include_str!("../platform.toml").parse().expect("platform.toml parses");
    table["physical"]
        .as_table()
        .expect("[physical] is a table")
        .iter()
        .map(|(name, address)| (name.clone(), u64::try_from(address.as_integer().expect("an address is an integer")).expect("an address is not negative")))
        .collect()
}

#[cfg(test)]
mod platform_tests {
    #[test]
    fn the_text_screen_is_at_its_physical_address() {
        assert_eq!(super::physical_addresses(), [("text_screen".to_owned(), 0xB8000)]);
    }
}
