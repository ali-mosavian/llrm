//! `dict[K, V]` (sections 3 and 13): an open-addressed hash table. Its value
//! is a near pointer to its slots behind a vec's descriptor, whose length word
//! counts the slots, a power of two, and whose capacity word the entries. A
//! slot is an entry `{hash, key, value}`; a zero hash marks it free and its
//! zeroed fields own nothing. So a dict drops and copies as the vec of its
//! slots does, and the runtime grows it by the stored hashes, calling no key
//! method. The probe is compiled here, over the slots as that vec.

use super::*;
use crate::frontends::modern::lexer::lex;
use crate::frontends::modern::parser::parse;
use super::borrows::expression_owner;
use super::vectors::GENERATED;
use crate::frontends::modern::syntax::{Clause, Struct, StructField};

/// Finds `KEY`'s slot in `SLOTS`: its own when `FOUND`, else the free one it
/// would take. `KEY` hashes and compares by its `Hashable` methods.
const PROBE: &str = "\
let HASH: u16 = KEY.hash() | 1
let mut AT: u16 = 0
let mut FOUND = false
if SLOTS.len != 0:
    let MASK: u16 = SLOTS.len - 1
    AT = HASH & MASK
    while SLOTS[AT].hash != 0:
        if SLOTS[AT].hash == HASH && SLOTS[AT].key.eq(KEY):
            FOUND = true
            break
        AT = (AT + 1) & MASK
";

/// A free slot found takes the key.
const CLAIM: &str = "\
if !FOUND:
    SLOTS[AT].hash = HASH
    SLOTS[AT].key = KEY
";

const PLACEHOLDERS: [&str; 6] = ["SLOTS", "KEY", "HASH", "AT", "FOUND", "MASK"];

impl TypeRegistry {
    /// `dict[key, value]`, registered on first use with its entry struct.
    pub(super) fn dictionary(&mut self, key: ElementType, value: ElementType, span: Span) -> Result<TypeName, Diagnostic> {
        if let Some((&type_id, _)) = self.dictionaries.iter().find(|(_, one)| (one.0, one.1) == (key, value)) {
            return Ok(TypeName::Dictionary { type_id });
        }
        let text = |one: ElementType| self.types[(one.id() - 1) as usize].name.clone();
        let name = format!("dict[{}, {}]", text(key), text(value));
        let entry_name = format!("{name}.entry");
        let fields = [("hash", ElementType::Scalar(TypeName::U16)), ("key", key), ("value", value)]
            .into_iter()
            .map(|(field, element)| StructField { name: field.into(), mutable: true, type_spec: self.spec_of(element), span })
            .collect();
        self.register_struct(&Struct { name: entry_name.clone(), generics: Vec::new(), bits: None, pack: None, fields, span })?;
        let entry = self.structs[&entry_name].id;
        let type_id = self.types.len() as u32 + 1;
        self.types.push(hir::Type {
            id: type_id,
            name,
            kind: "pointer",
            width: 2,
            signed: None,
            evaluation: "none",
            element: Some(entry),
            rank: 0,
            bounds: Vec::new(),
            address: "near",
        });
        self.dictionaries.insert(type_id, (key, value, entry));
        Ok(TypeName::Dictionary { type_id })
    }

    /// A dict type's key, value and entry struct.
    pub(super) fn dictionary_parts(&self, type_name: TypeName) -> Option<(ElementType, ElementType, u32)> {
        let TypeName::Dictionary { type_id } = type_name else {
            return None;
        };
        self.dictionaries.get(&type_id).copied()
    }

    /// What a heap buffer of this type holds, one per length unit: a string's
    /// chars, a vec's elements, a dict's slots.
    pub(super) fn owned_element(&self, type_name: TypeName) -> Option<ElementType> {
        self.sequence_element(type_name)
            .or_else(|| self.dictionary_parts(type_name).map(|(_, _, entry)| ElementType::Struct(entry)))
    }

    /// What `x[i]` of a value of this type reads: an element, or a dict's value.
    pub(super) fn indexed(&self, type_name: TypeName) -> Option<ElementType> {
        self.sequence_element(type_name)
            .or_else(|| self.dictionary_parts(type_name).map(|(_, value, _)| value))
    }
}

/// The names one probe binds, its placeholders' stand-ins.
struct Probe {
    names: BTreeMap<&'static str, String>,
}

impl Probe {
    fn name(&self, placeholder: &str) -> Expr {
        Expr::Name(self.names[placeholder].clone(), GENERATED)
    }

    /// `SLOTS[AT].field`.
    fn slot(&self, field: &str) -> Expr {
        let slot = Expr::Index { base: Box::new(self.name("SLOTS")), indices: vec![self.name("AT")], span: GENERATED };
        Expr::Member { base: Box::new(slot), field: field.into(), span: GENERATED }
    }
}

impl FunctionCompiler<'_> {
    /// `{k: v, ...}` or `{k: v for ...}`: a new dict, filled by assignment,
    /// held by a hidden local until it is moved where it goes.
    pub(super) fn dictionary_literal(&mut self, expression: &Expr, expected: Option<TypeName>, span: Span) -> Result<TypedOperand, Diagnostic> {
        let type_name = match expected {
            Some(one @ TypeName::Dictionary { .. }) => one,
            Some(other) => return Err(Diagnostic::new(span, format!("a dict literal is a dict, not {}", type_name_text(other)))),
            None => self.dictionary_type_hint(expression).ok_or_else(|| Diagnostic::new(span, "an empty dict needs a dict type"))?,
        };
        let name = self.hidden("dict");
        let place = self.place(&name, type_name, true);
        let empty = self.empty(type_name);
        self.emit("store", Vec::new(), vec![hir::Operand::Place(place), hir::Operand::Value(empty)], None);
        self.own(place);
        let binding = Binding { type_: BindingType::Scalar(type_name), mutable: true, storage: Storage::Place(place) };
        self.scopes.last_mut().expect("scope").insert(name.clone(), binding);
        let store = |key: &Expr, value: &Expr| Statement::Assign {
            target: AssignTarget::Index { base: name.clone(), indices: vec![key.clone()] },
            operation: None,
            value: value.clone(),
            span,
        };
        let fill = match expression {
            Expr::Dict(entries, _) => entries.iter().map(|(key, value)| store(key, value)).collect(),
            Expr::DictComprehension { key, value, clauses, .. } => Clause::loops(clauses, vec![store(key, value)]),
            _ => unreachable!("a dict literal"),
        };
        for statement in &fill {
            self.generated_statement(statement)?;
        }
        self.expression(&Expr::Name(name, span), Some(type_name))
    }

    /// The dict type an unannotated literal builds, from its first entry.
    pub(super) fn dictionary_type_hint(&mut self, expression: &Expr) -> Option<TypeName> {
        let (key, value) = match expression {
            Expr::Dict(entries, _) => {
                let (key, value) = entries.first()?;
                (self.element_hint(key)?, self.element_hint(value)?)
            }
            Expr::DictComprehension { key, value, clauses, .. } => {
                let depth = self.scopes.len();
                let hint = self.clause_scopes(clauses).and_then(|()| Some((self.element_hint(key)?, self.element_hint(value)?)));
                self.scopes.truncate(depth);
                hint?
            }
            _ => return None,
        };
        self.types.dictionary(key, value, expression.span()).ok()
    }

    /// `d[k]` read: `k`'s value, which must be there.
    pub(super) fn dictionary_value(&mut self, dictionary: &Expr, indices: &[Expr], span: Span) -> Result<Option<Expr>, Diagnostic> {
        let Some(type_name) = self.expression_type_hint(dictionary).filter(|one| self.types.dictionary_parts(*one).is_some()) else {
            return Ok(None);
        };
        let key = single_key(indices, span)?;
        let table = self.dictionary_table(dictionary, type_name, span)?;
        let probe = self.probe(table, type_name, key, true, span)?;
        let found = required(self.expression(&probe.name("FOUND"), Some(TypeName::Bool))?, span)?;
        self.panic_unless(found, "_rt_panic_key");
        Ok(Some(probe.slot("value")))
    }

    /// `d[k]` as a place: `k`'s value, a new entry when `k` was not there.
    pub(super) fn dictionary_entry(&mut self, dictionary: &str, indices: &[Expr], span: Span) -> Result<Option<Expr>, Diagnostic> {
        let Some(type_name) = self.visible(dictionary).and_then(|one| match one.type_ {
            BindingType::Scalar(one @ TypeName::Dictionary { .. }) => Some(one),
            _ => None,
        }) else {
            return Ok(None);
        };
        let key = single_key(indices, span)?;
        let (_, _, entry) = self.types.dictionary_parts(type_name).expect("a dict");
        let size = hir::Operand::Constant(U16, i64::from(self.types.width(entry)));
        let place = self.sequence_place(&Expr::Name(dictionary.into(), span), span)?;
        let table = self.value(type_name);
        self.emit("load", vec![table], vec![place.clone()], None);
        let grown = self.emit_builtin("_rt_dict_reserve", vec![hir::Operand::Value(table), size]).expect("a table");
        let grown = self.retyped(grown, type_name);
        self.emit("store", Vec::new(), vec![place, hir::Operand::Value(grown)], None);
        let probe = self.probe(grown, type_name, key, false, span)?;
        for statement in self.generated(CLAIM, &probe)? {
            self.generated_statement(&statement)?;
        }
        // One entry more, unless the key was there.
        let found = required(self.expression(&probe.name("FOUND"), Some(TypeName::Bool))?, span)?;
        let (count, done) = (self.block(), self.block());
        self.terminate(hir::Terminator { kind: "branch", operands: vec![found], targets: vec![done, count] });
        self.current = count;
        let entries = hir::Operand::DescriptorPlace { base: grown, field: "capacity", type_id: U16 };
        let before = self.value(TypeName::U16);
        self.emit("load", vec![before], vec![entries.clone()], None);
        let after = self.value(TypeName::U16);
        self.emit("add", vec![after], vec![hir::Operand::Value(before), hir::Operand::Constant(U16, 1)], None);
        self.emit("store", Vec::new(), vec![entries, hir::Operand::Value(after)], None);
        self.terminate(jump(done));
        self.current = done;
        Ok(Some(probe.slot("value")))
    }

    /// `d.len`, `d.get(k, default)` and `d.contains(k)`.
    pub(super) fn dictionary_method(
        &mut self,
        receiver: &Expr,
        type_name: TypeName,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let table = self.dictionary_table(receiver, type_name, span)?;
        match (name, arguments) {
            ("len", []) => {
                let count = self.value(TypeName::U16);
                self.emit("load", vec![count], vec![hir::Operand::DescriptorPlace { base: table, field: "capacity", type_id: U16 }], None);
                self.implicit(TypedOperand { operand: Some(hir::Operand::Value(count)), type_name: TypeName::U16 }, expected.unwrap_or(TypeName::U16), span)
            }
            ("contains", [key]) => {
                let probe = self.probe(table, type_name, key, true, span)?;
                self.expression(&probe.name("FOUND"), expected)
            }
            ("get", [key, default]) => {
                let probe = self.probe(table, type_name, key, true, span)?;
                let chosen = Expr::Conditional {
                    condition: Box::new(probe.name("FOUND")),
                    then: Box::new(probe.slot("value")),
                    otherwise: Box::new(default.clone()),
                    span,
                };
                self.expression(&chosen, expected)
            }
            _ => Err(Diagnostic::new(span, format!("dict has no method {name:?} of {} arguments", arguments.len()))),
        }
    }

    /// The table `dictionary` reads, not moved.
    fn dictionary_table(&mut self, dictionary: &Expr, type_name: TypeName, span: Span) -> Result<u32, Diagnostic> {
        let value = self.expression(dictionary, Some(type_name))?;
        Ok(self.materialized(required(value, span)?, type_id(type_name)))
    }

    /// Binds a probe of `table` for `key`, and runs it. A lookup lends a key
    /// that is a place; an insert moves its key into the table.
    fn probe(&mut self, table: u32, type_name: TypeName, key: &Expr, lend: bool, span: Span) -> Result<Probe, Diagnostic> {
        let (key_type, _, entry) = self.types.dictionary_parts(type_name).expect("a dict");
        let names = PLACEHOLDERS.into_iter().map(|one| (one, self.hidden(&one.to_lowercase()))).collect();
        let probe = Probe { names };
        let slots_type = self.types.vector(ElementType::Struct(entry));
        let slots = self.value(slots_type);
        self.emit("copy", vec![slots], vec![hir::Operand::Value(table)], None);
        let binding = Binding { type_: BindingType::Scalar(slots_type), mutable: true, storage: Storage::Parameter(slots) };
        self.scopes.last_mut().expect("scope").insert(probe.names["SLOTS"].clone(), binding);
        let lent = lend && expression_owner(key).is_some();
        let bind_key = Statement::Bind {
            mutable: false,
            name: probe.names["KEY"].clone(),
            annotation: (!lent).then(|| TypeAnnotation::Value(self.types.spec_of(key_type))),
            value: if lent { Expr::Borrow { mutable: false, operand: Box::new(key.clone()), span } } else { key.clone() },
            span,
        };
        self.generated_statement(&bind_key)?;
        for statement in self.generated(PROBE, &probe)? {
            self.generated_statement(&statement)?;
        }
        Ok(probe)
    }

    /// `source`, a statement list in this language, with each placeholder
    /// renamed to what `probe` binds it to.
    fn generated(&self, source: &str, probe: &Probe) -> Result<Vec<Statement>, Diagnostic> {
        let indented: String = source.lines().map(|line| format!("    {line}\n")).collect();
        let module = parse(lex(&format!("fn generated() -> void:\n{indented}"))?)?;
        let mut body = module.functions.into_iter().next().expect("one function").body;
        let rename = |name: &mut String| {
            if let Some(renamed) = probe.names.get(name.as_str()) {
                *name = renamed.clone();
            }
        };
        for statement in &mut body {
            statement.each_mut(&mut |one| match one {
                Statement::Bind { name, .. } => rename(name),
                Statement::Assign { target: AssignTarget::Name(name) | AssignTarget::Index { base: name, .. }, .. } => rename(name),
                _ => {}
            });
            let Ok(()) = statement.walk_mut(&mut |expression| -> Result<(), std::convert::Infallible> {
                if let Expr::Name(name, _) = expression {
                    rename(name);
                }
                Ok(())
            });
        }
        Ok(body)
    }

    /// Compiles a statement the compiler made, readied as a written one is.
    fn generated_statement(&mut self, statement: &Statement) -> Result<(), Diagnostic> {
        match self.prepared(statement)? {
            Some(rewritten) => self.statement(&rewritten),
            None => self.statement(statement),
        }
    }
}

/// A dict takes one key in brackets.
fn single_key(indices: &[Expr], span: Span) -> Result<&Expr, Diagnostic> {
    match indices {
        [key] => Ok(key),
        _ => Err(Diagnostic::new(span, "a dict is indexed by one key")),
    }
}
