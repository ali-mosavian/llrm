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
//! 1. Lines 0-E only; line F (FPU, PMMU, cache, `move16`) is not in the table.
//! 2. Extension words are not modelled: a word is listed for the mode bits it
//!    carries, and its length follows from them.

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
    cpus: u8,
}

impl Form {
    pub fn on(
        &self,
        cpu: Cpu,
    ) -> bool {
        self.cpus >> cpu as u8 & 1 != 0
    }

    pub fn matches(
        &self,
        word: u16,
    ) -> bool {
        self.cubes.iter().any(|&(mask, value)| word & mask == value)
    }
}

static FORMS: LazyLock<Vec<Form>> = LazyLock::new(|| {
    include_str!("isa/table.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let mut column = line.split('\t');
            let mut next = || column.next().unwrap_or_else(|| panic!("isa/table.tsv: short row {line:?}"));
            let (mnemonic, operands, cubes, cpus) = (next(), next(), next(), next());
            let hex = |text: &str| u16::from_str_radix(text, 16).unwrap_or_else(|_| panic!("isa/table.tsv: {line:?}"));
            Form {
                mnemonic,
                operands,
                cubes: cubes
                    .split(' ')
                    .map(|cube| {
                        let (mask, value) = cube.split_once('/').unwrap_or_else(|| panic!("isa/table.tsv: {line:?}"));
                        (hex(mask), hex(value))
                    })
                    .collect(),
                cpus: cpus.split(',').fold(0, |set, name| {
                    let cpu = Cpu::ALL.iter().position(|cpu| &cpu.name()[2..] == name);
                    set | 1 << cpu.unwrap_or_else(|| panic!("isa/table.tsv: cpu {name:?}"))
                }),
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
