//! What a target says about its registers that the allocator and the passes
//! around it read: which operand of which instruction form lives in which
//! register. Built from the selected target (`RegisterClasses::of`) and handed
//! down beside `Segments`; no pass reads another target's.

use std::collections::BTreeSet;

use iced_x86::Register;
use llrm_target::Target;
use llrm_x86_m16::instructions::{self, Side};

use crate::backend::target::{_on_the_stack, Occurrence, SEGMENTS};
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
    pins: crate::support::hash::HashMap<String, Vec<(&'static str, usize, usize, Vec<(Side, usize, Register)>)>>,
    /// The registers a value may be placed in, whole, in allocation order.
    pub available: Vec<Register>,
    /// The word registers an address is made of (`[bx+si]`): the bases, the
    /// indexes and the frame.
    pub word_bases: BTreeSet<Register>,
    pub word_indexes: BTreeSet<Register>,
    pub frame: Register,
    /// Every register an address may be made of, the frame's included:
    /// `[bx+si]`, `[bp+di]`.
    pub addressing: BTreeSet<Register>,
    /// The bases the encoding permits, the frame's included.
    pub encodable_bases: BTreeSet<Register>,
}

impl RegisterClasses {
    /// The target's: from the `fixed` column of its instruction forms. A pin
    /// that tells the members of a family apart (`les`, `lds`, `lfs` and
    /// `lgs` take the same operands and differ in the selector register) is
    /// a choice the allocator makes, not a requirement, and is left out.
    pub fn of(arch: &dyn Target) -> Self {
        // A form that pins nothing has no requirement to give, and the
        // description is read at every compile.
        let forms = instructions::parse::pinned(&arch.forms_text()).expect("the target's forms parse");
        let operations: HashMap<&str, &'static str> =
            Operation::ALL.iter().map(|op| (op.as_str(), op.as_str())).collect();
        let mut pins: HashMap<Key, Vec<(Side, usize, Register)>> = HashMap::default();
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
            Vec<(&'static str, usize, usize, Vec<(Side, usize, Register)>)>,
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
        Self {
            pins,
            available: held("gpr"),
            word_bases: words("base"),
            word_indexes: words("index"),
            frame: arch.frame_register(),
            addressing,
            encodable_bases,
        }
    }

    /// These classes for a function with no frame register (LLVM's `hasFP`
    /// false): the frame register is one more general register, the last to
    /// be given out, and an address may be made of its word.
    pub fn with_frame_free(&self) -> Self {
        let whole = self.frame;
        let mut free = self.clone();
        if !free.available.contains(&whole) {
            free.available.push(whole);
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
    ) -> IndexMap<Occurrence, Register> {
        let mut out = IndexMap::default();
        if _on_the_stack(what) {
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
                _ => !SEGMENTS.contains(register),
            };
            if pinned {
                out.insert(Occurrence::new(if *side == Side::Dest { "dest" } else { "source" }, *index), *register);
            }
        }
        out
    }
}

/// The register a form's `fixed` column names by its root: `ax` is EAX.
fn root_register(root: &str) -> Register {
    match root {
        "ax" => Register::EAX,
        "bx" => Register::EBX,
        "cx" => Register::ECX,
        "dx" => Register::EDX,
        "si" => Register::ESI,
        "di" => Register::EDI,
        "bp" => Register::EBP,
        "sp" => Register::ESP,
        "es" => Register::ES,
        "ds" => Register::DS,
        "fs" => Register::FS,
        "gs" => Register::GS,
        other => unreachable!("x86.instr names no register `{other}`"),
    }
}

/// The iced register a description names.
fn iced(name: &str) -> Register {
    Register::values()
        .find(|one| format!("{one:?}").eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no register {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `registers.regs` says of m16 is what the allocator's statics and
    /// `llrm_x86_m16`'s constants say.
    #[test]
    fn m16_registers_are_its_description() {
        let classes = RegisterClasses::of(&llrm_x86_m16::M16);
        assert_eq!(classes.available, llrm_x86_m16::GENERAL);
        assert_eq!(classes.word_bases, llrm_x86_m16::word_bases().into_iter().collect());
        assert_eq!(classes.word_indexes, llrm_x86::addressing16::INDEXES.into_iter().collect());
        assert_eq!(classes.frame, llrm_x86_m16::FRAME);
        assert_eq!(classes.encodable_bases, llrm_x86::addressing16::BASES.into_iter().collect());
        assert_eq!(classes.addressing, [Register::BX, Register::BP, Register::SI, Register::DI].into_iter().collect());
    }
}
