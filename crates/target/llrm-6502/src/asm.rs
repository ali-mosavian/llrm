//! A function's 6502 code as typed lines: labels and instructions with an
//! opcode and an addressing form, in place of printed text.
//!
//! # How it works
//! The emitter writes into an [`Asm`] as it did into a `String` (`writeln!`,
//! `push_str`); each finished line is parsed into a [`Line`] once. The passes
//! of [`crate::peephole`] and [`crate::relax`] then ask an [`Inst`] what it
//! does to the registers and flags ([`Op::flags`]), how many bytes it takes
//! ([`Inst::bytes`]) and where it branches, and never look at a mnemonic's
//! spelling. [`Asm`] prints back the text it was given.
//!
//! A line that is not an instruction of the table below nor a label is kept as
//! [`Line::Raw`]: the passes treat it as a barrier and size it as three bytes.
//!
//! # Example
//! ```text
//!   "  sta rc5\n  lda (rc2),y\n"  ->  [Inst(Sta, Direct "rc5"), Inst(Lda, IndirectY "rc2")]
//! ```

use std::fmt;

macro_rules! opcodes {
    ($($variant:ident $name:literal),* $(,)?) => {
        /// The 6502 and 65C02 opcodes the emitter prints.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Op { $($variant),* }

        impl Op {
            /// The opcode spelled `name`, `None` for one not in the table.
            pub fn parse(name: &str) -> Option<Op> {
                match name { $($name => Some(Op::$variant),)* _ => None }
            }

            /// The mnemonic.
            pub fn name(self) -> &'static str {
                match self { $(Op::$variant => $name),* }
            }
        }
    };
}

opcodes! {
    Adc "adc", And "and", Asl "asl", Bcc "bcc", Bcs "bcs", Beq "beq", Bit "bit", Bmi "bmi", Bne "bne", Bpl "bpl",
    Bra "bra", Bvc "bvc", Bvs "bvs", Clc "clc", Cld "cld", Clv "clv", Cmp "cmp", Cpx "cpx", Cpy "cpy", Dec "dec",
    Dex "dex", Dey "dey", Eor "eor", Inc "inc", Inx "inx", Iny "iny", Jmp "jmp", Jsr "jsr", Lda "lda", Ldx "ldx",
    Ldy "ldy", Lsr "lsr", Nop "nop", Ora "ora", Pha "pha", Php "php", Phx "phx", Phy "phy", Pla "pla", Plp "plp",
    Plx "plx", Ply "ply", Rol "rol", Ror "ror", Rts "rts", Sbc "sbc", Sec "sec", Sed "sed", Sei "sei", Sta "sta",
    Stx "stx", Sty "sty", Stz "stz", Tax "tax", Tay "tay", Tsx "tsx", Txa "txa", Txs "txs", Tya "tya",
}

/// What an instruction does to the accumulator and the zero and sign flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flags {
    /// Neither `a` nor the flags change: a store, a carry flag, `pha`, `nop`.
    Keeps,
    /// The flags describe `a` afterwards.
    OfA,
    /// The flags are set from some other result, or clobbered (a call).
    Sets,
    /// A branch, a jump or an instruction that may read the flags.
    Reads,
}

impl Op {
    /// What the opcode does to the zero and sign flags.
    pub fn flags(
        self,
        operand: &Operand,
    ) -> Flags {
        use Op::*;
        match self {
            Sta | Stx | Sty | Stz | Clc | Sec | Pha | Nop => Flags::Keeps,
            Lda | Txa | Tya | Tax | Tay | Pla | And | Ora | Eor | Adc | Sbc => Flags::OfA,
            Asl | Lsr | Rol | Ror | Inc | Dec if *operand == Operand::Acc => Flags::OfA,
            Ldx | Ldy | Cmp | Cpx | Cpy | Asl | Lsr | Rol | Ror | Inc | Dec | Inx | Iny | Dex | Dey | Bit | Jsr
            | Rts => Flags::Sets,
            _ => Flags::Reads,
        }
    }

    /// The branch that is taken when this one is not, for a conditional branch.
    pub fn opposite(self) -> Option<Op> {
        use Op::*;
        Some(match self {
            Beq => Bne,
            Bne => Beq,
            Bcc => Bcs,
            Bcs => Bcc,
            Bmi => Bpl,
            Bpl => Bmi,
            Bvc => Bvs,
            Bvs => Bvc,
            _ => return None,
        })
    }

    /// Whether the opcode is a conditional branch or `bra`.
    pub fn is_branch(self) -> bool {
        self == Op::Bra || self.opposite().is_some()
    }
}

/// An index register.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Index {
    X,
    Y,
}

/// An operand and its addressing form. A name is a zero-page cell (`rc2`), a
/// spill cell, a label or a symbol; [`Operand::zero_page`] tells which.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Operand {
    None,
    /// `a`, for a shift or `inc`/`dec`.
    Acc,
    /// `#` and the expression.
    Imm(String),
    Direct(String),
    Indexed(String, Index),
    /// `(name),y`
    IndirectY(String),
    /// `(name,x)`
    IndirectX(String),
    /// `(name)`
    Indirect(String),
}

impl Operand {
    /// The operand written `text`.
    pub fn parse(text: &str) -> Operand {
        if text.is_empty() {
            return Operand::None;
        }
        if text == "a" {
            return Operand::Acc;
        }
        if let Some(value) = text.strip_prefix('#') {
            return Operand::Imm(value.to_owned());
        }
        if let Some(inner) = text.strip_prefix('(') {
            if let Some(name) = inner.strip_suffix("),y") {
                return Operand::IndirectY(name.to_owned());
            }
            if let Some(name) = inner.strip_suffix(",x)") {
                return Operand::IndirectX(name.to_owned());
            }
            if let Some(name) = inner.strip_suffix(')') {
                return Operand::Indirect(name.to_owned());
            }
        }
        for (suffix, index) in [(",x", Index::X), (",y", Index::Y)] {
            if let Some(name) = text.strip_suffix(suffix) {
                return Operand::Indexed(name.to_owned(), index);
            }
        }
        Operand::Direct(text.to_owned())
    }

    /// The cell or symbol an operand names, `None` for an immediate, `a` or no
    /// operand.
    pub fn name(&self) -> Option<&str> {
        match self {
            Operand::None | Operand::Acc | Operand::Imm(_) => None,
            Operand::Direct(n)
            | Operand::Indexed(n, _)
            | Operand::IndirectY(n)
            | Operand::IndirectX(n)
            | Operand::Indirect(n) => Some(n),
        }
    }

    /// Whether the operand names an imaginary register `rcN`, which the
    /// platform's equates place in zero page.
    pub fn zero_page(&self) -> bool {
        self.name().is_some_and(|n| {
            n.strip_prefix("rc").is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        })
    }

    /// Whether the operand is a plain `rcN` or spill cell (`s_...`): a byte
    /// the code owns, addressed directly.
    pub fn is_cell(&self) -> bool {
        match self {
            Operand::Direct(n) => n.starts_with("rc") || n.starts_with("s_"),
            _ => false,
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Operand::None => Ok(()),
            Operand::Acc => f.write_str("a"),
            Operand::Imm(v) => write!(f, "#{v}"),
            Operand::Direct(n) => f.write_str(n),
            Operand::Indexed(n, Index::X) => write!(f, "{n},x"),
            Operand::Indexed(n, Index::Y) => write!(f, "{n},y"),
            Operand::IndirectY(n) => write!(f, "({n}),y"),
            Operand::IndirectX(n) => write!(f, "({n},x)"),
            Operand::Indirect(n) => write!(f, "({n})"),
        }
    }
}

/// One instruction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Inst {
    pub op: Op,
    pub operand: Operand,
}

impl Inst {
    pub fn new(
        op: Op,
        operand: Operand,
    ) -> Inst {
        Inst { op, operand }
    }

    /// What it does to the zero and sign flags.
    pub fn flags(&self) -> Flags {
        self.op.flags(&self.operand)
    }

    /// The label a branch or jump goes to.
    pub fn target(&self) -> Option<&str> {
        match (&self.operand, self.op) {
            (Operand::Direct(label), Op::Jmp) => Some(label),
            (Operand::Direct(label), op) if op.is_branch() => Some(label),
            _ => None,
        }
    }

    /// Its size in bytes, at most what the assembler emits: a form the
    /// assembler picks (zero page or absolute) is counted as absolute unless
    /// the operand is a zero-page register `rcN`.
    pub fn bytes(&self) -> usize {
        use Op::*;
        match (&self.operand, self.op) {
            (Operand::None | Operand::Acc, _) => 1,
            (_, Jmp | Jsr) => 3,
            (_, op) if op.is_branch() => 2,
            (Operand::Imm(_), _) => 2,
            (Operand::IndirectY(_) | Operand::IndirectX(_), _) if self.operand.zero_page() => 2,
            // zero page has `m,x` for all but `ldx` and `stx`, and `m,y` for
            // those
            (Operand::Indexed(_, index), op) if self.operand.zero_page() => {
                let has = match index {
                    Index::X => !matches!(op, Ldx | Stx),
                    Index::Y => matches!(op, Ldx | Stx),
                };
                if has { 2 } else { 3 }
            }
            (Operand::Direct(_), _) if self.operand.zero_page() => 2,
            _ => 3,
        }
    }
}

impl fmt::Display for Inst {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self.operand {
            Operand::None => write!(f, "  {}", self.op.name()),
            _ => write!(f, "  {} {}", self.op.name(), self.operand),
        }
    }
}

/// A line of a function.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Line {
    Label(String),
    Inst(Inst),
    /// A directive or anything else: a barrier for the passes.
    Raw(String),
}

impl Line {
    /// The line printed `text`, without its newline.
    pub fn parse(text: &str) -> Line {
        if let Some(body) = text.strip_prefix("  ") {
            let (name, operand) = body.trim().split_once(' ').unwrap_or((body.trim(), ""));
            if let Some(op) = Op::parse(name) {
                return Line::Inst(Inst::new(op, Operand::parse(operand.trim())));
            }
        } else if let Some(label) = text.strip_suffix(':').filter(|l| !l.is_empty() && !l.contains(' ')) {
            return Line::Label(label.to_owned());
        }
        Line::Raw(text.to_owned())
    }

    pub fn inst(&self) -> Option<&Inst> {
        match self {
            Line::Inst(inst) => Some(inst),
            _ => None,
        }
    }
}

impl fmt::Display for Line {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Line::Label(label) => write!(f, "{label}:"),
            Line::Inst(inst) => inst.fmt(f),
            Line::Raw(text) => f.write_str(text),
        }
    }
}

/// The lines of a function, written to as text and kept as [`Line`]s.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Asm {
    pub lines: Vec<Line>,
    /// What was written since the last newline.
    pending: String,
}

impl Asm {
    pub fn new() -> Asm {
        Asm::default()
    }

    /// The lines of `text`.
    pub fn parse(text: &str) -> Asm {
        let mut asm = Asm::new();
        asm.push_str(text);
        asm
    }

    /// Appends `text`; each line it completes is parsed.
    pub fn push_str(
        &mut self,
        text: &str,
    ) {
        self.pending.push_str(text);
        while let Some(end) = self.pending.find('\n') {
            let line: String = self.pending.drain(..=end).collect();
            self.lines.push(Line::parse(&line[..line.len() - 1]));
        }
    }

    /// The last line written, if it is an instruction.
    pub fn last_inst(&self) -> Option<&Inst> {
        self.lines.last().and_then(Line::inst)
    }

    /// Removes the last line.
    pub fn pop(&mut self) -> Option<Line> {
        self.lines.pop()
    }

    /// The text of the lines.
    pub fn text(&self) -> String {
        self.to_string()
    }
}

impl fmt::Write for Asm {
    fn write_str(
        &mut self,
        text: &str,
    ) -> fmt::Result {
        self.push_str(text);
        Ok(())
    }
}

impl fmt::Display for Asm {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        f.write_str(&self.pending)
    }
}
