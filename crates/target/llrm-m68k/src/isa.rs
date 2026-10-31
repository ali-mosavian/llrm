//! The 68k family's opcode table: which first words each CPU decodes, and as what.
//!
//! # How it works
//! `isa/table.tsv` has one row per mnemonic and operand shape: the cubes
//! (`mask/value`) of first words it covers and the CPUs that decode it. The
//! rows come from binutils' disassembler, and `isa/prm/*.tsv` holds the
//! Motorola reference manual's own bit patterns, transcribed as a second
//! reader; `tests/isa.rs` compares the two.
//!
//! # Limits
//! 1. binutils decodes the FPU and MMU (line F, cpid 0 and 1) under
//!    `-m68000`, which has no coprocessor interface and takes the F-line
//!    exception. The table puts those rows on the 68020 and later only.
//! 2. binutils has no mnemonics for coprocessors 2 to 7. The generic
//!    `cpGEN`, `cpBcc`, `cpDBcc`, `cpScc`, `cpTRAPcc`, `cpSAVE` and
//!    `cpRESTORE` rows are the manual's patterns, on the 68020 and 68030.
//! 3. The MC68851 is a separate chip: its rows (the MMU forms on a 68020) are
//!    the 68020's by the oracle; the 68030's on-chip MMU is the same forms.
//! 4. Extension words of lines 0-E are not modelled (an operand's length
//!    follows from its mode bits). Line F rows carry the command word's cubes.
//! 5. `emulated` lists the CPUs that decode a form but trap it to the
//!    software package (68040's Table 9-10); the 68060's list is
//!    `isa/prm/m68060_support.tsv`.

use std::sync::LazyLock;

/// A CPU of the family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cpu {
    M68000,
    M68010,
    M68020,
    M68030,
    M68040,
    M68060,
}

impl Cpu {
    pub const ALL: [Cpu; 6] = [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060];

    /// The name binutils gives it (`-m m68k:68020`).
    pub fn name(self) -> &'static str {
        ["68000", "68010", "68020", "68030", "68040", "68060"][self as usize]
    }
}

/// One row of the table.
#[derive(Debug)]
pub struct Form {
    pub mnemonic: &'static str,
    pub operands: &'static str,
    /// `(mask, value)`: a first word is in the form when `word & mask == value`.
    pub cubes: Vec<(u16, u16)>,
    /// Cubes of the command word that follows, for the forms that have one.
    pub ext: Vec<(u16, u16)>,
    /// FPU data formats (`bwlspdx`), `-` for none.
    pub formats: &'static str,
    cpus: u8,
    emulated: u8,
}

impl Form {
    pub fn on(
        &self,
        cpu: Cpu,
    ) -> bool {
        self.cpus >> cpu as u8 & 1 != 0
    }

    /// The CPU decodes the form but takes the unimplemented-instruction
    /// exception for it.
    pub fn emulated_on(
        &self,
        cpu: Cpu,
    ) -> bool {
        self.emulated >> cpu as u8 & 1 != 0
    }

    pub fn matches(
        &self,
        word: u16,
    ) -> bool {
        self.cubes.iter().any(|&(mask, value)| word & mask == value)
    }

    /// The command word after the first word is one the form takes.
    pub fn matches_ext(
        &self,
        word: u16,
    ) -> bool {
        self.ext.is_empty() || self.ext.iter().any(|&(mask, value)| word & mask == value)
    }
}

static FORMS: LazyLock<Vec<Form>> = LazyLock::new(|| {
    include_str!("isa/table.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let mut column = line.split('\t');
            let mut next = || column.next().unwrap_or_else(|| panic!("isa/table.tsv: short row {line:?}"));
            let (mnemonic, operands, first, ext) = (next(), next(), next(), next());
            let (formats, cpus, emulated) = (next(), next(), next());
            let hex = |text: &str| u16::from_str_radix(text, 16).unwrap_or_else(|_| panic!("isa/table.tsv: {line:?}"));
            let cubes = |text: &str| -> Vec<(u16, u16)> {
                text.split(' ')
                    .filter(|cube| *cube != "-")
                    .map(|cube| {
                        let (mask, value) = cube.split_once('/').unwrap_or_else(|| panic!("isa/table.tsv: {line:?}"));
                        (hex(mask), hex(value))
                    })
                    .collect()
            };
            let set = |names: &str| {
                names.split(',').filter(|name| *name != "-").fold(0, |set, name| {
                    let cpu = Cpu::ALL.iter().position(|cpu| &cpu.name()[2..] == name);
                    set | 1 << cpu.unwrap_or_else(|| panic!("isa/table.tsv: cpu {name:?}"))
                })
            };
            Form {
                mnemonic,
                operands,
                cubes: cubes(first),
                ext: cubes(ext),
                formats,
                cpus: set(cpus),
                emulated: set(emulated),
            }
        })
        .collect()
});

/// Every form of the table.
pub fn forms() -> &'static [Form] {
    &FORMS
}

/// The forms `cpu` decodes `word` as.
pub fn decode(
    word: u16,
    cpu: Cpu,
) -> impl Iterator<Item = &'static Form> {
    forms().iter().filter(move |form| form.on(cpu) && form.matches(word))
}
