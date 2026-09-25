//! Inline assembly (section 15): one HIR `asm` instruction whose operands
//! go into 16-bit registers and whose results come out of them. A byte
//! register is a part of its word, packed and unpacked here, so the
//! machine side sees only words.

use super::*;
use crate::syntax::{Asm, AsmTarget};
use llrm_core::abi::runtime::Reg;
use llrm_core::backend::inline_asm::{self, Part};

impl FunctionCompiler<'_> {
    pub(super) fn asm_statement(&mut self, asm: &Asm) -> Result<(), Diagnostic> {
        self.require_unsafe("inline assembly", asm.span)?;
        let lines: Vec<&str> = asm.lines.iter().map(|(text, _)| text.as_str()).collect();
        let code = inline_asm::assembled(&lines)
            .map_err(|refused| Diagnostic::new(asm.lines[refused.line].1, refused.message))?;

        // Each word register's input: whole, or its bytes.
        let mut words: Vec<(Reg, [Option<hir::Operand>; 3])> = Vec::new();
        for (name, value, span) in &asm.inputs {
            let (register, part) = operand(name, *span)?;
            let value = self.register_value(value, part, name, *span)?;
            let at = match words.iter().position(|(one, _)| *one == register) {
                Some(at) => at,
                None => {
                    words.push((register, [None, None, None]));
                    words.len() - 1
                }
            };
            let parts = &mut words[at].1;
            let whole = part == Part::Word || parts[Part::Word as usize].is_some();
            if parts[part as usize].is_some() || (whole && parts.iter().any(Option::is_some)) {
                return Err(Diagnostic::new(*span, format!("{name} overlaps another input")));
            }
            parts[part as usize] = Some(value);
        }
        let mut inputs = Vec::new();
        let mut operands = Vec::new();
        for (register, [word, low, high]) in words {
            let low = low.map(|one| self.resized(one, TypeName::U8, TypeName::U16));
            let high = high.map(|one| {
                let widened = self.resized(one, TypeName::U8, TypeName::U16);
                self.bit_shift("shl", widened, 8, TypeName::U16)
            });
            let value = match (word, low, high) {
                (Some(word), ..) => word,
                (None, Some(low), Some(high)) => self.binary_op("or", low, high, TypeName::U16),
                (None, Some(part), None) | (None, None, Some(part)) => part,
                (None, None, None) => unreachable!("a register is listed for an input"),
            };
            inputs.push(register_name(register));
            operands.push(value);
        }

        let mut outputs: Vec<Reg> = Vec::new();
        for (index, (name, _, span)) in asm.outputs.iter().enumerate() {
            let (register, _) = operand(name, *span)?;
            if asm.outputs[..index].iter().any(|(other, ..)| other.eq_ignore_ascii_case(name)) {
                return Err(Diagnostic::new(*span, format!("{name} is an output twice")));
            }
            if !outputs.contains(&register) {
                outputs.push(register);
            }
        }
        let mut memory = false;
        let mut clobbers = Vec::new();
        for (name, span) in &asm.clobbers {
            if name == "memory" {
                memory = true;
                continue;
            }
            let register = inline_asm::clobbered(name).ok_or_else(|| {
                Diagnostic::new(*span, format!("{name} is not a register a block may clobber: ax..di, their bytes, es, flags or memory"))
            })?;
            clobbers.push(register_name(register));
        }

        let results: Vec<u32> = outputs.iter().map(|_| self.value(TypeName::U16)).collect();
        self.emit("asm", results.clone(), operands, None);
        let instruction = self.current_block_mut().instructions.last_mut().expect("the asm instruction");
        instruction.asm = Some(hir::Asm {
            code,
            inputs,
            outputs: outputs.iter().map(|one| register_name(*one)).collect(),
            clobbers,
            memory,
        });

        // Each output is a hidden binding, then assigned or bound as source would be.
        for (name, target, span) in &asm.outputs {
            let (register, part) = operand(name, *span)?;
            let word = hir::Operand::Value(results[outputs.iter().position(|one| *one == register).expect("an output")]);
            let (value, type_name) = match part {
                Part::Word => (word, TypeName::U16),
                Part::Low => (self.resized(word, TypeName::U16, TypeName::U8), TypeName::U8),
                Part::High => {
                    let shifted = self.bit_shift("shr", word, 8, TypeName::U16);
                    (self.resized(shifted, TypeName::U16, TypeName::U8), TypeName::U8)
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

    /// `value` as the register `name` holds it: a 16-bit or 8-bit integer,
    /// or a near pointer, whose object the block may then reach.
    fn register_value(&mut self, value: &Expr, part: Part, name: &str, span: Span) -> Result<hir::Operand, Diagnostic> {
        let target = if part == Part::Word { TypeName::U16 } else { TypeName::U8 };
        let typed = if is_integer_literal(value) || matches!(value, Expr::Character(..)) {
            self.coerced(value, target)?
        } else {
            self.expression(value, None)?
        };
        if matches!(typed.type_name, TypeName::Pointer { width: 2, .. }) && part == Part::Word {
            return required(typed, span);
        }
        let fits = (is_integer(typed.type_name) || typed.type_name == TypeName::Char) && width(typed.type_name) == width(target);
        if !fits {
            let near = if part == Part::Word { " or a near pointer" } else { "" };
            return Err(Diagnostic::new(
                span,
                format!("{name} takes a {}-bit integer{near}, not {}", 8 * width(target), type_name_text(typed.type_name)),
            ));
        }
        let from = typed.type_name;
        Ok(self.resized(required(typed, span)?, from, target))
    }
}

fn operand(name: &str, span: Span) -> Result<(Reg, Part), Diagnostic> {
    inline_asm::operand_register(name)
        .ok_or_else(|| Diagnostic::new(span, format!("{name} is not a register an input or output may name: ax..di or their bytes")))
}

fn register_name(register: Reg) -> String {
    register.name().to_lowercase()
}
