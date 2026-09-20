//! Structural verification for target-independent MC fragments.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::diagnostic::{Diagnostic, Severity};

use super::{
    DataFragment, Fixup, FragmentId, MCFragment, MCModule, SectionId, SymbolDefinition, SymbolId,
};

/// Verifies deterministic IDs and references without assigning layout.
pub fn verify(module: &MCModule) -> Result<(), Vec<Diagnostic>> {
    let mut verifier = Verifier::default();
    verifier.collect(module);
    verifier.check(module);
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(verifier.diagnostics)
    }
}

impl MCModule {
    pub fn verify(&self) -> Result<(), Vec<Diagnostic>> {
        verify(self)
    }
}

#[derive(Default)]
struct Verifier {
    diagnostics: Vec<Diagnostic>,
    sections: BTreeSet<SectionId>,
    fragments: BTreeMap<FragmentId, SectionId>,
    symbols: BTreeSet<SymbolId>,
}

impl Verifier {
    fn collect(&mut self, module: &MCModule) {
        for section in &module.sections {
            if !self.sections.insert(section.id) {
                self.error(format!("duplicate MC section {}", section.id));
            }
            for fragment in &section.fragments {
                if let Some(previous) = self.fragments.insert(fragment.id(), section.id) {
                    self.error(format!(
                        "duplicate MC fragment {} in sections {} and {}",
                        fragment.id(),
                        previous,
                        section.id
                    ));
                }
            }
        }
        for symbol in &module.symbols {
            if !self.symbols.insert(symbol.id) {
                self.error(format!("duplicate MC symbol {}", symbol.id));
            }
        }
    }

    fn check(&mut self, module: &MCModule) {
        for section in &module.sections {
            self.check_alignment(
                section.alignment,
                format!("section {} alignment", section.id),
            );
            for fragment in &section.fragments {
                match fragment {
                    MCFragment::Data(data) => self.check_data(data),
                    MCFragment::ZeroFill(_) => {}
                    MCFragment::Align(align) => self.check_alignment(
                        align.alignment,
                        format!("fragment {} alignment", align.id),
                    ),
                    MCFragment::Instruction(instruction) => {
                        for fixup in &instruction.fixups {
                            self.check_fixup(instruction.id, fixup);
                        }
                        for operand in &instruction.instruction.operands {
                            if let super::MCOperand::Expression(expression) = operand {
                                self.check_symbol(expression.symbol, "instruction expression");
                            }
                        }
                    }
                }
            }
        }

        for symbol in &module.symbols {
            match symbol.definition {
                SymbolDefinition::Undefined => {}
                SymbolDefinition::SectionOffset { section, .. } => {
                    if !self.sections.contains(&section) {
                        self.error(format!(
                            "symbol {} refers to unknown section {}",
                            symbol.id, section
                        ));
                    }
                }
                SymbolDefinition::Fragment { fragment, .. } => {
                    if !self.fragments.contains_key(&fragment) {
                        self.error(format!(
                            "symbol {} refers to unknown fragment {}",
                            symbol.id, fragment
                        ));
                    }
                }
            }
        }
    }

    fn check_data(&mut self, data: &DataFragment) {
        for fixup in &data.fixups {
            if usize::try_from(fixup.offset)
                .map(|offset| offset >= data.bytes.len())
                .unwrap_or(true)
            {
                self.error(format!(
                    "fragment {} fixup offset {} is outside {} data byte(s)",
                    data.id,
                    fixup.offset,
                    data.bytes.len()
                ));
            }
            self.check_fixup(data.id, fixup);
        }
    }

    fn check_fixup(&mut self, fragment: FragmentId, fixup: &Fixup) {
        self.check_symbol(
            fixup.expression.symbol,
            &format!("fragment {fragment} fixup"),
        );
    }

    fn check_symbol(&mut self, symbol: SymbolId, context: &str) {
        if !self.symbols.contains(&symbol) {
            self.error(format!("{context} refers to unknown symbol {symbol}"));
        }
    }

    fn check_alignment(&mut self, alignment: u32, context: String) {
        if alignment == 0 || !alignment.is_power_of_two() {
            self.error(format!(
                "{context} {alignment} is not a nonzero power of two"
            ));
        }
    }

    fn error(&mut self, message: String) {
        self.diagnostics
            .push(Diagnostic::new(Severity::Error, message));
    }
}

#[cfg(test)]
mod tests {
    use super::verify;
    use crate::mc::{
        DataFragment, Fixup, FixupKind, FragmentId, MCExpression, MCFragment, MCModule, MCSection,
        MCSymbol, SectionFlags, SectionId, SectionKind, SymbolBinding, SymbolDefinition, SymbolId,
        SymbolVisibility,
    };

    fn module() -> MCModule {
        MCModule {
            sections: vec![MCSection {
                id: SectionId::new(1),
                name: ".text".into(),
                kind: SectionKind::Text,
                flags: SectionFlags::ALLOC.union(SectionFlags::EXECUTABLE),
                alignment: 16,
                fragments: vec![MCFragment::Data(DataFragment {
                    id: FragmentId::new(2),
                    bytes: vec![0, 0],
                    fixups: vec![Fixup {
                        offset: 0,
                        kind: FixupKind::new(1),
                        expression: MCExpression {
                            symbol: SymbolId::new(3),
                            addend: 0,
                        },
                        pc_relative: false,
                    }],
                })],
            }],
            symbols: vec![MCSymbol {
                id: SymbolId::new(3),
                name: "external".into(),
                binding: SymbolBinding::Global,
                visibility: SymbolVisibility::Default,
                definition: SymbolDefinition::Undefined,
            }],
        }
    }

    #[test]
    fn accepts_resolved_structure_before_layout() {
        assert_eq!(verify(&module()), Ok(()));
    }

    #[test]
    fn reports_duplicate_ids_bad_alignment_and_unknown_references() {
        let mut module = module();
        module.sections[0].alignment = 3;
        module.sections.push(MCSection {
            id: SectionId::new(1),
            name: ".data".into(),
            kind: SectionKind::Data,
            flags: SectionFlags::ALLOC,
            alignment: 1,
            fragments: vec![MCFragment::Data(DataFragment {
                id: FragmentId::new(2),
                bytes: vec![0],
                fixups: vec![Fixup {
                    offset: 1,
                    kind: FixupKind::new(2),
                    expression: MCExpression {
                        symbol: SymbolId::new(99),
                        addend: 0,
                    },
                    pc_relative: false,
                }],
            })],
        });

        let diagnostics = verify(&module).unwrap_err();

        assert!(
            diagnostics
                .iter()
                .any(|one| one.message.contains("duplicate MC section"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|one| one.message.contains("duplicate MC fragment"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|one| one.message.contains("not a nonzero power"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|one| one.message.contains("outside 1 data byte"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|one| one.message.contains("unknown symbol 99"))
        );
    }
}
