//! What the selected target says about its registers, for the allocator and
//! the passes around it.
//!
//! # How it works
//! `RegisterClasses::of` reads the target's register file and its
//! instruction forms once. From the register file: the registers a value may
//! sit in (`available`, in allocation order), the word registers an address
//! is made of (`word_bases`, `word_indexes`, `addressing`,
//! `encodable_bases`), the frame register, and the words whose low byte has
//! a register of its own (`byte_words`). From the forms' `fixed` column:
//! which operand of which form lives in which register (`requirements`),
//! such as a divide's `ax` and `dx` or a shift's `cl`.
//!
//! A pin that only tells members of a family apart (`les`, `lds`, `lfs` and
//! `lgs` take the same operands and differ in the selector) is a choice the
//! allocator makes, so it is left out. A pinned immediate is no register, and
//! a segment register is pinned only while it is a held value.
//!
//! `with_frame_free` is the same classes for a function with no frame
//! register: the frame register becomes one more general register, last to
//! be given out, and an address may be made of its word.
//!
//! # Example
//! What `RegisterClasses::of` says of the real-mode target:
//!
//! ```text
//! addressing   BX BP SI DI
//! status_word  AX
//! ```
//!
//! From unit tests `m16_registers_are_its_description` and
//! `the_status_word_is_where_the_form_table_stores_it` in `classes.rs`.
//!
//! # Used by
//! `allocate.rs`, `coalesce.rs`, `constrain.rs`, `regclass.rs`, the spiller
//! (`ssaspill.rs`), `floatalloc.rs`, `sharedstores.rs`,
//! `loopslots.rs` and `peephole.rs`. `assemble.rs` builds them per module and
//! `flow.rs` hands them to each phase beside `Segments`; no pass reads
//! another target's.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_target::Target;
use llrm_x86::parse::{self, Side};

use crate::backend::target::{_on_the_stack, Occurrence};
use crate::model::ir::{Loc, Operation, Semantics};
use crate::support::hash::HashMap;
use crate::support::hash::IndexMap;

type Key = (String, &'static str, usize, usize);

#[derive(Clone)]
pub struct RegisterClasses {
    /// Each form's pins, by the mnemonic, the operation and the operand counts
    /// that pick it out.
    /// By mnemonic first: asked of every instruction of a body, over and over,
    /// so no key is made of it.
    pins: crate::support::hash::HashMap<String, Vec<(&'static str, usize, usize, Vec<(Side, usize, RegId)>)>>,
    /// The registers a value may be placed in, whole, in allocation order.
    pub available: Vec<RegId>,
    /// The word registers an address is made of (`[bx+si]`): the bases, the
    /// indexes and the frame.
    pub word_bases: BTreeSet<RegId>,
    pub word_indexes: BTreeSet<RegId>,
    pub frame: RegId,
    /// Every register an address may be made of, the frame's included:
    /// `[bx+si]`, `[bp+di]`.
    pub addressing: BTreeSet<RegId>,
    /// The bases the encoding permits, the frame's included.
    pub encodable_bases: BTreeSet<RegId>,
    /// The SSE registers a `float` or `double` may be placed in, in
    /// allocation order; none where the target keeps floats on the x87 stack.
    pub xmm: Vec<RegId>,
    pub xmm_set: BTreeSet<RegId>,
    /// The words whose low byte has a register of its own.
    pub byte_words: BTreeSet<RegId>,
    /// Where the x87 status word is stored (`fnstsw`'s register).
    status_word: Option<RegId>,
    /// The register file of the target these classes are of.
    pub registers: llrm_lir::registers::Regs,
    /// The register the frame register follows in the order once it is free
    /// (`frame_follows` in the register file).
    frame_after: Option<RegId>,
    /// The pressure sets and limits of the target, where it has them.
    pub pressure: Option<(&'static llrm_target::pressure::PressureSets, Vec<u32>)>,
}

impl RegisterClasses {
    /// The target's: from the `fixed` column of its instruction forms. A pin
    /// that tells the members of a family apart (`les`, `lds`, `lfs` and
    /// `lgs` take the same operands and differ in the selector register) is
    /// a choice the allocator makes, not a requirement, and is left out.
    pub fn of(arch: &dyn Target) -> Self {
        // A form that pins nothing has no requirement to give, and the
        // description is read at every compile.
        let forms = parse::pinned(&arch.forms_text(), arch.dialect()).expect("the target's forms parse");
        let operations: HashMap<&str, &'static str> =
            Operation::ALL.iter().map(|op| (op.as_str(), op.as_str())).collect();
        let mut pins: HashMap<Key, Vec<(Side, usize, RegId)>> = HashMap::default();
        for form in &forms {
            let chosen = |side: Side, index: usize, root: &str| {
                forms
                    .iter()
                    .any(
                        |other| other.operation == form.operation
                            && other.dests == form.dests
                            && other.sources == form.sources
                            && other.fixed.iter().any(|(s, i, r)| *s == side && *i == index && r != root),
                    )
            };
            let required = form
                .fixed
                .iter()
                .filter(|(side, index, root)| !chosen(*side, *index, root))
                .map(|(side, index, root)| (*side, *index, root_register(root)))
                .collect();
            pins.entry((form.name.clone(), operations[form.operation.as_str()], form.dests.len(), form.sources.len()))
                .or_insert(required);
        }
        let mut by_name: crate::support::hash::HashMap<
            String,
            Vec<(&'static str, usize, usize, Vec<(Side, usize, RegId)>)>,
        > = crate::support::hash::HashMap::default();
        for ((name, operation, dests, sources), required) in pins {
            by_name.entry(name).or_default().push((operation, dests, sources, required));
        }
        let pins = by_name;
        let file = llrm_target::registers::parse(&arch.registers_text()).expect("the target's registers parse");
        let named = |name: &str| iced(name);
        let word = |root: &str| {
            file.iter()
                .find(|one| one.root == root && one.bits == 16)
                .map(|one| iced(&one.name))
                .expect("a register has a word view")
        };
        let held =
            |class: &str| llrm_target::registers::of_class(&file, class).into_iter().map(named).collect::<Vec<_>>();
        let words = |class: &str| {
            llrm_target::registers::of_class(&file, class)
                .into_iter()
                .filter(|root| !file.iter().any(|one| one.name == *root && one.is("reserved")))
                .map(word)
                .collect()
        };
        let every =
            |class: &str| llrm_target::registers::of_class(&file, class).into_iter().map(word).collect::<BTreeSet<_>>();
        let (encodable_bases, indexes) = (every("base"), every("index"));
        let addressing = encodable_bases.union(&indexes).copied().collect();
        let mut classes = Self {
            pins,
            available: held("gpr"),
            word_bases: words("base"),
            word_indexes: words("index"),
            frame: arch.frame_register(),
            addressing,
            encodable_bases,
            xmm_set: held("xmm").into_iter().collect(),
            xmm: held("xmm"),
            byte_words: {
                let regs = llrm_lir::registers::Regs(arch.registers());
                regs.entries()
                    .filter(|(id, one)| one.classes & llrm_lir::registers::class::BYTE != 0 && one.root == *id)
                    .filter_map(|(id, _)| regs.view(id, 16))
                    .collect()
            },
            status_word: None,
            registers: llrm_lir::registers::Regs(arch.registers()),
            frame_after: file.iter().find(|one| one.is("frame_follows")).map(|one| named(&one.name)),
            pressure: arch.pressure(),
        };
        classes.status_word = classes.found_status_word();
        classes
    }

    /// These classes for a function with no frame register (LLVM's `hasFP`
    /// false): the frame register is one more general register, the last to
    /// be given out, and an address may be made of its word.
    pub fn with_frame_free(&self) -> Self {
        // The root the register file names a family by (`ebp`, whatever the
        // target calls its frame register: `rbp` in long mode).
        let whole = self.registers.root(self.frame);
        let mut free = self.clone();
        if !free.available.contains(&whole) {
            // The pin's order (`X86RegisterInfo.td`): the frame register
            // follows the register the file names, ahead of the rest.
            let at = free
                .available
                .iter()
                .position(|one| Some(*one) == self.frame_after)
                .map_or(free.available.len(), |kept| kept + 1);
            free.available.insert(at, whole);
        }
        if let Some(word) = llrm_x86::registers::word_of(whole) {
            free.word_bases.insert(word);
            free.word_indexes.insert(word);
        }
        free
    }

    /// 16-bit x86's, which the tests of this crate are written for.
    #[cfg(test)]
    pub fn m16() -> std::rc::Rc<Self> {
        std::rc::Rc::new(Self::of(&llrm_x86_m16::M16))
    }

    /// The register the x87 status word is stored in (`fnstsw`'s fixed
    /// destination); the flags reach `sahf` through it.
    pub fn status_word(&self) -> Option<RegId> {
        self.status_word
    }

    fn found_status_word(&self) -> Option<RegId> {
        let regs = self.registers;
        let forms = self.pins.get("fnstsw")?;
        let (_, _, _, pins) =
            forms.iter().find(|(op, dests, sources, _)| *op == "barrier" && *dests == 1 && *sources == 0)?;
        let (_, _, register) = pins.iter().find(|(side, index, _)| matches!(side, Side::Dest) && *index == 0)?;
        // The status word is 16 bits whatever width the form pins its register
        // at.
        regs.view(regs.root(*register), 16)
    }

    /// Every operand this instruction requires in one particular register.
    ///
    /// The one place those are written down: the `fixed` column of the form
    /// that takes this instruction's operands; `reads` and `writes` read it
    /// too. An operand that is an immediate is no register: a shift's count
    /// or an `in`'s port written as one pins nothing, and a segment
    /// register is pinned only while it is a held value, a placed one being
    /// where it is.
    pub fn requirements(
        &self,
        what: &Semantics,
    ) -> IndexMap<Occurrence, RegId> {
        let regs = self.registers;
        let mut out = IndexMap::default();
        if _on_the_stack(regs, what) {
            return out;
        }
        let Some(name) = what.name.as_deref() else { return out };
        let operation = what.op.as_str();
        let Some(pins) = self
            .pins
            .get(name)
            .and_then(|forms| {
                forms.iter().find(|(op, dests, sources, _)| {
                    *op == operation && *dests == what.dests.len() && *sources == what.sources.len()
                })
            })
            .map(|(_, _, _, pins)| pins)
        else {
            return out;
        };
        for (side, index, register) in pins {
            let places = match side {
                Side::Dest => &what.dests,
                Side::Source => &what.sources,
            };
            let pinned = match &places[*index] {
                Loc::Imm(_) => false,
                Loc::Held(_) => true,
                _ => !self.registers.is_segment(*register),
            };
            if pinned {
                out.insert(Occurrence::new(if *side == Side::Dest { "dest" } else { "source" }, *index), *register);
            }
        }
        out
    }
}

/// The register a form's `fixed` column names by its root: `ax` is EAX.
fn root_register(root: &str) -> RegId {
    match root {
        "ax" => RegId::EAX,
        "bx" => RegId::EBX,
        "cx" => RegId::ECX,
        "dx" => RegId::EDX,
        "si" => RegId::ESI,
        "di" => RegId::EDI,
        "bp" => RegId::EBP,
        "sp" => RegId::ESP,
        "es" => RegId::ES,
        "ds" => RegId::DS,
        "fs" => RegId::FS,
        "gs" => RegId::GS,
        other => unreachable!("x86.instr names no register `{other}`"),
    }
}

/// The iced register a description names.
fn iced(name: &str) -> RegId {
    RegId::values()
        .find(|one| format!("{one:?}").eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no register {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `registers.regs` says of m16 is what the allocator's statics and
    /// `llrm_x86_m16`'s constants say.
    /// The x87 status word's register is the `fnstsw` form's fixed destination.
    #[test]
    fn the_status_word_is_where_the_form_table_stores_it() {
        assert_eq!(RegisterClasses::of(&llrm_x86_m16::M16).status_word(), Some(RegId::AX));
    }

    #[test]
    fn m16_registers_are_its_description() {
        let classes = RegisterClasses::of(&llrm_x86_m16::M16);
        assert_eq!(classes.available, llrm_x86_m16::GENERAL);
        assert_eq!(classes.word_bases, llrm_x86_m16::word_bases().into_iter().collect());
        assert_eq!(classes.word_indexes, llrm_x86::addressing16::INDEXES.into_iter().collect());
        assert_eq!(classes.frame, llrm_x86_m16::FRAME);
        assert_eq!(classes.encodable_bases, llrm_x86::addressing16::BASES.into_iter().collect());
        assert_eq!(classes.addressing, [RegId::BX, RegId::BP, RegId::SI, RegId::DI].into_iter().collect());
    }
}
