//! The instruction description: every form llrm selects, from `x86.instr`.
//! Generators read the same file at build time through `parse.rs`.

use std::sync::LazyLock;

use iced_x86::{Code, Instruction};
use llrm_support::hash::HashMap;

pub mod effects;
pub mod parse;

pub use parse::{Form, Operand, Side, CONDITIONS};

/// The forms real mode adds to the family's.
pub const OWN: &str = include_str!("x86.instr");

pub static TEXT: LazyLock<String> = LazyLock::new(|| llrm_x86::instructions::joined(OWN));

pub static FORMS: LazyLock<Vec<Form>> = LazyLock::new(|| parse::parse(&TEXT).expect("x86.instr parses"));

static CODES: LazyLock<HashMap<String, Code>> = LazyLock::new(|| Code::values().map(|code| (format!("{code:?}"), code)).collect());

/// Every form of `name`.
pub fn forms(name: &str) -> impl Iterator<Item = &'static Form> {
    FORMS.iter().filter(move |form| form.name == name)
}

/// iced's Code for `form` at `bits`.
pub fn code(form: &Form, bits: Option<u32>) -> Option<Code> {
    CODES.get(&form.code_name(bits)?).copied()
}

/// The flags `form` reads and writes (`RflagsBits`), as iced has them.
/// Undefined and cleared flags are written.
pub fn flags(form: &Form) -> Option<(u32, u32)> {
    let mut instruction = Instruction::default();
    instruction.set_code(code(form, None)?);
    let written = instruction.rflags_written() | instruction.rflags_cleared() | instruction.rflags_set() | instruction.rflags_undefined() | instruction.rflags_modified();
    Some((instruction.rflags_read(), written))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_x86::RflagsBits;

    #[test]
    fn every_form_names_an_iced_code_at_each_width() {
        for form in FORMS.iter() {
            let Some(template) = &form.iced else { continue };
            let widths: Vec<Option<u32>> = if template.contains("{w}") { form.widths.iter().copied().map(Some).collect() } else { vec![None] };
            for bits in widths {
                assert!(code(form, bits).is_some(), "x86.instr:{}: {:?} is no iced Code", form.line, form.code_name(bits));
            }
        }
    }

    #[test]
    fn flags_come_from_iced() {
        let (_, add) = flags(forms("add").next().unwrap()).unwrap();
        assert_eq!(add & RflagsBits::CF, RflagsBits::CF);
        let (read, _) = flags(forms("adc").next().unwrap()).unwrap();
        assert_eq!(read & RflagsBits::CF, RflagsBits::CF);
        let (read, written) = flags(forms("mov").next().unwrap()).unwrap();
        assert_eq!((read, written), (0, 0));
        let (read, _) = flags(forms("jb").next().unwrap()).unwrap();
        assert_eq!(read, RflagsBits::CF);
    }

    #[test]
    fn a_malformed_line_is_refused_with_its_line() {
        let error = parse::parse("add binary rm/^0,rmx 16 alu_rr - - - Add_rm{w}_r{w}").unwrap_err();
        assert_eq!(error, "x86.instr:1: operand `rmx` is not made of the kinds r m i a s");
    }

    /// The joined description is the family's rows and the target's, none lost or repeated.
    #[test]
    fn the_family_and_real_mode_rows_are_all_of_the_description() {
        let rows = |text: &str| text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).count();
        assert_eq!(rows(&TEXT), rows(llrm_x86::instructions::FAMILY) + rows(OWN));
        assert!(forms("les").next().is_some() && forms("call").next().is_some());
    }
}
