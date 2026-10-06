//! Inline assembly (section 15): one HIR `asm` instruction whose operands
//! go into registers and whose results come out of them. A register is a whole (16 bits, or 32 where
//! the target's register file names an e-register) and a byte register is a part of it, packed and
//! unpacked here, so the machine side sees only wholes. Which registers a block may name is the
//! target's register file's (`registers.regs`).

use super::*;
use crate::syntax::{Asm, AsmTarget};
use llrm_core::abi::runtime::Reg;
use llrm_core::backend::inline_asm::{self, Part};

impl FunctionCompiler<'_> {
    pub(super) fn asm_statement(&mut self, asm: &Asm) -> Result<(), Diagnostic> {
        self.require_unsafe("inline assembly", asm.span)?;
        let lines: Vec<&str> = asm.lines.iter().map(|(text, _)| text.as_str()).collect();
        let code = inline_asm::assembled(&lines, inline_asm::Mode { bits: self.types.code_bits, segmented: self.types.sizes.segmented, address_bytes: self.types.sizes.near })
            .map_err(|refused| Diagnostic::new(asm.lines[refused.line].1, refused.message))?;

        // Each register's input: whole, or its bytes.
        let mut words: Vec<(Reg, u32, [Option<hir::Operand>; 3])> = Vec::new();
        for (name, value, span) in &asm.inputs {
            let (register, part, bits) = self.operand(name, *span)?;
            let value = self.register_value(value, part, bits, name, *span)?;
            let at = match words.iter().position(|(one, ..)| *one == register) {
                Some(at) if words[at].1 != bits => return Err(Diagnostic::new(*span, format!("{name} overlaps another input"))),
                Some(at) => at,
                None => {
                    words.push((register, bits, [None, None, None]));
                    words.len() - 1
                }
            };
            let parts = &mut words[at].2;
            let whole = part == Part::Word || parts[Part::Word as usize].is_some();
            if parts[part as usize].is_some() || (whole && parts.iter().any(Option::is_some)) {
                return Err(Diagnostic::new(*span, format!("{name} overlaps another input")));
            }
            parts[part as usize] = Some(value);
        }
        let mut inputs = Vec::new();
        let mut operands = Vec::new();
        for (register, bits, [word, low, high]) in words {
            let whole = whole_type(bits);
            let low = low.map(|one| self.resized(one, TypeName::U8, whole));
            let high = high.map(|one| {
                let widened = self.resized(one, TypeName::U8, whole);
                self.bit_shift("shl", widened, 8, whole)
            });
            let value = match (word, low, high) {
                (Some(word), ..) => word,
                (None, Some(low), Some(high)) => self.binary_op("or", low, high, whole),
                (None, Some(part), None) | (None, None, Some(part)) => part,
                (None, None, None) => unreachable!("a register is listed for an input"),
            };
            inputs.push(register_name(register, bits));
            operands.push(value);
        }

        let mut outputs: Vec<(Reg, u32)> = Vec::new();
        for (index, (name, _, span)) in asm.outputs.iter().enumerate() {
            let (register, _, bits) = self.operand(name, *span)?;
            if asm.outputs[..index].iter().any(|(other, ..)| other.eq_ignore_ascii_case(name)) {
                return Err(Diagnostic::new(*span, format!("{name} is an output twice")));
            }
            match outputs.iter().find(|(one, _)| *one == register) {
                Some((_, taken)) if *taken != bits => return Err(Diagnostic::new(*span, format!("{name} overlaps another output"))),
                Some(_) => {}
                None => outputs.push((register, bits)),
            }
        }
        let mut memory = false;
        let mut clobbers = Vec::new();
        for (name, span) in &asm.clobbers {
            if name == "memory" {
                memory = true;
                continue;
            }
            // `es` and `flags` are not in the file as a general register; any other the target's file must have.
            let register = match name.to_ascii_lowercase().as_str() {
                "es" | "flags" => inline_asm::clobbered(name),
                _ => self.operand(name, *span).ok().map(|(register, ..)| register),
            }
            .ok_or_else(|| Diagnostic::new(*span, format!("{name} is not a register a block may clobber: the target's general registers, their bytes, es, flags or memory")))?;
            clobbers.push(register_name(register, 16));
        }

        let results: Vec<u32> = outputs.iter().map(|(_, bits)| self.value(whole_type(*bits))).collect();
        self.emit("asm", results.clone(), operands, None);
        let instruction = self.current_block_mut().instructions.last_mut().expect("the asm instruction");
        instruction.asm = Some(hir::Asm {
            code,
            inputs,
            outputs: outputs.iter().map(|(one, bits)| register_name(*one, *bits)).collect(),
            clobbers,
            memory,
        });

        // Each output is a hidden binding, then assigned or bound as source would be.
        for (name, target, span) in &asm.outputs {
            let (register, part, bits) = self.operand(name, *span)?;
            let word = hir::Operand::Value(results[outputs.iter().position(|(one, _)| *one == register).expect("an output")]);
            let whole = whole_type(bits);
            let (value, type_name) = match part {
                Part::Word => (word, whole),
                Part::Low => (self.resized(word, whole, TypeName::U8), TypeName::U8),
                Part::High => {
                    let shifted = self.bit_shift("shr", word, 8, whole);
                    (self.resized(shifted, whole, TypeName::U8), TypeName::U8)
                }
            };
            let hidden = self.hidden("asm");
            let place = self.place(&hidden, type_name, false);
            self.emit("store", Vec::new(), vec![hir::Operand::Place(place), value], None);
            self.scopes.last_mut().expect("scope").insert(
                hidden.clone(),
                Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Place(place) },
            );
            let value = Expr::Name(hidden, *span);
            let statement = match target {
                AsmTarget::Bind { mutable, name } => {
                    Statement::Bind { mutable: *mutable, name: name.clone(), annotation: None, value, span: *span }
                }
                AsmTarget::Place(target) => Statement::Assign { target: target.clone(), operation: None, value, span: *span },
            };
            self.statement(&statement)?;
        }
        Ok(())
    }

    /// The register `name` an input or output names, by the target's register file: its root, the part
    /// of it and the bits of the whole it is in (16, or 32 where the target has an e-register named).
    fn operand(&self, name: &str, span: Span) -> Result<(Reg, Part, u32), Diagnostic> {
        let refused = || Diagnostic::new(span, format!("{name} is not a register an input or output may name: ax..di or their bytes, and the target's wider registers"));
        let file = |one: &str| self.types.registers.iter().find(|register| register.name.eq_ignore_ascii_case(one));
        // The register must be in the target's file, general, and within the target's code.
        let register = file(name).ok_or_else(refused)?;
        let root = file(&register.root).ok_or_else(refused)?;
        if !root.is("gpr") || root.is("reserved") || register.bits > self.types.code_bits {
            return Err(refused());
        }
        let (root16, bits) = inline_asm::view(name).ok_or_else(refused)?;
        let part = match (register.bits, register.lane) {
            (8, 0) => Part::Low,
            (8, 8) => Part::High,
            _ => Part::Word,
        };
        Ok((root16, part, if register.bits == 32 { 32 } else { bits.min(16) }))
    }

    /// `value` as the register `name` holds it: a 16-bit or 8-bit integer (32 for an e-register),
    /// or a near pointer, whose object the block may then reach.
    fn register_value(&mut self, value: &Expr, part: Part, bits: u32, name: &str, span: Span) -> Result<hir::Operand, Diagnostic> {
        let target = if part == Part::Word { whole_type(bits) } else { TypeName::U8 };
        let typed = if is_integer_literal(value) || matches!(value, Expr::Character(..)) {
            self.coerced(value, target)?
        } else {
            self.expression(value, None)?
        };
        // A near pointer fills a whole that is as wide as the target's near pointer.
        if matches!(typed.type_name, TypeName::Pointer { far: false, .. }) && part == Part::Word && width(self.types.sizes, typed.type_name) == width(self.types.sizes, target) {
            return required(typed, span);
        }
        let fits = (is_integer(typed.type_name) || typed.type_name == TypeName::Char) && width(self.types.sizes, typed.type_name) == width(self.types.sizes, target);
        if !fits {
            let near = if part == Part::Word { " or a near pointer" } else { "" };
            return Err(Diagnostic::new(
                span,
                format!("{name} takes a {}-bit integer{near}, not {}", 8 * width(self.types.sizes, target), type_name_text(typed.type_name)),
            ));
        }
        let from = typed.type_name;
        Ok(self.resized(required(typed, span)?, from, target))
    }
}

/// The integer a whole register of `bits` holds.
fn whole_type(bits: u32) -> TypeName {
    if bits == 32 { TypeName::U32 } else { TypeName::U16 }
}

/// The name the HIR spells a register of `bits` with: `ax`, or `eax` for its 32-bit view.
fn register_name(register: Reg, bits: u32) -> String {
    let name = register.name().to_lowercase();
    if bits == 32 { format!("e{name}") } else { name }
}
